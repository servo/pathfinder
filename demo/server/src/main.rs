// pathfinder/demo/server/main.rs
//
// Copyright © 2017 Mozilla Foundation

#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate app_units;
extern crate base64;
extern crate bincode;
extern crate euclid;
extern crate fontsan;
extern crate pathfinder_font_renderer;
extern crate pathfinder_partitioner;
extern crate rocket;
extern crate rocket_contrib;
extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

use app_units::Au;
use bincode::Infinite;
use euclid::{Point2D, Size2D, Transform2D};
use pathfinder_font_renderer::{FontContext, FontInstanceKey, FontKey};
use pathfinder_font_renderer::{GlyphKey, GlyphOutlineBuffer};
use pathfinder_partitioner::partitioner::Partitioner;
use rocket::http::{ContentType, Status};
use rocket::request::Request;
use rocket::response::{NamedFile, Responder, Response};
use rocket_contrib::json::Json;
use serde::Serialize;
use std::fs::File;
use std::io;
use std::mem;
use std::path::{Path, PathBuf};

static STATIC_ROOT_PATH: &'static str = "../client/index.html";
static STATIC_CSS_BOOTSTRAP_PATH: &'static str = "../client/node_modules/bootstrap/dist/css";
static STATIC_JS_BOOTSTRAP_PATH: &'static str = "../client/node_modules/bootstrap/dist/js";
static STATIC_JS_JQUERY_PATH: &'static str = "../client/node_modules/jquery/dist";
static STATIC_JS_PATHFINDER_JS_PATH: &'static str = "../client/pathfinder.js";
static STATIC_GLSL_PATH: &'static str = "../../shaders";

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct IndexRange {
    start: usize,
    end: usize,
}

impl IndexRange {
    fn new(start: usize, end: usize) -> IndexRange {
        IndexRange {
            start: start,
            end: end,
        }
    }

    fn from_data<T>(dest: &mut Vec<u8>, src: &[T]) -> Result<IndexRange, ()> where T: Serialize {
        let byte_len_before = dest.len();
        for src_value in src {
            try!(bincode::serialize_into(dest, src_value, Infinite).map_err(drop))
        }
        let byte_len_after = dest.len();
        Ok(IndexRange {
            start: byte_len_before / mem::size_of::<T>(),
            end: byte_len_after / mem::size_of::<T>(),
        })
    }
}

