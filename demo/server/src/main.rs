// pathfinder/demo/server/main.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(decl_macro, plugin)]
#![plugin(rocket_codegen)]

extern crate app_units;
extern crate base64;
extern crate env_logger;
extern crate euclid;
extern crate fontsan;
extern crate image;
extern crate lru_cache;
extern crate lyon_geom;
extern crate lyon_path;
extern crate pathfinder_font_renderer;
extern crate pathfinder_partitioner;
extern crate pathfinder_path_utils;
extern crate rocket;
extern crate rocket_contrib;

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_derive;

#[cfg(feature = "reftests")]
extern crate cairo;
#[cfg(feature = "reftests")]
extern crate rsvg;

use app_units::Au;
use euclid::{Point2D, Transform2D};
use image::{DynamicImage, ImageBuffer, ImageFormat, ImageRgba8};
use lru_cache::LruCache;
use lyon_path::PathEvent;
use lyon_path::builder::{FlatPathBuilder, PathBuilder};
use lyon_path::iterator::PathIter;
use pathfinder_font_renderer::{FontContext, FontInstance, FontKey, GlyphImage};
use pathfinder_font_renderer::{GlyphKey, SubpixelOffset};
use pathfinder_partitioner::FillRule;
use pathfinder_partitioner::mesh_library::MeshLibrary;
use pathfinder_partitioner::partitioner::Partitioner;
use pathfinder_path_utils::stroke::{StrokeStyle, StrokeToFillIter};
use pathfinder_path_utils::transform::Transform2DPathIter;
use rocket::http::{ContentType, Header, Status};
use rocket::request::Request;
use rocket::response::{NamedFile, Redirect, Responder, Response};
use rocket_contrib::json::Json;
use std::fs::File;
use std::io::{self, Cursor, Read};
use std::path::{self, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::u32;

#[cfg(target_os = "macos")]
use pathfinder_font_renderer::core_graphics;

#[cfg(feature = "reftests")]
use euclid::Size2D;
#[cfg(feature = "reftests")]
use cairo::{Format, ImageSurface};
#[cfg(feature = "reftests")]
use rsvg::{Handle, HandleExt};

const SUGGESTED_JSON_SIZE_LIMIT: u64 = 32 * 1024 * 1024;

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
static STATIC_TOOLS_REFERENCE_TEST_PATH: &'static str = "../client/reference-test.html";
static STATIC_TOOLS_MESH_DEBUGGER_PATH: &'static str = "../client/mesh-debugger.html";
static STATIC_DOC_API_PATH: &'static str = "../../target/doc";
static STATIC_CSS_BOOTSTRAP_PATH: &'static str = "../client/node_modules/bootstrap/dist/css";
static STATIC_CSS_PATH: &'static str = "../client/css";
static STATIC_JS_BOOTSTRAP_PATH: &'static str = "../client/node_modules/bootstrap/dist/js";
static STATIC_JS_JQUERY_PATH: &'static str = "../client/node_modules/jquery/dist";
static STATIC_JS_POPPER_JS_PATH: &'static str = "../client/node_modules/popper.js/dist/umd";
static STATIC_JS_PATHFINDER_PATH: &'static str = "../client";
static STATIC_WOFF2_INTER_UI_PATH: &'static str = "../../resources/fonts/inter-ui";
static STATIC_WOFF2_MATERIAL_ICONS_PATH: &'static str = "../../resources/fonts/material-icons";
static STATIC_GLSL_PATH: &'static str = "../../shaders";
static STATIC_DATA_PATH: &'static str = "../../resources/data";
static STATIC_TEST_DATA_PATH: &'static str = "../../resources/tests";
static STATIC_TEXTURES_PATH: &'static str = "../../resources/textures";

static STATIC_DOC_API_INDEX_URI: &'static str = "/doc/api/pathfinder/index.html";

static BUILTIN_FONTS: [(&'static str, &'static str); 4] = [
    ("open-sans", "../../resources/fonts/open-sans/OpenSans-Regular.ttf"),
    ("nimbus-sans", "../../resources/fonts/nimbus-sans/NimbusSanL-Regu.ttf"),
    ("eb-garamond", "../../resources/fonts/eb-garamond/EBGaramond12-Regular.ttf"),
    ("inter-ui", "../../resources/fonts/inter-ui/Inter-UI-Regular.ttf"),
];

static BUILTIN_SVGS: [(&'static str, &'static str); 4] = [
    ("tiger", "../../resources/svg/Ghostscript_Tiger.svg"),
    ("logo", "../../resources/svg/pathfinder_logo.svg"),
    ("icons", "../../resources/svg/material_design_icons.svg"),
    ("logo-bw", "../../resources/svg/pathfinder_logo_bw.svg"),
];

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct MeshLibraryCacheKey {
    builtin_font_name: String,
    glyph_ids: Vec<u32>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PartitionFontRequest {
    face: FontRequestFace,
    #[serde(rename = "fontIndex")]
    font_index: u32,
    glyphs: Vec<PartitionGlyph>,
    #[serde(rename = "pointSize")]
    point_size: f64,
}

#[derive(Clone, Serialize, Deserialize)]
enum FontRequestFace {
    /// One of the builtin fonts in `BUILTIN_FONTS`.
    Builtin(String),
    /// Base64-encoded OTF data.
    Custom(String),
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum ReferenceTextRenderer {
    #[serde(rename = "freetype")]
    FreeType,
    #[serde(rename = "core-graphics")]
    CoreGraphics,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum ReferenceSvgRenderer {
    #[serde(rename = "pixman")]
    Pixman,
}

#[derive(Clone, Serialize, Deserialize)]
struct RenderTextReferenceRequest {
    face: FontRequestFace,
    #[serde(rename = "fontIndex")]
    font_index: u32,
    glyph: u32,

    #[serde(rename = "pointSize")]
    point_size: f64,
    renderer: ReferenceTextRenderer,
}

#[derive(Clone, Serialize, Deserialize)]
struct RenderSvgReferenceRequest {
    name: String,
    scale: f64,
    renderer: ReferenceSvgRenderer,
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
enum FontError {
    UnknownBuiltinFont,
    Base64DecodingFailed,
    FontSanitizationFailed,
    FontLoadingFailed,
    RasterizationFailed,
    ReferenceRasterizerUnavailable,
    Unimplemented,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum SvgError {
    ReftestsDisabled,
    UnknownBuiltinSvg,
    LoadingFailed,
    ImageWritingFailed,
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
    Fill(PartitionSvgFillRule),
    Stroke(f32),
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum PartitionSvgFillRule {
    Winding,
    EvenOdd,
}

impl PartitionSvgFillRule {
    fn to_fill_rule(self) -> FillRule {
        match self {
            PartitionSvgFillRule::Winding => FillRule::Winding,
            PartitionSvgFillRule::EvenOdd => FillRule::EvenOdd,
        }
    }
}

#[derive(Clone)]
struct PathDescriptor {
    path_index: usize,
    fill_rule: FillRule,
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
    fn compute(partitioner: &mut Partitioner,
               path_descriptors: &[PathDescriptor],
               paths: &[Vec<PathEvent>])
               -> PathPartitioningResult {
        let timestamp_before = Instant::now();

        for (path, path_descriptor) in paths.iter().zip(path_descriptors.iter()) {
            path.iter().for_each(|event| partitioner.builder_mut().path_event(*event));
            partitioner.partition((path_descriptor.path_index + 1) as u16,
                                  path_descriptor.fill_rule);
            partitioner.builder_mut().build_and_reset();
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

#[derive(Clone)]
struct ReferenceImage {
    image: DynamicImage,
}

impl<'r> Responder<'r> for ReferenceImage {
    fn respond_to(self, _: &Request) -> Result<Response<'r>, Status> {
        let mut builder = Response::build();
        builder.header(ContentType::PNG);

        let mut bytes = vec![];
        try!(self.image
                 .save(&mut bytes, ImageFormat::PNG)
                 .map_err(|_| Status::InternalServerError));
        builder.sized_body(Cursor::new(bytes));
        builder.ok()
    }
}

// Fetches the OTF data.
fn otf_data_from_request(face: &FontRequestFace) -> Result<Arc<Vec<u8>>, FontError> {
    match *face {
        FontRequestFace::Builtin(ref builtin_font_name) => {
            // Read in the builtin font.
            match BUILTIN_FONTS.iter().filter(|& &(name, _)| name == builtin_font_name).next() {
                Some(&(_, path)) => {
                    let mut data = vec![];
                    File::open(path).expect("Couldn't find builtin font!")
                                    .read_to_end(&mut data)
                                    .expect("Couldn't read builtin font!");
                    Ok(Arc::new(data))
                }
                None => return Err(FontError::UnknownBuiltinFont),
            }
        }
        FontRequestFace::Custom(ref encoded_data) => {
            // Decode Base64-encoded OTF data.
            let unsafe_otf_data = match base64::decode(encoded_data) {
                Ok(unsafe_otf_data) => unsafe_otf_data,
                Err(_) => return Err(FontError::Base64DecodingFailed),
            };

            // Sanitize.
            match fontsan::process(&unsafe_otf_data) {
                Ok(otf_data) => Ok(Arc::new(otf_data)),
                Err(_) => return Err(FontError::FontSanitizationFailed),
            }
        }
    }
}

// Fetches the SVG data.
#[cfg(feature = "reftests")]
fn svg_data_from_request(builtin_svg_name: &str) -> Result<Arc<Vec<u8>>, SvgError> {
    // Read in the builtin SVG.
    match BUILTIN_SVGS.iter().filter(|& &(name, _)| name == builtin_svg_name).next() {
        Some(&(_, path)) => {
            let mut data = vec![];
            File::open(path).expect("Couldn't find builtin SVG!")
                            .read_to_end(&mut data)
                            .expect("Couldn't read builtin SVG!");
            Ok(Arc::new(data))
        }
        None => return Err(SvgError::UnknownBuiltinSvg),
    }
}

#[cfg(target_os = "macos")]
fn rasterize_glyph_with_core_graphics(font_key: &FontKey,
                                      font_index: u32,
                                      otf_data: Arc<Vec<u8>>,
                                      font_instance: &FontInstance,
                                      glyph_key: &GlyphKey)
                                      -> Result<GlyphImage, FontError> {
    let mut font_context =
        try!(core_graphics::FontContext::new().map_err(|_| FontError::FontLoadingFailed));
    try!(font_context.add_font_from_memory(font_key, otf_data, font_index)
                     .map_err(|_| FontError::FontLoadingFailed));
    font_context.rasterize_glyph_with_native_rasterizer(&font_instance, &glyph_key, true)
                .map_err(|_| FontError::RasterizationFailed)
}

#[cfg(not(target_os = "macos"))]
fn rasterize_glyph_with_core_graphics(_: &FontKey,
                                      _: u32,
                                      _: Arc<Vec<u8>>,
                                      _: &FontInstance,
                                      _: &GlyphKey)
                                      -> Result<GlyphImage, FontError> {
    Err(FontError::ReferenceRasterizerUnavailable)
}

#[post("/partition-font", format = "application/json", data = "<request>")]
fn partition_font(request: Json<PartitionFontRequest>) -> Result<PartitionResponder, FontError> {
    // Check the cache.
    let cache_key = match request.face {
        FontRequestFace::Builtin(ref builtin_font_name) => {
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

    // Parse glyph data.
    let mut font_context = match FontContext::new() {
        Ok(font_context) => font_context,
        Err(_) => {
            println!("Failed to create a font context!");
            return Err(FontError::FontLoadingFailed)
        }
    };

    let font_key = FontKey::new();
    let otf_data = try!(otf_data_from_request(&request.face));
    if font_context.add_font_from_memory(&font_key, otf_data, request.font_index).is_err() {
        return Err(FontError::FontLoadingFailed)
    }

    let font_instance = FontInstance {
        font_key: font_key,
        size: Au::from_f64_px(request.point_size),
    };

    // Read glyph info.
    let mut paths: Vec<Vec<PathEvent>> = vec![];
    let mut path_descriptors = vec![];

    for (glyph_index, glyph) in request.glyphs.iter().enumerate() {
        let glyph_key = GlyphKey::new(glyph.id, SubpixelOffset(0));

        // This might fail; if so, just leave it blank.
        match font_context.glyph_outline(&font_instance, &glyph_key) {
            Ok(glyph_outline) => {
                paths.push(Transform2DPathIter::new(glyph_outline, &glyph.transform).collect())
            }
            Err(_) => continue,
        };

        path_descriptors.push(PathDescriptor {
            path_index: glyph_index,
            fill_rule: FillRule::Winding,
        })
    }

    // Partition the decoded glyph outlines.
    let mut library = MeshLibrary::new();
    for (stored_path_index, path_descriptor) in path_descriptors.iter().enumerate() {
        library.push_segments((path_descriptor.path_index + 1) as u16,
                              PathIter::new(paths[stored_path_index].iter().cloned()));
        library.push_normals((path_descriptor.path_index + 1) as u16,
                             PathIter::new(paths[stored_path_index].iter().cloned()));
    }

    let mut partitioner = Partitioner::new(library);
    let path_partitioning_result = PathPartitioningResult::compute(&mut partitioner,
                                                                   &path_descriptors,
                                                                   &paths);

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
    let mut paths = vec![];
    let mut path_descriptors = vec![];
    let mut partitioner = Partitioner::new(MeshLibrary::new());
    let mut path_index = 0;

    for path in &request.paths {
        let mut stream = vec![];

        for segment in &path.segments {
            match segment.kind {
                'M' => {
                    stream.push(PathEvent::MoveTo(Point2D::new(segment.values[0] as f32,
                                                               segment.values[1] as f32)))
                }
                'L' => {
                    stream.push(PathEvent::LineTo(Point2D::new(segment.values[0] as f32,
                                                               segment.values[1] as f32)))
                }
                'C' => {
                    stream.push(PathEvent::CubicTo(Point2D::new(segment.values[0] as f32,
                                                                segment.values[1] as f32),
                                                   Point2D::new(segment.values[2] as f32,
                                                                segment.values[3] as f32),
                                                   Point2D::new(segment.values[4] as f32,
                                                                segment.values[5] as f32)))
                }
                'Z' => stream.push(PathEvent::Close),
                _ => return Err(PartitionSvgPathsError::UnknownSvgPathCommandType),
            }
        }

        let fill_rule = match path.kind {
            PartitionSvgPathKind::Fill(fill_rule) => fill_rule.to_fill_rule(),
            PartitionSvgPathKind::Stroke(_) => FillRule::Winding,
        };

        path_descriptors.push(PathDescriptor {
            path_index: path_index,
            fill_rule: fill_rule,
        });

        match path.kind {
            PartitionSvgPathKind::Fill(_) => paths.push(stream),
            PartitionSvgPathKind::Stroke(stroke_width) => {
                let iterator = PathIter::new(stream.into_iter());
                let stroke_style = StrokeStyle::new(stroke_width);
                let path: Vec<_> = StrokeToFillIter::new(iterator, stroke_style).collect();
                paths.push(path);
            }
        }

        path_index += 1;
    }

    // Partition the paths.
    let path_partitioning_result = PathPartitioningResult::compute(&mut partitioner,
                                                                   &path_descriptors,
                                                                   &paths);

    // Return the response.
    let elapsed_ms = path_partitioning_result.elapsed_ms();
    Ok(PartitionResponder {
        data: path_partitioning_result.encoded_data,
        time: elapsed_ms,
    })
}

#[post("/render-reference/text", format = "application/json", data = "<request>")]
fn render_reference_text(request: Json<RenderTextReferenceRequest>)
                         -> Result<ReferenceImage, FontError> {
    let font_key = FontKey::new();
    let otf_data = try!(otf_data_from_request(&request.face));
    let font_instance = FontInstance {
        font_key: font_key,
        size: Au::from_f64_px(request.point_size),
    };
    let glyph_key = GlyphKey::new(request.glyph, SubpixelOffset(0));

    // Rasterize the glyph using the right rasterizer.
    let glyph_image = match request.renderer {
        ReferenceTextRenderer::FreeType => {
            let mut font_context =
                try!(FontContext::new().map_err(|_| FontError::FontLoadingFailed));
            try!(font_context.add_font_from_memory(&font_key, otf_data, request.font_index)
                             .map_err(|_| FontError::FontLoadingFailed));
            try!(font_context.rasterize_glyph_with_native_rasterizer(&font_instance,
                                                                     &glyph_key,
                                                                     true)
                             .map_err(|_| FontError::RasterizationFailed))
        }
        ReferenceTextRenderer::CoreGraphics => {
            try!(rasterize_glyph_with_core_graphics(&font_key,
                                                    request.font_index,
                                                    otf_data,
                                                    &font_instance,
                                                    &glyph_key))
        }
    };

    let dimensions = &glyph_image.dimensions;
    let image_buffer = ImageBuffer::from_raw(dimensions.size.width,
                                             dimensions.size.height,
                                             glyph_image.pixels).unwrap();
    let reference_image = ReferenceImage {
        image: ImageRgba8(image_buffer),
    };

    Ok(reference_image)
}

#[cfg(feature = "reftests")]
#[post("/render-reference/svg", format = "application/json", data = "<request>")]
fn render_reference_svg(request: Json<RenderSvgReferenceRequest>)
                        -> Result<ReferenceImage, SvgError> {
    let svg_data = try!(svg_data_from_request(&request.name));
    let svg_string = String::from_utf8_lossy(&*svg_data);
    let svg_handle = try!(Handle::new_from_str(&svg_string).map_err(|_| SvgError::LoadingFailed));

    let svg_dimensions = svg_handle.get_dimensions();
    let mut image_size = Size2D::new(svg_dimensions.width as f64, svg_dimensions.height as f64);
    image_size = (image_size * request.scale).ceil();

    // Rasterize the SVG using the appropriate rasterizer.
    let mut surface = ImageSurface::create(Format::ARgb32,
                                           image_size.width as i32,
                                           image_size.height as i32).unwrap();

    {
        let cairo_context = cairo::Context::new(&surface);
        cairo_context.scale(request.scale, request.scale);
        svg_handle.render_cairo(&cairo_context);
    }

    let mut image_data = (*surface.get_data().unwrap()).to_vec();
    image_data.chunks_mut(4).for_each(|color| color.swap(0, 2));

    let image_buffer = match ImageBuffer::from_raw(image_size.width as u32,
                                                   image_size.height as u32,
                                                   image_data) {
        None => return Err(SvgError::ImageWritingFailed),
        Some(image_buffer) => image_buffer,
    };

    Ok(ReferenceImage {
        image: ImageRgba8(image_buffer),
    })
}

#[cfg(not(feature = "reftests"))]
#[post("/render-reference/svg", format = "application/json", data = "<request>")]
#[allow(unused_variables)]
fn render_reference_svg(request: Json<RenderSvgReferenceRequest>)
                        -> Result<ReferenceImage, SvgError> {
    Err(SvgError::ReftestsDisabled)
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
#[get("/tools/reference-test")]
fn static_tools_reference_test() -> io::Result<NamedFile> {
    NamedFile::open(STATIC_TOOLS_REFERENCE_TEST_PATH)
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
    NamedFile::open(path::Path::new(STATIC_DOC_API_PATH).join(file)).ok()
}
#[get("/css/bootstrap/<file..>")]
fn static_css_bootstrap(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new(STATIC_CSS_BOOTSTRAP_PATH).join(file)).ok()
}
#[get("/css/<file>")]
fn static_css(file: String) -> Option<NamedFile> {
    NamedFile::open(path::Path::new(STATIC_CSS_PATH).join(file)).ok()
}
#[get("/js/bootstrap/<file..>")]
fn static_js_bootstrap(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new(STATIC_JS_BOOTSTRAP_PATH).join(file)).ok()
}
#[get("/js/jquery/<file..>")]
fn static_js_jquery(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new(STATIC_JS_JQUERY_PATH).join(file)).ok()
}
#[get("/js/popper.js/<file..>")]
fn static_js_popper_js(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new(STATIC_JS_POPPER_JS_PATH).join(file)).ok()
}
#[get("/js/pathfinder/<file..>")]
fn static_js_pathfinder(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new(STATIC_JS_PATHFINDER_PATH).join(file)).ok()
}
#[get("/woff2/inter-ui/<file..>")]
fn static_woff2_inter_ui(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new(STATIC_WOFF2_INTER_UI_PATH).join(file)).ok()
}
#[get("/woff2/material-icons/<file..>")]
fn static_woff2_material_icons(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new(STATIC_WOFF2_MATERIAL_ICONS_PATH).join(file)).ok()
}
#[get("/glsl/<file..>")]
fn static_glsl(file: PathBuf) -> Option<Shader> {
    Shader::open(path::Path::new(STATIC_GLSL_PATH).join(file)).ok()
}
#[get("/otf/demo/<font_name>")]
fn static_otf_demo(font_name: String) -> Option<NamedFile> {
    BUILTIN_FONTS.iter()
                 .filter(|& &(name, _)| name == font_name)
                 .next()
                 .and_then(|&(_, path)| NamedFile::open(path::Path::new(path)).ok())
}
#[get("/svg/demo/<svg_name>")]
fn static_svg_demo(svg_name: String) -> Option<NamedFile> {
    BUILTIN_SVGS.iter()
                .filter(|& &(name, _)| name == svg_name)
                .next()
                .and_then(|&(_, path)| NamedFile::open(path::Path::new(path)).ok())
}
#[get("/data/<file..>")]
fn static_data(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new(STATIC_DATA_PATH).join(file)).ok()
}
#[get("/test-data/<file..>")]
fn static_test_data(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new(STATIC_TEST_DATA_PATH).join(file)).ok()
}
#[get("/textures/<file..>")]
fn static_textures(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new(STATIC_TEXTURES_PATH).join(file)).ok()
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

    let rocket = rocket::ignite();

    match rocket.config().limits.get("json") {
        Some(size) if size >= SUGGESTED_JSON_SIZE_LIMIT => {}
        None | Some(_) => {
            eprintln!("warning: the JSON size limit is small; many SVGs will not upload properly");
            eprintln!("warning: adding the following to `Rocket.toml` is suggested:");
            eprintln!("warning:    [development]");
            eprintln!("warning:    limits = {{ json = 33554432 }}");
        }
    }

    rocket.mount("/", routes![
        partition_font,
        partition_svg_paths,
        render_reference_text,
        render_reference_svg,
        static_index,
        static_demo_text,
        static_demo_svg,
        static_demo_3d,
        static_tools_benchmark,
        static_tools_reference_test,
        static_tools_mesh_debugger,
        static_doc_api_index,
        static_doc_api,
        static_css,
        static_css_bootstrap,
        static_js_bootstrap,
        static_js_jquery,
        static_js_popper_js,
        static_js_pathfinder,
        static_woff2_inter_ui,
        static_woff2_material_icons,
        static_glsl,
        static_otf_demo,
        static_svg_demo,
        static_data,
        static_test_data,
        static_textures,
    ]).launch();
}
