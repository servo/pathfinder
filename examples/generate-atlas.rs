/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

extern crate compute_shader;
extern crate gl;
extern crate glfw;
extern crate memmap;
extern crate pathfinder;

use compute_shader::instance::Instance;
use glfw::{Context, OpenGlProfileHint, WindowHint, WindowMode};
use memmap::{Mmap, Protection};
use pathfinder::batch::{BatchBuilder, GlyphRange};
use pathfinder::charmap::CodepointRange;
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

    let (mut window, _) = context.expect("Couldn't create a window!");
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

    let glyph_buffer = glyph_buffer_builder.finish(&rasterizer.device).unwrap();
    let batch = batch_builder.finish(&rasterizer.device).unwrap();
    // TODO(pcwalton): ...
}