#[allow(non_snake_case)]
#[derive(Clone, Serialize, Deserialize)]
struct PartitionFontRequest {
    // Base64 encoded.
    otf: String,
    fontIndex: u32,
    glyphs: Vec<PartitionGlyph>,
    pointSize: f64,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
struct PartitionGlyph {
    id: u32,
    transform: Transform2D<f32>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
struct PartitionGlyphDimensions {
    origin: Point2D<i32>,
    size: Size2D<u32>,
    advance: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct DecodedOutlineIndices {
    endpoint_indices: IndexRange,
    control_point_indices: IndexRange,
    subpath_indices: IndexRange,
}

#[allow(non_snake_case)]
#[derive(Clone, Copy, Serialize, Deserialize)]
struct PartitionGlyphInfo {
    id: u32,
    dimensions: PartitionGlyphDimensions,
    bQuadIndices: IndexRange,
    bVertexIndices: IndexRange,
    coverInteriorIndices: IndexRange,
    coverCurveIndices: IndexRange,
    edgeUpperLineIndices: IndexRange,
    edgeUpperCurveIndices: IndexRange,
    edgeLowerLineIndices: IndexRange,
    edgeLowerCurveIndices: IndexRange,
}

#[allow(non_snake_case)]
#[derive(Clone, Serialize, Deserialize)]
struct PartitionFontResponse {
    glyphInfo: Vec<PartitionGlyphInfo>,
    // Base64-encoded `bincode`-encoded `BQuad`s.
    bQuads: String,
    // Base64-encoded `bincode`-encoded `Point2D<f32>`s.
    bVertexPositions: String,
    // Base64-encoded `bincode`-encoded `u16`s.
    bVertexPathIDs: String,
    // Base64-encoded `bincode`-encoded `BVertexLoopBlinnData`s.
    bVertexLoopBlinnData: String,
    // Base64-encoded `u32`s.
    coverInteriorIndices: String,
    // Base64-encoded `u32`s.
    coverCurveIndices: String,
    // Base64-encoded `bincode`-encoded `LineIndices` instances.
    edgeUpperLineIndices: String,
    // Base64-encoded `bincode`-encoded `CurveIndices` instances.
    edgeUpperCurveIndices: String,
    // Base64-encoded `bincode`-encoded `LineIndices` instances.
    edgeLowerLineIndices: String,
    // Base64-encoded `bincode`-encoded `CurveIndices` instances.
    edgeLowerCurveIndices: String,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum PartitionFontError {
    Base64DecodingFailed,
    FontSanitizationFailed,
    FontLoadingFailed,
    BincodeSerializationFailed,
    Unimplemented,
}

#[post("/partition-font", format = "application/json", data = "<request>")]
fn partition_font(request: Json<PartitionFontRequest>)
                  -> Json<Result<PartitionFontResponse, PartitionFontError>> {
    let unsafe_otf_data = match base64::decode(&request.otf) {
        Ok(unsafe_otf_data) => unsafe_otf_data,
        Err(_) => return Json(Err(PartitionFontError::Base64DecodingFailed)),
    };

    // Sanitize.
    let otf_data = match fontsan::process(&unsafe_otf_data) {
        Ok(otf_data) => otf_data,
        Err(_) => return Json(Err(PartitionFontError::FontSanitizationFailed)),
    };

    // Parse glyph data.
    let font_key = FontKey::new();
    let font_instance_key = FontInstanceKey {
        font_key: font_key,
        size: Au::from_f64_px(request.pointSize),
    };
    let mut font_context = FontContext::new();
    if font_context.add_font_from_memory(&font_key, otf_data, request.fontIndex).is_err() {
        return Json(Err(PartitionFontError::FontLoadingFailed))
    }

    // Read glyph info.
    let mut outline_buffer = GlyphOutlineBuffer::new();
    let decoded_outline_indices: Vec<_> = request.glyphs.iter().map(|glyph| {
        let glyph_key = GlyphKey::new(glyph.id);

        let first_endpoint_index = outline_buffer.endpoints.len();
        let first_control_point_index = outline_buffer.control_points.len();
        let first_subpath_index = outline_buffer.subpaths.len();

        // This might fail; if so, just leave it blank.
        drop(font_context.push_glyph_outline(&font_instance_key,
                                             &glyph_key,
                                             &mut outline_buffer,
                                             &glyph.transform));

        let last_endpoint_index = outline_buffer.endpoints.len();
        let last_control_point_index = outline_buffer.control_points.len();
        let last_subpath_index = outline_buffer.subpaths.len();

        DecodedOutlineIndices {
            endpoint_indices: IndexRange::new(first_endpoint_index, last_endpoint_index),
            control_point_indices: IndexRange::new(first_control_point_index,
                                                   last_control_point_index),
            subpath_indices: IndexRange::new(first_subpath_index, last_subpath_index),
        }
    }).collect();

    // Partition the decoded glyph outlines.
    let mut partitioner = Partitioner::new();
    let (mut b_quads, mut b_vertex_positions) = (vec![], vec![]);
    let (mut b_vertex_path_ids, mut b_vertex_loop_blinn_data) = (vec![], vec![]);
    let (mut cover_interior_indices, mut cover_curve_indices) = (vec![], vec![]);
    let (mut edge_upper_line_indices, mut edge_upper_curve_indices) = (vec![], vec![]);
    let (mut edge_lower_line_indices, mut edge_lower_curve_indices) = (vec![], vec![]);

    partitioner.init(&outline_buffer.endpoints,
                     &outline_buffer.control_points,
                     &outline_buffer.subpaths);

    let mut glyph_info = vec![];
    for (path_index, (&glyph, decoded_outline_indices)) in
            request.glyphs.iter().zip(decoded_outline_indices.iter()).enumerate() {
        let glyph_key = GlyphKey::new(glyph.id);

        let dimensions = match font_context.glyph_dimensions(&font_instance_key, &glyph_key) {
            Some(dimensions) => {
                PartitionGlyphDimensions {
                    origin: dimensions.origin,
                    size: dimensions.size,
                    advance: dimensions.advance,
                }
            }
            None => {
                PartitionGlyphDimensions {
                    origin: Point2D::zero(),
                    size: Size2D::zero(),
                    advance: 0.0,
                }
            }
        };

        partitioner.partition((path_index + 1) as u16,
                              decoded_outline_indices.subpath_indices.start as u32,
                              decoded_outline_indices.subpath_indices.end as u32);

        let path_b_vertex_positions = partitioner.b_vertex_positions();
        let path_b_vertex_path_ids = partitioner.b_vertex_path_ids();
        let path_b_vertex_loop_blinn_data = partitioner.b_vertex_loop_blinn_data();
        let cover_indices = partitioner.cover_indices();
        let edge_indices = partitioner.edge_indices();

        let positions_start = IndexRange::from_data(&mut b_vertex_positions,
                                                    path_b_vertex_positions).unwrap().start as u32;
        IndexRange::from_data(&mut b_vertex_path_ids, path_b_vertex_path_ids).unwrap();

        let mut path_b_quads = partitioner.b_quads().to_vec();
        let mut path_cover_interior_indices = cover_indices.interior_indices.to_vec();
        let mut path_cover_curve_indices = cover_indices.curve_indices.to_vec();
        let mut path_edge_upper_line_indices = edge_indices.upper_line_indices.to_vec();
        let mut path_edge_upper_curve_indices = edge_indices.upper_curve_indices.to_vec();
        let mut path_edge_lower_line_indices = edge_indices.lower_line_indices.to_vec();
        let mut path_edge_lower_curve_indices = edge_indices.lower_curve_indices.to_vec();

        for path_b_quad in &mut path_b_quads {
            path_b_quad.offset(positions_start);
        }
        for path_cover_interior_index in &mut path_cover_interior_indices {
            *path_cover_interior_index += positions_start
        }
        for path_cover_curve_index in &mut path_cover_curve_indices {
            *path_cover_curve_index += positions_start
        }
        for path_edge_upper_line_indices in &mut path_edge_upper_line_indices {
            path_edge_upper_line_indices.offset(positions_start);
        }
        for path_edge_upper_curve_indices in &mut path_edge_upper_curve_indices {
            path_edge_upper_curve_indices.offset(positions_start);
        }
        for path_edge_lower_line_indices in &mut path_edge_lower_line_indices {
            path_edge_lower_line_indices.offset(positions_start);
        }
        for path_edge_lower_curve_indices in &mut path_edge_lower_curve_indices {
            path_edge_lower_curve_indices.offset(positions_start);
        }

        glyph_info.push(PartitionGlyphInfo {
            id: glyph.id,
            dimensions: dimensions,
            bQuadIndices: IndexRange::from_data(&mut b_quads, &path_b_quads).unwrap(),
            bVertexIndices: IndexRange::from_data(&mut b_vertex_loop_blinn_data,
                                                  path_b_vertex_loop_blinn_data).unwrap(),
            coverInteriorIndices: IndexRange::from_data(&mut cover_interior_indices,
                                                        &path_cover_interior_indices).unwrap(),
            coverCurveIndices: IndexRange::from_data(&mut cover_curve_indices,
                                                     &path_cover_curve_indices).unwrap(),
            edgeUpperLineIndices: IndexRange::from_data(&mut edge_upper_line_indices,
                                                        &path_edge_upper_line_indices).unwrap(),
            edgeUpperCurveIndices: IndexRange::from_data(&mut edge_upper_curve_indices,
                                                         &path_edge_upper_curve_indices).unwrap(),
            edgeLowerLineIndices: IndexRange::from_data(&mut edge_lower_line_indices,
                                                        &path_edge_lower_line_indices).unwrap(),
            edgeLowerCurveIndices: IndexRange::from_data(&mut edge_lower_curve_indices,
                                                         &path_edge_lower_curve_indices).unwrap(),
        })
    }

    // Return the response.
    Json(Ok(PartitionFontResponse {
        glyphInfo: glyph_info,
        bQuads: base64::encode(&b_quads),
        bVertexPositions: base64::encode(&b_vertex_positions),
        bVertexPathIDs: base64::encode(&b_vertex_path_ids),
        bVertexLoopBlinnData: base64::encode(&b_vertex_loop_blinn_data),
        coverInteriorIndices: base64::encode(&cover_interior_indices),
        coverCurveIndices: base64::encode(&cover_curve_indices),
        edgeUpperLineIndices: base64::encode(&edge_upper_line_indices),
        edgeUpperCurveIndices: base64::encode(&edge_upper_curve_indices),
        edgeLowerLineIndices: base64::encode(&edge_lower_line_indices),
        edgeLowerCurveIndices: base64::encode(&edge_lower_curve_indices),
    }))
}

// Static files
#[get("/")]
fn static_index() -> io::Result<NamedFile> {
    NamedFile::open(STATIC_ROOT_PATH)
}
#[get("/js/pathfinder.js")]
fn static_js_pathfinder_js() -> io::Result<NamedFile> {
    NamedFile::open(STATIC_JS_PATHFINDER_JS_PATH)
}
#[get("/css/bootstrap/<file..>")]
fn static_css_bootstrap(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_CSS_BOOTSTRAP_PATH).join(file)).ok()
}
#[get("/js/bootstrap/<file..>")]
fn static_js_bootstrap(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_JS_BOOTSTRAP_PATH).join(file)).ok()
}
#[get("/js/jquery/<file..>")]
fn static_js_jquery(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_JS_JQUERY_PATH).join(file)).ok()
}
#[get("/glsl/<file..>")]
fn static_glsl(file: PathBuf) -> Option<Shader> {
    Shader::open(Path::new(STATIC_GLSL_PATH).join(file)).ok()
}

struct Shader {
    file: File,
}

impl Shader {
    fn open(path: PathBuf) -> io::Result<Shader> {
        File::open(path).map(|file| Shader {
            file: file,
        })
    }
}

impl<'a> Responder<'a> for Shader {
    fn respond_to(self, _: &Request) -> Result<Response<'a>, Status> {
        Response::build().header(ContentType::Plain).streamed_body(self.file).ok()
    }
}

fn main() {
    rocket::ignite().mount("/", routes![
        partition_font,
        static_index,
        static_js_pathfinder_js,
        static_css_bootstrap,
        static_js_bootstrap,
        static_js_jquery,
        static_glsl,
    ]).launch();
}
