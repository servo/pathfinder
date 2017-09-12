// pathfinder/demo/server/main.rs
//
// Copyright © 2017 Mozilla Foundation

#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate app_units;
extern crate base64;
extern crate bincode;
extern crate env_logger;
extern crate euclid;
extern crate fontsan;
extern crate pathfinder_font_renderer;
extern crate pathfinder_partitioner;
extern crate pathfinder_path_utils;
extern crate rocket;
extern crate rocket_contrib;
extern crate serde;

#[macro_use]
extern crate serde_derive;

use app_units::Au;
use bincode::Infinite;
use euclid::{Point2D, Size2D, Transform2D};
use pathfinder_font_renderer::{FontContext, FontInstanceKey, FontKey, GlyphKey};
use pathfinder_partitioner::partitioner::Partitioner;
use pathfinder_path_utils::monotonic::MonotonicPathSegmentStream;
use pathfinder_path_utils::stroke;
use pathfinder_path_utils::{PathBuffer, PathBufferStream, PathSegment, Transform2DPathStream};
use rocket::http::{ContentType, Status};
use rocket::request::Request;
use rocket::response::{NamedFile, Redirect, Responder, Response};
use rocket_contrib::json::Json;
use serde::Serialize;
use std::fs::File;
use std::io::{self, Read};
use std::mem;
use std::path::{Path, PathBuf};
use std::u32;

static STATIC_INDEX_PATH: &'static str = "../client/index.html";
static STATIC_TEXT_DEMO_PATH: &'static str = "../client/text-demo.html";
static STATIC_SVG_DEMO_PATH: &'static str = "../client/svg-demo.html";
static STATIC_3D_DEMO_PATH: &'static str = "../client/3d-demo.html";
static STATIC_TOOLS_BENCHMARK_PATH: &'static str = "../client/benchmark.html";
static STATIC_TOOLS_MESH_DEBUGGER_PATH: &'static str = "../client/mesh-debugger.html";
static STATIC_DOC_API_PATH: &'static str = "../../font-renderer/target/doc";
static STATIC_CSS_BOOTSTRAP_PATH: &'static str = "../client/node_modules/bootstrap/dist/css";
static STATIC_CSS_OCTICONS_PATH: &'static str = "../client/node_modules/octicons/build";
static STATIC_CSS_PATHFINDER_PATH: &'static str = "../client/css/pathfinder.css";
static STATIC_JS_BOOTSTRAP_PATH: &'static str = "../client/node_modules/bootstrap/dist/js";
static STATIC_JS_JQUERY_PATH: &'static str = "../client/node_modules/jquery/dist";
static STATIC_JS_POPPER_JS_PATH: &'static str = "../client/node_modules/popper.js/dist/umd";
static STATIC_JS_PATHFINDER_PATH: &'static str = "../client";
static STATIC_SVG_OCTICONS_PATH: &'static str = "../client/node_modules/octicons/build/svg";
static STATIC_GLSL_PATH: &'static str = "../../shaders";
static STATIC_DATA_PATH: &'static str = "../../resources/data";

static STATIC_DOC_API_INDEX_URI: &'static str = "/doc/api/pathfinder_font_renderer/index.html";

static BUILTIN_FONTS: [(&'static str, &'static str); 3] = [
    ("open-sans", "../../resources/fonts/open-sans/OpenSans-Regular.ttf"),
    ("nimbus-sans", "../../resources/fonts/nimbus-sans/NimbusSanL-Regu.ttf"),
    ("eb-garamond", "../../resources/fonts/eb-garamond/EBGaramond12-Regular.ttf"),
];

static BUILTIN_SVGS: [(&'static str, &'static str); 1] = [
    ("tiger", "../../resources/svg/Ghostscript_Tiger.svg"),
];

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

#[derive(Clone, Serialize, Deserialize)]
struct PartitionFontRequest {
    face: PartitionFontRequestFace,
    #[serde(rename = "fontIndex")]
    font_index: u32,
    glyphs: Vec<PartitionGlyph>,
    #[serde(rename = "pointSize")]
    point_size: f64,
}

