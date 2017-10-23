// pathfinder/demo/server/main.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.// Copyright © 2017 Mozilla Foundation

#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate app_units;
extern crate base64;
extern crate env_logger;
extern crate euclid;
extern crate fontsan;
extern crate lru_cache;
extern crate pathfinder_font_renderer;
extern crate pathfinder_partitioner;
extern crate pathfinder_path_utils;
extern crate rocket;
extern crate rocket_contrib;

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_derive;

use app_units::Au;
use euclid::{Point2D, Transform2D};
use lru_cache::LruCache;
use pathfinder_font_renderer::{FontContext, FontInstanceKey, FontKey, GlyphKey};
use pathfinder_partitioner::mesh_library::MeshLibrary;
use pathfinder_partitioner::partitioner::Partitioner;
use pathfinder_path_utils::cubic::CubicCurve;
use pathfinder_path_utils::monotonic::MonotonicPathCommandStream;
use pathfinder_path_utils::stroke::Stroke;
use pathfinder_path_utils::{PathBuffer, PathBufferStream, PathCommand, Transform2DPathStream};
use rocket::http::{ContentType, Header, Status};
use rocket::request::Request;
use rocket::response::{NamedFile, Redirect, Responder, Response};
use rocket_contrib::json::Json;
use std::fs::File;
use std::io::{self, Cursor, Read};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::u32;

const CUBIC_ERROR_TOLERANCE: f32 = 0.1;

const MESH_LIBRARY_CACHE_SIZE: usize = 16;

lazy_static! {
    static ref MESH_LIBRARY_CACHE: Mutex<LruCache<MeshLibraryCacheKey, PartitionResponder>> = {
        Mutex::new(LruCache::new(MESH_LIBRARY_CACHE_SIZE))
    };
}

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
static STATIC_WOFF2_INTER_UI_PATH: &'static str = "../../resources/fonts/inter-ui";
static STATIC_GLSL_PATH: &'static str = "../../shaders";
static STATIC_DATA_PATH: &'static str = "../../resources/data";

static STATIC_DOC_API_INDEX_URI: &'static str = "/doc/api/pathfinder_font_renderer/index.html";

static BUILTIN_FONTS: [(&'static str, &'static str); 4] = [
    ("open-sans", "../../resources/fonts/open-sans/OpenSans-Regular.ttf"),
    ("nimbus-sans", "../../resources/fonts/nimbus-sans/NimbusSanL-Regu.ttf"),
    ("eb-garamond", "../../resources/fonts/eb-garamond/EBGaramond12-Regular.ttf"),
    ("inter-ui", "../../resources/fonts/inter-ui/Inter-UI-Regular.ttf"),
];

