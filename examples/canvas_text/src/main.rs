// pathfinder/examples/canvas_text/src/main.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use pathfinder_canvas::CanvasRenderingContext2D;
use pathfinder_geometry::basic::point::{Point2DF, Point2DI};
use pathfinder_geometry::color::ColorF;
use pathfinder_gl::{GLDevice, GLVersion};
use pathfinder_gpu::resources::FilesystemResourceLoader;
use pathfinder_gpu::{ClearParams, Device};
use pathfinder_renderer::concurrent::rayon::RayonExecutor;
use pathfinder_renderer::concurrent::scene_proxy::SceneProxy;
use pathfinder_renderer::gpu::renderer::{DestFramebuffer, Renderer};
use pathfinder_renderer::options::RenderOptions;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::video::GLProfile;

fn main() {
    // Set up SDL2.
    let sdl_context = sdl2::init().unwrap();
    let video = sdl_context.video().unwrap();

    // Make sure we have at least a GL 3.0 context. Pathfinder requires this.
    let gl_attributes = video.gl_attr();
    gl_attributes.set_context_profile(GLProfile::Core);
    gl_attributes.set_context_version(3, 3);

    // Open a window.
    let window_size = Point2DI::new(640, 480);
    let window = video.window("Text example", window_size.x() as u32, window_size.y() as u32)
                      .opengl()
                      .build()
                      .unwrap();

    // Create the GL context, and make it current.
    let gl_context = window.gl_create_context().unwrap();
    gl::load_with(|name| video.gl_get_proc_address(name) as *const _);
    window.gl_make_current(&gl_context).unwrap();

    // Create a Pathfinder renderer.
    let mut renderer = Renderer::new(GLDevice::new(GLVersion::GL3, 0),
                                     &FilesystemResourceLoader::locate(),
                                     DestFramebuffer::full_window(window_size));

    // Clear to white.
    renderer.device.clear(&ClearParams { color: Some(ColorF::white()), ..ClearParams::default() });

    // Make a canvas. We're going to draw some text.
    let mut canvas = CanvasRenderingContext2D::new(window_size.to_f32());

    // Draw the text.
    canvas.set_font_size(32.0);
    canvas.fill_text("Hello Pathfinder!", Point2DF::new(32.0, 48.0));
    canvas.stroke_text("Goodbye Pathfinder!", Point2DF::new(32.0, 96.0));

    // Render the canvas to screen.
    let scene = SceneProxy::from_scene(canvas.into_scene(), RayonExecutor);
    scene.build_and_render(&mut renderer, RenderOptions::default());
    window.gl_swap_window();

    // Wait for a keypress.
    let mut event_pump = sdl_context.event_pump().unwrap();
    loop {
        match event_pump.wait_event() {
            Event::Quit {..} | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => return,
            _ => {}
        }
    }
}
