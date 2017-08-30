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
use pathfinder_partitioner::{Endpoint, Subpath};
use rocket::http::{ContentType, Status};
use rocket::request::Request;
use rocket::response::{NamedFile, Responder, Response};
use rocket_contrib::json::Json;
use serde::Serialize;
use std::fs::File;
use std::io;
use std::mem;
use std::path::{Path, PathBuf};
use std::u32;

static STATIC_TEXT_DEMO_PATH: &'static str = "../client/text-demo.html";
static STATIC_SVG_DEMO_PATH: &'static str = "../client/svg-demo.html";
static STATIC_3D_DEMO_PATH: &'static str = "../client/3d-demo.html";
static STATIC_CSS_BOOTSTRAP_PATH: &'static str = "../client/node_modules/bootstrap/dist/css";
static STATIC_CSS_PATHFINDER_PATH: &'static str = "../client/css/pathfinder.css";
static STATIC_JS_BOOTSTRAP_PATH: &'static str = "../client/node_modules/bootstrap/dist/js";
static STATIC_JS_JQUERY_PATH: &'static str = "../client/node_modules/jquery/dist";
static STATIC_JS_PATHFINDER_PATH: &'static str = "../client";
static STATIC_GLSL_PATH: &'static str = "../../shaders";

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct SubpathRange {
    start: u32,
    end: u32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct IndexRange {
    start: usize,
    end: usize,
}

impl IndexRange {
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

impl PartitionGlyphDimensions {
    fn dummy() -> PartitionGlyphDimensions {
        PartitionGlyphDimensions {
            origin: Point2D::zero(),
            size: Size2D::zero(),
            advance: 0.0,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
struct PartitionGlyphInfo {
    id: u32,
    dimensions: PartitionGlyphDimensions,
    #[serde(rename = "pathIndices")]
    path_indices: PartitionPathIndices,
}

#[derive(Clone, Serialize, Deserialize)]
struct PartitionFontResponse {
    #[serde(rename = "glyphInfo")]
    glyph_info: Vec<PartitionGlyphInfo>,
    #[serde(rename = "pathData")]
    path_data: PartitionEncodedPathData,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
struct PartitionPathIndices {
    #[serde(rename = "bQuadIndices")]
    b_quad_indices: IndexRange,
    #[serde(rename = "bVertexIndices")]
    b_vertex_indices: IndexRange,
    #[serde(rename = "coverInteriorIndices")]
    cover_interior_indices: IndexRange,
    #[serde(rename = "coverCurveIndices")]
    cover_curve_indices: IndexRange,
    #[serde(rename = "coverUpperLineIndices")]
    edge_upper_line_indices: IndexRange,
    #[serde(rename = "coverUpperCurveIndices")]
    edge_upper_curve_indices: IndexRange,
    #[serde(rename = "coverLowerLineIndices")]
    edge_lower_line_indices: IndexRange,
    #[serde(rename = "coverLowerCurveIndices")]
    edge_lower_curve_indices: IndexRange,
}

#[derive(Clone, Serialize, Deserialize)]
struct PartitionEncodedPathData {
    // Base64-encoded `bincode`-encoded `BQuad`s.
    #[serde(rename = "bQuads")]
    b_quads: String,
    // Base64-encoded `bincode`-encoded `Point2D<f32>`s.
    #[serde(rename = "bVertexPositions")]
    b_vertex_positions: String,
    // Base64-encoded `bincode`-encoded `u16`s.
    #[serde(rename = "bVertexPathIDs")]
    b_vertex_path_ids: String,
    // Base64-encoded `bincode`-encoded `BVertexLoopBlinnData`s.
    #[serde(rename = "bVertexLoopBlinnData")]
    b_vertex_loop_blinn_data: String,
    // Base64-encoded `u32`s.
    #[serde(rename = "coverInteriorIndices")]
    cover_interior_indices: String,
    // Base64-encoded `u32`s.
    #[serde(rename = "coverCurveIndices")]
    cover_curve_indices: String,
    // Base64-encoded `bincode`-encoded `LineIndices` instances.
    #[serde(rename = "edgeUpperLineIndices")]
    edge_upper_line_indices: String,
    // Base64-encoded `bincode`-encoded `CurveIndices` instances.
    #[serde(rename = "edgeUpperCurveIndices")]
    edge_upper_curve_indices: String,
    // Base64-encoded `bincode`-encoded `LineIndices` instances.
    #[serde(rename = "edgeLowerLineIndices")]
    edge_lower_line_indices: String,
    // Base64-encoded `bincode`-encoded `CurveIndices` instances.
    #[serde(rename = "edgeLowerCurveIndices")]
    edge_lower_curve_indices: String,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum PartitionFontError {
    Base64DecodingFailed,
    FontSanitizationFailed,
    FontLoadingFailed,
    Unimplemented,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum PartitionSvgPathsError {
    UnknownSvgPathSegmentType,
    Unimplemented,
}

#[derive(Clone, Serialize, Deserialize)]
struct PartitionSvgPathsRequest {
    paths: Vec<PartitionSvgPath>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PartitionSvgPath {
    segments: Vec<PartitionSvgPathSegment>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PartitionSvgPathSegment {
    #[serde(rename = "type")]
    kind: char,
    values: Vec<f64>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PartitionSvgPathsResponse {
    #[serde(rename = "pathIndices")]
    path_indices: Vec<PartitionPathIndices>,
    #[serde(rename = "pathData")]
    path_data: PartitionEncodedPathData,
}

fn partition_paths(partitioner: &mut Partitioner, subpath_indices: &[SubpathRange])
                   -> (PartitionEncodedPathData, Vec<PartitionPathIndices>) {
    let (mut b_quads, mut b_vertex_positions) = (vec![], vec![]);
    let (mut b_vertex_path_ids, mut b_vertex_loop_blinn_data) = (vec![], vec![]);
    let (mut cover_interior_indices, mut cover_curve_indices) = (vec![], vec![]);
    let (mut edge_upper_line_indices, mut edge_upper_curve_indices) = (vec![], vec![]);
    let (mut edge_lower_line_indices, mut edge_lower_curve_indices) = (vec![], vec![]);

    let mut path_indices = vec![];

    for (path_index, subpath_range) in subpath_indices.iter().enumerate() {
        partitioner.partition((path_index + 1) as u16, subpath_range.start, subpath_range.end);

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

        path_indices.push(PartitionPathIndices {
            b_quad_indices: IndexRange::from_data(&mut b_quads, &path_b_quads).unwrap(),
            b_vertex_indices: IndexRange::from_data(&mut b_vertex_loop_blinn_data,
                                                    path_b_vertex_loop_blinn_data).unwrap(),
            cover_interior_indices: IndexRange::from_data(&mut cover_interior_indices,
                                                          &path_cover_interior_indices).unwrap(),
            cover_curve_indices: IndexRange::from_data(&mut cover_curve_indices,
                                                       &path_cover_curve_indices).unwrap(),
            edge_upper_line_indices: IndexRange::from_data(&mut edge_upper_line_indices,
                                                           &path_edge_upper_line_indices).unwrap(),
            edge_upper_curve_indices:
                IndexRange::from_data(&mut edge_upper_curve_indices,
                                      &path_edge_upper_curve_indices).unwrap(),
            edge_lower_line_indices: IndexRange::from_data(&mut edge_lower_line_indices,
                                                           &path_edge_lower_line_indices).unwrap(),
            edge_lower_curve_indices:
                IndexRange::from_data(&mut edge_lower_curve_indices,
                                      &path_edge_lower_curve_indices).unwrap(),
        })
    }

    let encoded_path_data = PartitionEncodedPathData {
        b_quads: base64::encode(&b_quads),
        b_vertex_positions: base64::encode(&b_vertex_positions),
        b_vertex_path_ids: base64::encode(&b_vertex_path_ids),
        b_vertex_loop_blinn_data: base64::encode(&b_vertex_loop_blinn_data),
        cover_interior_indices: base64::encode(&cover_interior_indices),
        cover_curve_indices: base64::encode(&cover_curve_indices),
        edge_upper_line_indices: base64::encode(&edge_upper_line_indices),
        edge_upper_curve_indices: base64::encode(&edge_upper_curve_indices),
        edge_lower_line_indices: base64::encode(&edge_lower_line_indices),
        edge_lower_curve_indices: base64::encode(&edge_lower_curve_indices),
    };

    (encoded_path_data, path_indices)
}

#[post("/partition-font", format = "application/json", data = "<request>")]
fn partition_font(request: Json<PartitionFontRequest>)
                  -> Json<Result<PartitionFontResponse, PartitionFontError>> {
    // Decode Base64-encoded OTF data.
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
    let subpath_indices: Vec<_> = request.glyphs.iter().map(|glyph| {
        let glyph_key = GlyphKey::new(glyph.id);

        let first_subpath_index = outline_buffer.subpaths.len();

        // This might fail; if so, just leave it blank.
        drop(font_context.push_glyph_outline(&font_instance_key,
                                             &glyph_key,
                                             &mut outline_buffer,
                                             &glyph.transform));

        let last_subpath_index = outline_buffer.subpaths.len();

        SubpathRange {
            start: first_subpath_index as u32,
            end: last_subpath_index as u32,
        }
    }).collect();

    // Partition the decoded glyph outlines.
    let mut partitioner = Partitioner::new();
    partitioner.init(&outline_buffer.endpoints,
                     &outline_buffer.control_points,
                     &outline_buffer.subpaths);
    let (encoded_path_data, path_indices) = partition_paths(&mut partitioner, &subpath_indices);

    // Package up other miscellaneous glyph info.
    let mut glyph_info = vec![];
    for (glyph, glyph_path_indices) in request.glyphs.iter().zip(path_indices.iter()) {
        let glyph_key = GlyphKey::new(glyph.id);

        let dimensions = match font_context.glyph_dimensions(&font_instance_key, &glyph_key) {
            Some(dimensions) => {
                PartitionGlyphDimensions {
                    origin: dimensions.origin,
                    size: dimensions.size,
                    advance: dimensions.advance,
                }
            }
            None => PartitionGlyphDimensions::dummy(),
        };

        glyph_info.push(PartitionGlyphInfo {
            id: glyph.id,
            dimensions: dimensions,
            path_indices: *glyph_path_indices,
        })
    }

    // Return the response.
    Json(Ok(PartitionFontResponse {
        glyph_info: glyph_info,
        path_data: encoded_path_data,
    }))
}

#[post("/partition-svg-paths", format = "application/json", data = "<request>")]
fn partition_svg_paths(request: Json<PartitionSvgPathsRequest>)
                       -> Json<Result<PartitionSvgPathsResponse, PartitionSvgPathsError>> {
    // Parse the SVG path.
    //
    // The client has already normalized it, so we only have to handle `M`, `L`, `C`, and `Z`
    // commands.
    let (mut endpoints, mut control_points, mut subpaths) = (vec![], vec![], vec![]);
    let mut paths = vec![];
    for path in &request.paths {
        let first_subpath_index = subpaths.len() as u32;

        let mut first_endpoint_index_in_subpath = endpoints.len();
        for segment in &path.segments {
            match segment.kind {
                'M' => {
                    if first_endpoint_index_in_subpath < endpoints.len() {
                        subpaths.push(Subpath {
                            first_endpoint_index: first_endpoint_index_in_subpath as u32,
                            last_endpoint_index: endpoints.len() as u32,
                        });
                        first_endpoint_index_in_subpath = endpoints.len();
                    }

                    endpoints.push(Endpoint {
                        position: Point2D::new(segment.values[0] as f32, segment.values[1] as f32),
                        control_point_index: u32::MAX,
                        subpath_index: subpaths.len() as u32,
                    })
                }
                'L' => {
                    endpoints.push(Endpoint {
                        position: Point2D::new(segment.values[0] as f32, segment.values[1] as f32),
                        control_point_index: u32::MAX,
                        subpath_index: subpaths.len() as u32,
                    })
                }
                'C' => {
                    // FIXME(pcwalton): Do real cubic-to-quadratic conversion.
                    let control_point_0 = Point2D::new(segment.values[0] as f32,
                                                       segment.values[1] as f32);
                    let control_point_1 = Point2D::new(segment.values[2] as f32,
                                                       segment.values[3] as f32);
                    let control_point = control_point_0.lerp(control_point_1, 0.5);
                    endpoints.push(Endpoint {
                        position: Point2D::new(segment.values[4] as f32, segment.values[5] as f32),
                        control_point_index: control_points.len() as u32,
                        subpath_index: subpaths.len() as u32,
                    });
                    control_points.push(control_point);
                }
                'Z' => {
                    subpaths.push(Subpath {
                        first_endpoint_index: first_endpoint_index_in_subpath as u32,
                        last_endpoint_index: endpoints.len() as u32,
                    });
                    first_endpoint_index_in_subpath = endpoints.len();
                }
                _ => return Json(Err(PartitionSvgPathsError::UnknownSvgPathSegmentType)),
            }
        }

        let last_subpath_index = subpaths.len() as u32;
        paths.push(SubpathRange {
            start: first_subpath_index,
            end: last_subpath_index,
        })
    }

    // Partition the paths.
    let mut partitioner = Partitioner::new();
    partitioner.init(&endpoints, &control_points, &subpaths);
    let (encoded_path_data, path_indices) = partition_paths(&mut partitioner, &paths);

    // Return the response.
    Json(Ok(PartitionSvgPathsResponse {
        path_indices: path_indices,
        path_data: encoded_path_data,
    }))
}

// Static files
#[get("/")]
fn static_text_demo() -> io::Result<NamedFile> {
    NamedFile::open(STATIC_TEXT_DEMO_PATH)
}
#[get("/demo/svg")]
fn static_svg_demo() -> io::Result<NamedFile> {
    NamedFile::open(STATIC_SVG_DEMO_PATH)
}
#[get("/demo/3d")]
fn static_3d_demo() -> io::Result<NamedFile> {
    NamedFile::open(STATIC_3D_DEMO_PATH)
}
#[get("/css/bootstrap/<file..>")]
fn static_css_bootstrap(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_CSS_BOOTSTRAP_PATH).join(file)).ok()
}
#[get("/css/pathfinder.css")]
fn static_css_pathfinder_css() -> io::Result<NamedFile> {
    NamedFile::open(STATIC_CSS_PATHFINDER_PATH)
}
#[get("/js/bootstrap/<file..>")]
fn static_js_bootstrap(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_JS_BOOTSTRAP_PATH).join(file)).ok()
}
#[get("/js/jquery/<file..>")]
fn static_js_jquery(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_JS_JQUERY_PATH).join(file)).ok()
}
#[get("/js/pathfinder/<file..>")]
fn static_js_pathfinder(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_JS_PATHFINDER_PATH).join(file)).ok()
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
        partition_svg_paths,
        static_text_demo,
        static_svg_demo,
        static_3d_demo,
        static_css_bootstrap,
        static_css_pathfinder_css,
        static_js_bootstrap,
        static_js_jquery,
        static_js_pathfinder,
        static_glsl,
    ]).launch();
}