static BUILTIN_SVGS: [(&'static str, &'static str); 1] = [
    ("tiger", "../../resources/svg/Ghostscript_Tiger.svg"),
];

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct MeshLibraryCacheKey {
    builtin_font_name: String,
    glyph_ids: Vec<u32>,
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

#[derive(Clone, Serialize, Deserialize)]
struct PartitionFontResponse {
    #[serde(rename = "pathData")]
    path_data: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum PartitionFontError {
    UnknownBuiltinFont,
    Base64DecodingFailed,
    FontSanitizationFailed,
    FontLoadingFailed,
    Unimplemented,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum PartitionSvgPathsError {
    UnknownSvgPathCommandType,
    Unimplemented,
}

#[derive(Clone, Serialize, Deserialize)]
struct PartitionSvgPathsRequest {
    paths: Vec<PartitionSvgPath>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PartitionSvgPath {
    segments: Vec<PartitionSvgPathCommand>,
    kind: PartitionSvgPathKind,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum PartitionSvgPathKind {
    Fill,
    Stroke(f32),
}

#[derive(Clone, Serialize, Deserialize)]
struct PartitionSvgPathCommand {
    #[serde(rename = "type")]
    kind: char,
    values: Vec<f64>,
}

struct PathPartitioningResult {
    encoded_data: Arc<Vec<u8>>,
    time: Duration,
}

impl PathPartitioningResult {
    fn compute(partitioner: &mut Partitioner, subpath_indices: &[Range<u32>])
               -> PathPartitioningResult {
        let timestamp_before = Instant::now();

        for (path_index, subpath_range) in subpath_indices.iter().enumerate() {
            partitioner.partition((path_index + 1) as u16, subpath_range.start, subpath_range.end);
        }

        partitioner.library_mut().optimize();

        let time_elapsed = timestamp_before.elapsed();

        let mut data_buffer = Cursor::new(vec![]);
        drop(partitioner.library().serialize_into(&mut data_buffer));

        PathPartitioningResult {
            encoded_data: Arc::new(data_buffer.into_inner()),
            time: time_elapsed,
        }
    }

    fn elapsed_ms(&self) -> f64 {
        self.time.as_secs() as f64 * 1000.0 + self.time.subsec_nanos() as f64 * 1e-6
    }
}

#[derive(Clone)]
struct PartitionResponder {
    data: Arc<Vec<u8>>,
    time: f64,
}

impl<'r> Responder<'r> for PartitionResponder {
    fn respond_to(self, _: &Request) -> Result<Response<'r>, Status> {
        let mut builder = Response::build();
        builder.header(ContentType::new("application", "vnd.mozilla.pfml"));
        builder.header(Header::new("Server-Timing", format!("Partitioning={}", self.time)));

        // FIXME(pcwalton): Don't clone! Requires a `Cursor` implementation for `Arc<Vec<u8>>`…
        builder.sized_body(Cursor::new((*self.data).clone()));
        builder.ok()
    }
}

#[post("/partition-font", format = "application/json", data = "<request>")]
fn partition_font(request: Json<PartitionFontRequest>)
                  -> Result<PartitionResponder, PartitionFontError> {
    let cache_key = match request.face {
        PartitionFontRequestFace::Builtin(ref builtin_font_name) => {
            Some(MeshLibraryCacheKey {
                builtin_font_name: (*builtin_font_name).clone(),
                glyph_ids: request.glyphs.iter().map(|glyph| glyph.id).collect(),
            })
        }
        _ => None,
    };

    if let Some(ref cache_key) = cache_key {
        if let Ok(mut mesh_library_cache) = MESH_LIBRARY_CACHE.lock() {
            if let Some(cache_entry) = mesh_library_cache.get_mut(cache_key) {
                return Ok((*cache_entry).clone())
            }
        }
    }

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
                    Arc::new(data)
                }
                None => return Err(PartitionFontError::UnknownBuiltinFont),
            }
        }
        PartitionFontRequestFace::Custom(ref encoded_data) => {
            // Decode Base64-encoded OTF data.
            let unsafe_otf_data = match base64::decode(encoded_data) {
                Ok(unsafe_otf_data) => unsafe_otf_data,
                Err(_) => return Err(PartitionFontError::Base64DecodingFailed),
            };

            // Sanitize.
            match fontsan::process(&unsafe_otf_data) {
                Ok(otf_data) => Arc::new(otf_data),
                Err(_) => return Err(PartitionFontError::FontSanitizationFailed),
            }
        }
    };

    // Parse glyph data.
    let font_key = FontKey::new();
    let font_instance_key = FontInstanceKey {
        font_key: font_key,
        size: Au::from_f64_px(request.point_size),
    };
    let mut font_context = match FontContext::new() {
        Ok(font_context) => font_context,
        Err(_) => {
            println!("Failed to create a font context!");
            return Err(PartitionFontError::FontLoadingFailed)
        }
    };
    if font_context.add_font_from_memory(&font_key, otf_data, request.font_index).is_err() {
        println!("Failed to add font from memory!");
        return Err(PartitionFontError::FontLoadingFailed)
    }

    // Read glyph info.
    let mut path_buffer = PathBuffer::new();
    let subpath_indices: Vec<_> = request.glyphs.iter().map(|glyph| {
        let glyph_key = GlyphKey::new(glyph.id);

        let first_subpath_index = path_buffer.subpaths.len();

        // This might fail; if so, just leave it blank.
        if let Ok(glyph_outline) = font_context.glyph_outline(&font_instance_key, &glyph_key) {
            let stream = Transform2DPathStream::new(glyph_outline.into_iter(), &glyph.transform);
            let stream = MonotonicPathCommandStream::new(stream);
            path_buffer.add_stream(stream)
        }

        let last_subpath_index = path_buffer.subpaths.len();

        (first_subpath_index as u32)..(last_subpath_index as u32)
    }).collect();

    // Partition the decoded glyph outlines.
    let mut library = MeshLibrary::new();
    for (path_index, subpath_range) in subpath_indices.iter().enumerate() {
        let stream = PathBufferStream::subpath_range(&path_buffer, (*subpath_range).clone());
        library.push_segments((path_index + 1) as u16, stream);
        let stream = PathBufferStream::subpath_range(&path_buffer, (*subpath_range).clone());
        library.push_normals(stream);
    }

    let mut partitioner = Partitioner::new(library);
    partitioner.init_with_path_buffer(&path_buffer);
    let path_partitioning_result = PathPartitioningResult::compute(&mut partitioner,
                                                                   &subpath_indices);

    // Build the response.
    let elapsed_ms = path_partitioning_result.elapsed_ms();
    let responder = PartitionResponder {
        data: path_partitioning_result.encoded_data,
        time: elapsed_ms,
    };

    if let Some(cache_key) = cache_key {
        if let Ok(mut mesh_library_cache) = MESH_LIBRARY_CACHE.lock() {
            mesh_library_cache.insert(cache_key, responder.clone());
        }
    }

    Ok(responder)
}