#[derive(Clone, Serialize, Deserialize)]
enum PartitionFontRequestFace {
    /// One of the builtin fonts in `BUILTIN_FONTS`.
    Builtin(String),
    /// Base64-encoded OTF data.
    Custom(String),
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
    UnknownBuiltinFont,
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
    kind: PartitionSvgPathKind,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum PartitionSvgPathKind {
    Fill,
    Stroke(f32),
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

    // Reverse interior indices for early Z optimizations.
    let mut new_cover_interior_indices = Vec::with_capacity(cover_interior_indices.len());
    for path_indices in &mut path_indices {
        let old_byte_start = path_indices.cover_interior_indices.start * mem::size_of::<u32>();
        let old_byte_end = path_indices.cover_interior_indices.end * mem::size_of::<u32>();
        let new_start_index = new_cover_interior_indices.len() / mem::size_of::<u32>();
        new_cover_interior_indices.extend(
            cover_interior_indices[old_byte_start..old_byte_end].into_iter());
        let new_end_index = new_cover_interior_indices.len() / mem::size_of::<u32>();
        path_indices.cover_interior_indices.start = new_start_index;
        path_indices.cover_interior_indices.end = new_end_index;
    }
    cover_interior_indices = new_cover_interior_indices;

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
    // Fetch the OTF data.
    let otf_data = match request.face {
        PartitionFontRequestFace::Builtin(ref builtin_font_name) => {
            // Read in the builtin font.
            match BUILTIN_FONTS.iter().filter(|& &(name, _)| name == builtin_font_name).next() {
                Some(&(_, path)) => {
                    let mut data = vec![];
                    File::open(path).expect("Couldn't find builtin font!")
                                    .read_to_end(&mut data)
                                    .expect("Couldn't read builtin font!");
                    data
                }
                None => return Json(Err(PartitionFontError::UnknownBuiltinFont)),
            }
        }
        PartitionFontRequestFace::Custom(ref encoded_data) => {
            // Decode Base64-encoded OTF data.
            let unsafe_otf_data = match base64::decode(encoded_data) {
                Ok(unsafe_otf_data) => unsafe_otf_data,
                Err(_) => return Json(Err(PartitionFontError::Base64DecodingFailed)),
            };

            // Sanitize.
            match fontsan::process(&unsafe_otf_data) {
                Ok(otf_data) => otf_data,
                Err(_) => return Json(Err(PartitionFontError::FontSanitizationFailed)),
            }
        }
    };

    // Parse glyph data.
    let font_key = FontKey::new();
    let font_instance_key = FontInstanceKey {
        font_key: font_key,
        size: Au::from_f64_px(request.point_size),
    };
    let mut font_context = FontContext::new();
    if font_context.add_font_from_memory(&font_key, otf_data, request.font_index).is_err() {
        return Json(Err(PartitionFontError::FontLoadingFailed))
    }

    // Read glyph info.
    let mut path_buffer = PathBuffer::new();
    let subpath_indices: Vec<_> = request.glyphs.iter().map(|glyph| {
        let glyph_key = GlyphKey::new(glyph.id);

        let first_subpath_index = path_buffer.subpaths.len();

        // This might fail; if so, just leave it blank.
        if let Ok(glyph_outline) = font_context.glyph_outline(&font_instance_key, &glyph_key) {
            let stream = Transform2DPathStream::new(glyph_outline, &glyph.transform);
            let stream = MonotonicPathSegmentStream::new(stream);
            path_buffer.add_stream(stream)
        }

        let last_subpath_index = path_buffer.subpaths.len();

        SubpathRange {
            start: first_subpath_index as u32,
            end: last_subpath_index as u32,
        }
    }).collect();

    // Partition the decoded glyph outlines.
    let mut partitioner = Partitioner::new();
    partitioner.init_with_path_buffer(&path_buffer);
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
    let mut path_buffer = PathBuffer::new();
    let mut paths = vec![];
    for path in &request.paths {
        let mut stream = vec![];

        let first_subpath_index = path_buffer.subpaths.len() as u32;

        for segment in &path.segments {
            match segment.kind {
                'M' => {
                    stream.push(PathSegment::MoveTo(Point2D::new(segment.values[0] as f32,
                                                                 segment.values[1] as f32)))
                }
                'L' => {
                    stream.push(PathSegment::LineTo(Point2D::new(segment.values[0] as f32,
                                                                 segment.values[1] as f32)))
                }
                'C' => {
                    // FIXME(pcwalton): Do real cubic-to-quadratic conversion.
                    let control_point_0 = Point2D::new(segment.values[0] as f32,
                                                       segment.values[1] as f32);
                    let control_point_1 = Point2D::new(segment.values[2] as f32,
                                                       segment.values[3] as f32);
                    let control_point = control_point_0.lerp(control_point_1, 0.5);
                    stream.push(PathSegment::CurveTo(control_point,
                                                     Point2D::new(segment.values[4] as f32,
                                                                  segment.values[5] as f32)))
                }
                'Z' => stream.push(PathSegment::ClosePath),
                _ => return Json(Err(PartitionSvgPathsError::UnknownSvgPathSegmentType)),
            }
        }

        match path.kind {
            PartitionSvgPathKind::Fill => {
                path_buffer.add_stream(MonotonicPathSegmentStream::new(stream.into_iter()))
            }
            PartitionSvgPathKind::Stroke(stroke_width) => {
                let mut temp_path_buffer = PathBuffer::new();
                stroke::stroke(&mut temp_path_buffer, stream.into_iter(), stroke_width);
                let stream = PathBufferStream::new(&temp_path_buffer);
                let stream = MonotonicPathSegmentStream::new(stream);
                path_buffer.add_stream(stream)
            }
        }

        let last_subpath_index = path_buffer.subpaths.len() as u32;

        paths.push(SubpathRange {
            start: first_subpath_index,
            end: last_subpath_index,
        })
    }

    // Partition the paths.
    let mut partitioner = Partitioner::new();
    partitioner.init_with_path_buffer(&path_buffer);
    let (encoded_path_data, path_indices) = partition_paths(&mut partitioner, &paths);

    // Return the response.
    Json(Ok(PartitionSvgPathsResponse {
        path_indices: path_indices,
        path_data: encoded_path_data,
    }))
}

// Static files
#[get("/")]
fn static_index() -> io::Result<NamedFile> {
    NamedFile::open(STATIC_INDEX_PATH)
}
#[get("/demo/text")]
fn static_demo_text() -> io::Result<NamedFile> {
    NamedFile::open(STATIC_TEXT_DEMO_PATH)
}
#[get("/demo/svg")]
fn static_demo_svg() -> io::Result<NamedFile> {
    NamedFile::open(STATIC_SVG_DEMO_PATH)
}
#[get("/demo/3d")]
fn static_demo_3d() -> io::Result<NamedFile> {
    NamedFile::open(STATIC_3D_DEMO_PATH)
}
#[get("/tools/benchmark")]
fn static_tools_benchmark() -> io::Result<NamedFile> {
    NamedFile::open(STATIC_TOOLS_BENCHMARK_PATH)
}
#[get("/tools/mesh-debugger")]
fn static_tools_mesh_debugger() -> io::Result<NamedFile> {
    NamedFile::open(STATIC_TOOLS_MESH_DEBUGGER_PATH)
}
#[get("/doc/api")]
fn static_doc_api_index() -> Redirect {
    Redirect::to(STATIC_DOC_API_INDEX_URI)
}
#[get("/doc/api/<file..>")]
fn static_doc_api(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_DOC_API_PATH).join(file)).ok()
}
#[get("/css/bootstrap/<file..>")]
fn static_css_bootstrap(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_CSS_BOOTSTRAP_PATH).join(file)).ok()
}
#[get("/css/octicons/<file..>")]
fn static_css_octicons(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_CSS_OCTICONS_PATH).join(file)).ok()
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
#[get("/js/popper.js/<file..>")]
fn static_js_popper_js(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_JS_POPPER_JS_PATH).join(file)).ok()
}
#[get("/js/pathfinder/<file..>")]
fn static_js_pathfinder(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_JS_PATHFINDER_PATH).join(file)).ok()
}
#[get("/svg/octicons/<file..>")]
fn static_svg_octicons(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_SVG_OCTICONS_PATH).join(file)).ok()
}
#[get("/glsl/<file..>")]
fn static_glsl(file: PathBuf) -> Option<Shader> {
    Shader::open(Path::new(STATIC_GLSL_PATH).join(file)).ok()
}
#[get("/otf/demo/<font_name>")]
fn static_otf_demo(font_name: String) -> Option<NamedFile> {
    BUILTIN_FONTS.iter()
                 .filter(|& &(name, _)| name == font_name)
                 .next()
                 .and_then(|&(_, path)| NamedFile::open(Path::new(path)).ok())
}
#[get("/svg/demo/<svg_name>")]
fn static_svg_demo(svg_name: String) -> Option<NamedFile> {
    BUILTIN_SVGS.iter()
                .filter(|& &(name, _)| name == svg_name)
                .next()
                .and_then(|&(_, path)| NamedFile::open(Path::new(path)).ok())
}
#[get("/data/<file..>")]
fn static_data(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_DATA_PATH).join(file)).ok()
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
    drop(env_logger::init());

    rocket::ignite().mount("/", routes![
        partition_font,
        partition_svg_paths,
        static_index,
        static_demo_text,
        static_demo_svg,
        static_demo_3d,
        static_tools_benchmark,
        static_tools_mesh_debugger,
        static_doc_api_index,
        static_doc_api,
        static_css_bootstrap,
        static_css_octicons,
        static_css_pathfinder_css,
        static_js_bootstrap,
        static_js_jquery,
        static_js_popper_js,
        static_js_pathfinder,
        static_svg_octicons,
        static_glsl,
        static_otf_demo,
        static_svg_demo,
        static_data,
    ]).launch();
}
