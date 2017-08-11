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
use euclid::{Point2D, Size2D};
use pathfinder_font_renderer::{FontContext, FontInstanceKey, FontKey};
use pathfinder_font_renderer::{GlyphKey, GlyphOutlineBuffer};
use pathfinder_partitioner::partitioner::Partitioner;
use rocket::response::NamedFile;
use rocket_contrib::json::Json;
use serde::Serialize;
use std::io;
use std::mem;
use std::path::{Path, PathBuf};

static STATIC_ROOT_PATH: &'static str = "../client/index.html";
static STATIC_CSS_BOOTSTRAP_PATH: &'static str = "../client/node_modules/bootstrap/dist/css";
static STATIC_JS_BOOTSTRAP_PATH: &'static str = "../client/node_modules/bootstrap/dist/js";
static STATIC_JS_JQUERY_PATH: &'static str = "../client/node_modules/jquery/dist";
static STATIC_JS_PATHFINDER_JS_PATH: &'static str = "../client/pathfinder.js";

#[derive(Clone, Copy, Serialize, Deserialize)]
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

    fn from_vector_append_operation<T>(dest: &mut Vec<T>, src: &[T]) -> IndexRange where T: Clone {
        let start = dest.len();
        dest.extend(src.iter().cloned());
        let end = dest.len();
        IndexRange {
            start: start,
            end: end,
        }
    }

    fn from_vector_append_and_serialization<T>(dest: &mut Vec<u8>, src: &[T])
                                               -> Result<IndexRange, ()>
                                               where T: Serialize {
        let byte_len_before = dest.len();
        try!(bincode::serialize_into(dest, src, Infinite).map_err(drop));
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
    glyphIDs: Vec<u32>,
    pointSize: f64,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
struct PartitionGlyphDimensions {
    origin: Point2D<i32>,
    size: Size2D<u32>,
    advance: f32,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
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
    // Base64-encoded `bincode`-encoded `BVertex`es.
    bVertices: String,
    coverInteriorIndices: Vec<u32>,
    coverCurveIndices: Vec<u32>,
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
    let decoded_outline_indices: Vec<_> = request.glyphIDs.iter().map(|&glyph_id| {
        let glyph_key = GlyphKey::new(glyph_id);

        let first_endpoint_index = outline_buffer.endpoints.len();
        let first_control_point_index = outline_buffer.control_points.len();
        let first_subpath_index = outline_buffer.subpaths.len();

        // This might fail; if so, just leave it blank.
        drop(font_context.push_glyph_outline(&font_instance_key, &glyph_key, &mut outline_buffer));

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
    let (mut b_quads, mut b_vertices) = (vec![], vec![]);
    let (mut cover_interior_indices, mut cover_curve_indices) = (vec![], vec![]);
    let (mut edge_upper_line_indices, mut edge_upper_curve_indices) = (vec![], vec![]);
    let (mut edge_lower_line_indices, mut edge_lower_curve_indices) = (vec![], vec![]);
    partitioner.init(&outline_buffer.endpoints,
                     &outline_buffer.control_points,
                     &outline_buffer.subpaths);
    let mut glyph_info = vec![];
    for (path_index, (&glyph_id, decoded_outline_indices)) in
            request.glyphIDs.iter().zip(decoded_outline_indices.iter()).enumerate() {
        let glyph_key = GlyphKey::new(glyph_id);

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

        partitioner.partition(path_index as u32,
                              decoded_outline_indices.subpath_indices.start as u32,
                              decoded_outline_indices.subpath_indices.end as u32);

        let (path_b_quads, path_b_vertices) = (partitioner.b_quads(), partitioner.b_vertices());
        let cover_indices = partitioner.cover_indices();
        let edge_indices = partitioner.edge_indices();

        glyph_info.push(PartitionGlyphInfo {
            id: glyph_id,
            dimensions: dimensions,
            bQuadIndices: IndexRange::from_vector_append_and_serialization(&mut b_quads,
                                                                           path_b_quads).unwrap(),
            bVertexIndices:
                IndexRange::from_vector_append_and_serialization(&mut b_vertices,
                                                                 path_b_vertices).unwrap(),
            coverInteriorIndices:
                IndexRange::from_vector_append_operation(&mut cover_interior_indices,
                                                         cover_indices.interior_indices),
            coverCurveIndices:
                IndexRange::from_vector_append_operation(&mut cover_curve_indices,
                                                         cover_indices.curve_indices),
            edgeUpperLineIndices: IndexRange::from_vector_append_and_serialization(
                &mut edge_upper_line_indices,
                edge_indices.upper_line_indices).unwrap(),
            edgeUpperCurveIndices: IndexRange::from_vector_append_and_serialization(
                &mut edge_upper_curve_indices,
                edge_indices.upper_curve_indices).unwrap(),
            edgeLowerLineIndices: IndexRange::from_vector_append_and_serialization(
                &mut edge_lower_line_indices,
                edge_indices.lower_line_indices).unwrap(),
            edgeLowerCurveIndices: IndexRange::from_vector_append_and_serialization(
                &mut edge_lower_curve_indices,
                edge_indices.lower_curve_indices).unwrap(),
        })
    }

    // Return the response.
    Json(Ok(PartitionFontResponse {
        glyphInfo: glyph_info,
        bQuads: base64::encode(&b_quads),
        bVertices: base64::encode(&b_vertices),
        coverInteriorIndices: cover_interior_indices,
        coverCurveIndices: cover_curve_indices,
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

fn main() {
    rocket::ignite().mount("/", routes![
        partition_font,
        static_index,
        static_js_pathfinder_js,
        static_css_bootstrap,
        static_js_bootstrap,
        static_js_jquery,
    ]).launch();
}