#[post("/partition-svg-paths", format = "application/json", data = "<request>")]
fn partition_svg_paths(request: Json<PartitionSvgPathsRequest>)
                       -> Result<PartitionResponder, PartitionSvgPathsError> {
    // Parse the SVG path.
    //
    // The client has already normalized it, so we only have to handle `M`, `L`, `C`, and `Z`
    // commands.
    let mut path_buffer = PathBuffer::new();
    let mut paths = vec![];
    let mut last_point = Point2D::zero();

    for path in &request.paths {
        let mut stream = vec![];

        let first_subpath_index = path_buffer.subpaths.len() as u32;

        for segment in &path.segments {
            match segment.kind {
                'M' => {
                    last_point = Point2D::new(segment.values[0] as f32, segment.values[1] as f32);
                    stream.push(PathCommand::MoveTo(last_point))
                }
                'L' => {
                    last_point = Point2D::new(segment.values[0] as f32, segment.values[1] as f32);
                    stream.push(PathCommand::LineTo(last_point))
                }
                'C' => {
                    let control_point_0 = Point2D::new(segment.values[0] as f32,
                                                       segment.values[1] as f32);
                    let control_point_1 = Point2D::new(segment.values[2] as f32,
                                                       segment.values[3] as f32);
                    let endpoint_1 = Point2D::new(segment.values[4] as f32,
                                                  segment.values[5] as f32);
                    let cubic = CubicCurve::new(&last_point,
                                                &control_point_0,
                                                &control_point_1,
                                                &endpoint_1);
                    last_point = endpoint_1;
                    stream.extend(cubic.approx_curve(CUBIC_ERROR_TOLERANCE)
                                       .map(|curve| curve.to_path_segment()));
                }
                'Z' => stream.push(PathCommand::ClosePath),
                _ => return Err(PartitionSvgPathsError::UnknownSvgPathCommandType),
            }
        }

        match path.kind {
            PartitionSvgPathKind::Fill => {
                path_buffer.add_stream(MonotonicPathCommandStream::new(stream.into_iter()))
            }
            PartitionSvgPathKind::Stroke(stroke_width) => {
                let mut temp_path_buffer = PathBuffer::new();
                Stroke::new(stroke_width).apply(&mut temp_path_buffer, stream.into_iter());

                let stream = PathBufferStream::new(&temp_path_buffer);
                let stream = MonotonicPathCommandStream::new(stream);
                path_buffer.add_stream(stream)
            }
        }

        let last_subpath_index = path_buffer.subpaths.len() as u32;

        paths.push(first_subpath_index..last_subpath_index)
    }

    // Partition the paths.
    let mut partitioner = Partitioner::new(MeshLibrary::new());
    partitioner.init_with_path_buffer(&path_buffer);
    let path_partitioning_result = PathPartitioningResult::compute(&mut partitioner, &paths);

    // Return the response.
    let elapsed_ms = path_partitioning_result.elapsed_ms();
    Ok(PartitionResponder {
        data: path_partitioning_result.encoded_data,
        time: elapsed_ms,
    })
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
#[get("/woff2/inter-ui/<file..>")]
fn static_woff2_inter_ui(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_WOFF2_INTER_UI_PATH).join(file)).ok()
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
        static_woff2_inter_ui,
        static_glsl,
        static_otf_demo,
        static_svg_demo,
        static_data,
    ]).launch();
}
