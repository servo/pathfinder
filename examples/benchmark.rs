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
use compute_shader::instance::Instance;
use compute_shader::texture::Format;
use euclid::{Point2D, Rect, Size2D};
use gl::types::GLuint;
use glfw::{Context, OpenGlProfileHint, WindowHint, WindowMode};
use memmap::{Mmap, Protection};
use pathfinder::batch::BatchBuilder;
use pathfinder::charmap::CodepointRange;
use pathfinder::coverage::CoverageBuffer;
use pathfinder::glyph_buffer::GlyphBufferBuilder;
use pathfinder::otf::Font;
use pathfinder::rasterizer::{Rasterizer, RasterizerOptions};
use std::env;
use std::os::raw::c_void;

const WIDTH: u32 = 512;
const HEIGHT: u32 = 384;

fn main() {
    let mut glfw = glfw::init(glfw::LOG_ERRORS).unwrap();
    glfw.window_hint(WindowHint::ContextVersion(3, 3));
    glfw.window_hint(WindowHint::OpenGlForwardCompat(true));
    glfw.window_hint(WindowHint::OpenGlProfile(OpenGlProfileHint::Core));
    let context = glfw.create_window(WIDTH, HEIGHT, "generate-atlas", WindowMode::Windowed);

    let (mut window, _events) = context.expect("Couldn't create a window!");
    window.make_current();
    gl::load_with(|symbol| window.get_proc_address(symbol) as *const c_void);
    let (device_pixel_width, device_pixel_height) = window.get_framebuffer_size();

    let instance = Instance::new().unwrap();
    let device = instance.create_device().unwrap();
    let queue = device.create_queue().unwrap();

    let rasterizer_options = RasterizerOptions::from_env().unwrap();
    let rasterizer = Rasterizer::new(&instance, device, queue, rasterizer_options).unwrap();

    for point_size in 6..201 {
        // FIXME(pcwalton)
        let shelf_height = point_size * 2;

        let mut glyph_buffer_builder = GlyphBufferBuilder::new();
        let mut batch_builder = BatchBuilder::new(device_pixel_width as GLuint, shelf_height);

        let file = Mmap::open_path(env::args().nth(1).unwrap(), Protection::Read).unwrap();
        let mut glyph_count = 0;
        unsafe {
            let font = Font::new(file.as_slice()).unwrap();
            let codepoint_ranges = [CodepointRange::new(' ' as u32, '~' as u32)];

            let glyph_ranges = font.cmap.glyph_ranges_for_codepoint_ranges(&codepoint_ranges)
                                        .unwrap();
            for (glyph_index, glyph_id) in glyph_ranges.iter().enumerate() {
                glyph_buffer_builder.add_glyph(&font, glyph_id).unwrap();
                batch_builder.add_glyph(&glyph_buffer_builder,
                                        glyph_index as u32,
                                        point_size as f32)
                             .unwrap();
                glyph_count += 1
            }
        }

        let glyph_buffers = glyph_buffer_builder.create_buffers().unwrap();
        let batch = batch_builder.create_batch(&glyph_buffer_builder).unwrap();

        let atlas_size = Size2D::new(device_pixel_width as GLuint, device_pixel_height as GLuint);
        let coverage_buffer = CoverageBuffer::new(&rasterizer.device, &atlas_size).unwrap();

        let texture = rasterizer.device
                                .create_texture(Format::R8,
                                                buffer::Protection::WriteOnly,
                                                &atlas_size)
                                .unwrap();

        let mut results = vec![];
        let start_time = time::precise_time_ns();
        loop {
            let events = rasterizer.draw_atlas(&Rect::new(Point2D::new(0, 0), atlas_size),
                                               &batch_builder.atlas,
                                               &glyph_buffers,
                                               &batch,
                                               &coverage_buffer,
                                               &texture).unwrap();

            let mut draw_time = 0u64;
            unsafe {
                gl::GetQueryObjectui64v(events.draw, gl::QUERY_RESULT, &mut draw_time);
            }
            let accum_time = events.accum.time_elapsed().unwrap() as f64;
            let time_per_glyph = (draw_time as f64 + accum_time as f64) /
                (1000.0 * glyph_count as f64);
            results.push(time_per_glyph);

            let now = time::precise_time_ns();
            if (now - start_time > 300_000_000 && results.median_abs_dev_pct() < 1.0) ||
                now - start_time > 3_000_000_000 {
                break
            }
        }

        stats::winsorize(&mut results, 5.0);
        let time_per_glyph = results.mean();
        println!("{},{}", point_size, time_per_glyph);
    }

    window.set_should_close(true);
}


