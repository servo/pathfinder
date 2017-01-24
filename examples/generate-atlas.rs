/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

extern crate compute_shader;
extern crate euclid;
extern crate gl;
extern crate glfw;
extern crate lord_drawquaad;
extern crate memmap;
extern crate pathfinder;

use compute_shader::buffer;
use compute_shader::instance::Instance;
use compute_shader::texture::{ExternalTexture, Format};
use euclid::{Point2D, Rect, Size2D};
use gl::types::GLint;
use glfw::{Action, Context, Key, OpenGlProfileHint, WindowEvent, WindowHint, WindowMode};
use memmap::{Mmap, Protection};
use pathfinder::batch::{BatchBuilder, GlyphRange};
use pathfinder::charmap::CodepointRange;
use pathfinder::coverage::CoverageBuffer;
use pathfinder::glyph_buffer::GlyphBufferBuilder;
use pathfinder::otf::FontData;
use pathfinder::rasterizer::Rasterizer;
use std::env;
use std::os::raw::c_void;

const POINT_SIZE: f32 = 24.0;
const WIDTH: u32 = 512;
const HEIGHT: u32 = 384;
const SHELF_HEIGHT: u32 = 32;

fn main() {
    let mut glfw = glfw::init(glfw::LOG_ERRORS).unwrap();
    glfw.window_hint(WindowHint::ContextVersion(3, 3));
    glfw.window_hint(WindowHint::OpenGlForwardCompat(true));
    glfw.window_hint(WindowHint::OpenGlProfile(OpenGlProfileHint::Core));
    let context = glfw.create_window(WIDTH, HEIGHT, "generate-atlas", WindowMode::Windowed);

    let (mut window, events) = context.expect("Couldn't create a window!");
    window.make_current();
    gl::load_with(|symbol| window.get_proc_address(symbol) as *const c_void);

    let instance = Instance::new().unwrap();
    let device = instance.create_device().unwrap();
    let queue = device.create_queue().unwrap();

    let rasterizer = Rasterizer::new(device, queue).unwrap();

    let mut glyph_buffer_builder = GlyphBufferBuilder::new();
    let mut batch_builder = BatchBuilder::new(WIDTH, SHELF_HEIGHT);

    let file = Mmap::open_path(env::args().nth(1).unwrap(), Protection::Read).unwrap();
    unsafe {
        let font = FontData::new(file.as_slice());
        let cmap = font.cmap_table().unwrap();
        let glyf = font.glyf_table().unwrap();
        let head = font.head_table().unwrap();
        let loca = font.loca_table(&head).unwrap();
        let codepoint_ranges = [CodepointRange::new('!' as u32, '~' as u32)];

        let glyph_ranges = cmap.glyph_ranges_for_codepoint_ranges(&codepoint_ranges).unwrap();
        for (glyph_index, glyph_id) in glyph_ranges.iter().flat_map(GlyphRange::iter).enumerate() {
            glyph_buffer_builder.add_glyph(glyph_id as u32, &head, &loca, &glyf).unwrap();
            batch_builder.add_glyph(&glyph_buffer_builder, glyph_index as u32, POINT_SIZE).unwrap()
        }
    }

    let glyph_buffers = glyph_buffer_builder.finish().unwrap();
    let batch = batch_builder.finish(&glyph_buffer_builder).unwrap();

    let atlas_size = Size2D::new(WIDTH, HEIGHT);
    let coverage_buffer = CoverageBuffer::new(&rasterizer.device, &atlas_size).unwrap();

    let texture = rasterizer.device
                            .create_texture(Format::R8, buffer::Protection::WriteOnly, &atlas_size)
                            .unwrap();

    rasterizer.draw_atlas(&Rect::new(Point2D::new(0, 0), atlas_size),
                          SHELF_HEIGHT,
                          &glyph_buffers,
                          &batch,
                          &coverage_buffer,
                          &texture).unwrap().wait().unwrap();

    let draw_context = lord_drawquaad::Context::new();

    let mut gl_texture = 0;
    unsafe {
        gl::GenTextures(1, &mut gl_texture);
        texture.bind_to(&ExternalTexture::Gl(gl_texture)).unwrap();

        gl::BindTexture(gl::TEXTURE_RECTANGLE, gl_texture);
        gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
        gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);
    }

    unsafe {
        gl::ClearColor(1.0, 1.0, 1.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }

    draw_context.draw(gl_texture);
    window.swap_buffers();

    while !window.should_close() {
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    window.set_should_close(true)
                }
                _ => {}
            }
        }
    }
}

