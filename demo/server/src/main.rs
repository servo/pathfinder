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
extern crate opentype_sanitizer_sys;
extern crate pathfinder_font_renderer;
extern crate pathfinder_partitioner;
extern crate rocket;
extern crate rocket_contrib;
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
use std::io;
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
}

#[derive(Clone, Serialize, Deserialize)]
struct PartitionFontRequest {
    // Base64 encoded.
    otf: String,
    font_index: u32,
    glyph_ids: Vec<u32>,
    point_size: f64,
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

#[derive(Clone, Copy, Serialize, Deserialize)]
struct PartitionGlyphInfo {
    id: u32,
    dimensions: PartitionGlyphDimensions,
    b_quad_indices: IndexRange,
    b_vertex_indices: IndexRange,
}

#[derive(Clone, Serialize, Deserialize)]
struct PartitionFontResponse {
    glyph_info: Vec<PartitionGlyphInfo>,
    // Base64-encoded `bincode`-encoded `BQuad`s.
    b_quads: String,
    // Base64-encoded `bincode`-encoded `BVertex`es.
    b_vertices: String,
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
        size: Au::from_f64_px(request.point_size),
    };
    let mut font_context = FontContext::new();
    if font_context.add_font_from_memory(&font_key, otf_data, request.font_index).is_err() {
        return Json(Err(PartitionFontError::FontLoadingFailed))
    }

    // Read glyph info.
    let mut outline_buffer = GlyphOutlineBuffer::new();
    let decoded_outline_indices: Vec<_> = request.glyph_ids.iter().map(|&glyph_id| {
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
    let (mut b_quad_count, mut b_vertex_count) = (0, 0);
    let (mut b_quads, mut b_vertices) = (vec![], vec![]);
    partitioner.init(&outline_buffer.endpoints,
                     &outline_buffer.control_points,
                     &outline_buffer.subpaths);
    let mut glyph_info = vec![];
    for (path_index, (&glyph_id, decoded_outline_indices)) in
            request.glyph_ids.iter().zip(decoded_outline_indices.iter()).enumerate() {
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
        let (first_b_quad_index, first_b_vertex_index) = (b_quad_count, b_vertex_count);
        let last_b_quad_index = first_b_quad_index + path_b_quads.len();
        let last_b_vertex_index = first_b_vertex_index + path_b_vertices.len();

        for b_quad in partitioner.b_quads() {
            if bincode::serialize_into(&mut b_quads, b_quad, Infinite).is_err() {
                return Json(Err(PartitionFontError::BincodeSerializationFailed))
            }
        }
        for b_vertex in partitioner.b_vertices() {
            if bincode::serialize_into(&mut b_vertices, b_vertex, Infinite).is_err() {
                return Json(Err(PartitionFontError::BincodeSerializationFailed))
            }
        }

        b_quad_count = last_b_quad_index;
        b_vertex_count = last_b_vertex_index;

        glyph_info.push(PartitionGlyphInfo {
            id: glyph_id,
            dimensions: dimensions,
            b_quad_indices: IndexRange::new(first_b_quad_index, last_b_quad_index),
            b_vertex_indices: IndexRange::new(first_b_vertex_index, last_b_vertex_index),
        })
    }

    // Return the response.
    Json(Ok(PartitionFontResponse {
        glyph_info: glyph_info,
        b_quads: base64::encode(&b_quads),
        b_vertices: base64::encode(&b_vertices),
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
