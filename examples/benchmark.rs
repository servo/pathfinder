/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

extern crate bencher;
extern crate compute_shader;
extern crate euclid;
extern crate gl;
extern crate glfw;
extern crate lord_drawquaad;
extern crate memmap;
extern crate pathfinder;
extern crate time;

use bencher::stats::{self, Stats};
use compute_shader::buffer;
use compute_shader::image::Format;
use compute_shader::instance::Instance;
use euclid::{Point2D, Rect, Size2D};
use gl::types::GLuint;
use glfw::{Context, OpenGlProfileHint, WindowHint, WindowMode};
use memmap::{Mmap, Protection};
use pathfinder::atlas::AtlasBuilder;
use pathfinder::charmap::CodepointRange;
use pathfinder::coverage::CoverageBuffer;
use pathfinder::font::Font;
use pathfinder::outline::OutlineBuilder;
use pathfinder::rasterizer::{Rasterizer, RasterizerOptions};
use std::env;
use std::os::raw::c_void;
use std::path::PathBuf;

const ATLAS_SIZE: u32 = 2048;
const WIDTH: u32 = 512;
const HEIGHT: u32 = 384;

const MIN_TIME_PER_SIZE: u64 = 300_000_000;
const MAX_TIME_PER_SIZE: u64 = 3_000_000_000;

static SHADER_PATH: &'static str = "resources/shaders/";

fn main() {
    let mut glfw = glfw::init(glfw::LOG_ERRORS).unwrap();
    glfw.window_hint(WindowHint::ContextVersion(3, 3));
    glfw.window_hint(WindowHint::OpenGlForwardCompat(true));
    glfw.window_hint(WindowHint::OpenGlProfile(OpenGlProfileHint::Core));
    let context = glfw.create_window(WIDTH, HEIGHT, "generate-atlas", WindowMode::Windowed);

    let (mut window, _events) = context.expect("Couldn't create a window!");
    window.make_current();
    gl::load_with(|symbol| window.get_proc_address(symbol) as *const c_void);
    let (device_pixel_width, _) = window.get_framebuffer_size();

    let instance = Instance::new().unwrap();
    let device = instance.open_device().unwrap();
    let queue = device.create_queue().unwrap();

    let mut rasterizer_options = RasterizerOptions::from_env().unwrap();
    if env::var("PATHFINDER_SHADER_PATH").is_err() {
        rasterizer_options.shader_path = PathBuf::from(SHADER_PATH)
    }

    let rasterizer = Rasterizer::new(&instance, device, queue, rasterizer_options).unwrap();

    for point_size in 6..201 {
        // FIXME(pcwalton)
        let shelf_height = point_size * 2;

        let file = Mmap::open_path(env::args().nth(1).unwrap(), Protection::Read).unwrap();

        let mut results = vec![];
        let start = time::precise_time_ns();
        let mut last_time = start;
        let (mut outlines, mut glyph_count, mut atlas);

        loop {
            glyph_count = 0;
            unsafe {
                let font = Font::new(file.as_slice()).unwrap();
                let codepoint_ranges = [CodepointRange::new(' ' as u32, '~' as u32)];

                let glyph_mapping = font.glyph_mapping_for_codepoint_ranges(&codepoint_ranges)
                                        .unwrap();
                let mut outline_builder = OutlineBuilder::new();
                for (_, glyph_id) in glyph_mapping.iter() {
                    outline_builder.add_glyph(&font, glyph_id).unwrap();
                    glyph_count += 1
                }
                outlines = outline_builder.create_buffers().unwrap();

                let mut atlas_builder = AtlasBuilder::new(device_pixel_width as GLuint,
                                                          shelf_height);
                for glyph_index in 0..(glyph_count as u16) {
                    atlas_builder.pack_glyph(&outlines, glyph_index, point_size as f32, 0.0)
                                 .unwrap();
                }
                atlas = atlas_builder.create_atlas().unwrap();
            }

            let end = time::precise_time_ns();
            results.push((end - last_time) as f64);
            if end - start > MAX_TIME_PER_SIZE {
                break
            }
            last_time = end
        }

        stats::winsorize(&mut results, 5.0);
        let time_per_glyph = results.mean() / 1_000.0 / glyph_count as f64;
        println!("cpu,{}", time_per_glyph);

        let atlas_size = Size2D::new(ATLAS_SIZE, ATLAS_SIZE);
        let coverage_buffer = CoverageBuffer::new(rasterizer.device(), &atlas_size).unwrap();

        let image = rasterizer.device()
                              .create_image(Format::R8, buffer::Protection::WriteOnly, &atlas_size)
                              .unwrap();

        let rect = Rect::new(Point2D::new(0, 0), atlas_size);

        let mut results = vec![];
        let start_time = time::precise_time_ns();
        loop {
            let events = rasterizer.draw_atlas(&image, &rect, &atlas, &outlines, &coverage_buffer)
                                   .unwrap();

            let mut draw_time = 0u64;
            unsafe {
                gl::GetQueryObjectui64v(events.draw, gl::QUERY_RESULT, &mut draw_time);
            }
            let accum_time = events.accum.time_elapsed().unwrap() as f64;
            let time_per_glyph = (draw_time as f64 + accum_time as f64) /
                (1000.0 * glyph_count as f64);
            results.push(time_per_glyph);

            let now = time::precise_time_ns();
            if (now - start_time > MIN_TIME_PER_SIZE && results.median_abs_dev_pct() < 1.0) ||
                now - start_time > MAX_TIME_PER_SIZE {
                break
            }
        }

        stats::winsorize(&mut results, 5.0);
        let time_per_glyph = results.mean();
        println!("{},{}", point_size, time_per_glyph);
    }

    window.set_should_close(true);
}


