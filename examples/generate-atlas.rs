/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

extern crate clap;
extern crate compute_shader;
extern crate euclid;
extern crate gl;
extern crate glfw;
extern crate lord_drawquaad;
extern crate memmap;
extern crate pathfinder;

use clap::{App, Arg};
use compute_shader::buffer;
use compute_shader::image::{Color, ExternalImage, Format};
use compute_shader::instance::{Instance, ShadingLanguage};
use euclid::{Point2D, Rect, Size2D};
use gl::types::{GLint, GLuint};
use glfw::{Action, Context, Key, OpenGlProfileHint, WindowEvent, WindowHint, WindowMode};
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

const DEFAULT_POINT_SIZE: f32 = 24.0;
const WIDTH: u32 = 512;
const HEIGHT: u32 = 384;

static SHADER_PATH: &'static str = "resources/shaders/";

fn main() {
    let index_arg = Arg::with_name("index").short("i")
                                           .long("index")
                                           .help("Select an index within a font collection")
                                           .takes_value(true);
    let font_arg = Arg::with_name("FONT-FILE").help("Select the font file (`.ttf`, `.otf`, etc.)")
                                              .required(true)
                                              .index(1);
    let point_size_arg = Arg::with_name("POINT-SIZE").help("Select the point size")
                                                     .index(2);
    let matches = App::new("generate-atlas").arg(index_arg)
                                            .arg(font_arg)
                                            .arg(point_size_arg)
                                            .get_matches();

    let mut glfw = glfw::init(glfw::LOG_ERRORS).unwrap();
    glfw.window_hint(WindowHint::ContextVersion(3, 3));
    glfw.window_hint(WindowHint::OpenGlForwardCompat(true));
    glfw.window_hint(WindowHint::OpenGlProfile(OpenGlProfileHint::Core));
    let context = glfw.create_window(WIDTH, HEIGHT, "generate-atlas", WindowMode::Windowed);

    let (mut window, events) = context.expect("Couldn't create a window!");
    window.make_current();
    gl::load_with(|symbol| window.get_proc_address(symbol) as *const c_void);
    let (device_pixel_width, device_pixel_height) = window.get_framebuffer_size();

    let instance = Instance::new().unwrap();
    let device = instance.open_device().unwrap();
    let queue = device.create_queue().unwrap();

    let mut rasterizer_options = RasterizerOptions::from_env().unwrap();
    if env::var("PATHFINDER_SHADER_PATH").is_err() {
        rasterizer_options.shader_path = PathBuf::from(SHADER_PATH)
    }

    let rasterizer = Rasterizer::new(&instance, device, queue, rasterizer_options).unwrap();

    let file = Mmap::open_path(matches.value_of("FONT-FILE").unwrap(), Protection::Read).unwrap();

    let point_size = match matches.value_of("POINT-SIZE") {
        Some(point_size) => point_size.parse().unwrap(),
        None => DEFAULT_POINT_SIZE,
    };

    let font_index = match matches.value_of("index") {
        Some(index) => index.parse().unwrap(),
        None => 0,
    };

    let (outlines, atlas);
    unsafe {
        let font = Font::from_collection_index(file.as_slice(), font_index).unwrap();
        let codepoint_ranges = [CodepointRange::new(' ' as u32, '~' as u32)];
        let glyph_mapping = font.glyph_mapping_for_codepoint_ranges(&codepoint_ranges).unwrap();

        let shelf_height = font.shelf_height(point_size);

        let mut outline_builder = OutlineBuilder::new();
        let mut glyph_count = 0;
        for (_, glyph_id) in glyph_mapping.iter() {
            outline_builder.add_glyph(&font, glyph_id).unwrap();
            glyph_count += 1
        }
        outlines = outline_builder.create_buffers().unwrap();

        let mut atlas_builder = AtlasBuilder::new(device_pixel_width as GLuint, shelf_height);
        for glyph_index in 0..glyph_count {
            atlas_builder.pack_glyph(&outlines, glyph_index, point_size, 0.0).unwrap();
        }
        atlas = atlas_builder.create_atlas().unwrap();
    }

    let atlas_size = Size2D::new(device_pixel_width as GLuint, device_pixel_height as GLuint);
    let coverage_buffer = CoverageBuffer::new(rasterizer.device(), &atlas_size).unwrap();

    let image = rasterizer.device()
                          .create_image(Format::RGBA8, buffer::Protection::ReadWrite, &atlas_size)
                          .unwrap();

    rasterizer.queue().submit_clear(&image, &Color::UInt(0, 0, 0, 0), &[]).unwrap();

    let rect = Rect::new(Point2D::new(0, 0), atlas_size);

    rasterizer.draw_atlas(&image, &rect, &atlas, &outlines, &coverage_buffer).unwrap();
    rasterizer.queue().flush().unwrap();

    let draw_context = lord_drawquaad::Context::new();

    let mut gl_texture = 0;
    unsafe {
        if instance.shading_language() == ShadingLanguage::Glsl {
            gl::MemoryBarrier(gl::SHADER_IMAGE_ACCESS_BARRIER_BIT | gl::TEXTURE_FETCH_BARRIER_BIT);
        }

        gl::GenTextures(1, &mut gl_texture);
        image.bind_to(&ExternalImage::GlTexture(gl_texture)).unwrap();

        gl::BindTexture(gl::TEXTURE_RECTANGLE, gl_texture);
        gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
        gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);

        gl::Viewport(0, 0, device_pixel_width, device_pixel_height);
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

