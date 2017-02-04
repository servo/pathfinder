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
use pathfinder::glyph_buffer::GlyphBufferBuilder;
use pathfinder::otf::Font;
use pathfinder::rasterizer::{Rasterizer, RasterizerOptions};
use std::env;
use std::os::raw::c_void;

const ATLAS_SIZE: u32 = 2048;
const WIDTH: u32 = 512;
const HEIGHT: u32 = 384;

const MIN_TIME_PER_SIZE: u64 = 300_000_000;
const MAX_TIME_PER_SIZE: u64 = 3_000_000_000;

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

    let rasterizer_options = RasterizerOptions::from_env().unwrap();
    let rasterizer = Rasterizer::new(&instance, device, queue, rasterizer_options).unwrap();

    for point_size in 6..201 {
        // FIXME(pcwalton)
        let shelf_height = point_size * 2;

        let file = Mmap::open_path(env::args().nth(1).unwrap(), Protection::Read).unwrap();

        let mut results = vec![];
        let start = time::precise_time_ns();
        let mut last_time = start;
        let (mut glyph_buffer_builder, mut glyph_buffers, mut glyph_count);
        let (mut atlas_builder, mut atlas);

        loop {
            glyph_buffer_builder = GlyphBufferBuilder::new();
            atlas_builder = AtlasBuilder::new(device_pixel_width as GLuint, shelf_height);
            glyph_count = 0;
            unsafe {
                let font = Font::new(file.as_slice()).unwrap();
                let codepoint_ranges = [CodepointRange::new(' ' as u32, '~' as u32)];

                let glyph_ranges = font.glyph_ranges_for_codepoint_ranges(&codepoint_ranges)
                                       .unwrap();
                for (glyph_index, glyph_id) in glyph_ranges.iter().enumerate() {
                    glyph_buffer_builder.add_glyph(&font, glyph_id).unwrap();
                    atlas_builder.pack_glyph(&glyph_buffer_builder,
                                             glyph_index as u32,
                                             point_size as f32).unwrap();
                    glyph_count += 1
                }

            }

            glyph_buffers = glyph_buffer_builder.create_buffers().unwrap();
            atlas = atlas_builder.create_atlas(&glyph_buffer_builder).unwrap();

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
        let coverage_buffer = CoverageBuffer::new(&rasterizer.device, &atlas_size).unwrap();

        let image = rasterizer.device
                              .create_image(Format::R8, buffer::Protection::WriteOnly, &atlas_size)
                              .unwrap();

        let rect = Rect::new(Point2D::new(0, 0), atlas_size);

        let mut results = vec![];
        let start_time = time::precise_time_ns();
        loop {
            let events =
                rasterizer.draw_atlas(&image, &rect, &atlas, &glyph_buffers, &coverage_buffer)
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


