// pathfinder/examples/canvas_metal_minimal/src/main.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[macro_use]
extern crate objc;

use foreign_types::ForeignTypeRef;
use metal::{CAMetalLayer, CoreAnimationLayerRef, DeviceRef, MTLClearColor, MTLDevice};
use metal::{MTLLoadAction, MTLStoreAction, RenderPassDescriptor};
use pathfinder_canvas::{CanvasFontContext, CanvasRenderingContext2D, Path2D};
use pathfinder_geometry::basic::vector::{Vector2F, Vector2I};
use pathfinder_geometry::basic::rect::RectF;
use pathfinder_geometry::color::ColorF;
use pathfinder_gl::{GLDevice, GLVersion};
use pathfinder_gpu::resources::FilesystemResourceLoader;
use pathfinder_gpu::{ClearParams, Device};
use pathfinder_renderer::concurrent::rayon::RayonExecutor;
use pathfinder_renderer::concurrent::scene_proxy::SceneProxy;
use pathfinder_renderer::gpu::renderer::{DestFramebuffer, Renderer};
use pathfinder_renderer::options::RenderOptions;
use sdl2::event::Event;
use sdl2::hint;
use sdl2::keyboard::Keycode;
use sdl2::render::Canvas;
use sdl2::video::GLProfile;
use sdl2_sys::SDL_RenderGetMetalLayer;

fn main() {
    // Set up SDL2.
    assert!(hint::set("SDL_RENDER_DRIVER", "metal"));
    let sdl_context = sdl2::init().unwrap();
    let video = sdl_context.video().unwrap();

    // Open a window.
    let window_size = Vector2I::new(640, 480);
    let window = video.window("Minimal example", window_size.x() as u32, window_size.y() as u32)
                      .opengl()
                      .build()
                      .unwrap();

    // Create a Metal context.
    let canvas = window.into_canvas().present_vsync().build().unwrap();
    let (metal_layer, device, drawable);
    unsafe {
        metal_layer = CoreAnimationLayerRef::from_ptr(SDL_RenderGetMetalLayer(canvas.raw()) as
                                                      *mut CAMetalLayer);
        device = DeviceRef::from_ptr(msg_send![metal_layer.as_ptr(), device]);
        drawable = metal_layer.next_drawable().unwrap();
    }

    // Clear to white.
    let render_pass_descriptor = RenderPassDescriptor::new();
    let color_attachment = render_pass_descriptor.color_attachments().object_at(0).unwrap();
    color_attachment.set_texture(Some(drawable.texture()));
    color_attachment.set_clear_color(MTLClearColor::new(0.0, 0.0, 1.0, 1.0));
    color_attachment.set_load_action(MTLLoadAction::Clear);
    color_attachment.set_store_action(MTLStoreAction::Store);
    let queue = device.new_command_buffer();
    let command_buffer = queue.new_command_buffer();
    let encoder = command_buffer.new_render_command_encoder(render_pass_descriptor);
    encoder.end_encoding();
    command_buffer.present_drawable(drawable);
    command_buffer.commit();

    // Wait for a keypress.
    let mut event_pump = sdl_context.event_pump().unwrap();
    loop {
        match event_pump.wait_event() {
            Event::Quit {..} | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => return,
            _ => {}
        }
    }
}
