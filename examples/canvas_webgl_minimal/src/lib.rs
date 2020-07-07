// pathfinder/examples/canvas_webgl_minimal/src/lib.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use pathfinder_canvas::{Canvas, CanvasFontContext, Path2D};
use pathfinder_color::ColorF;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{vec2f, vec2i};
use pathfinder_renderer::concurrent::executor::SequentialExecutor;
use pathfinder_renderer::gpu::options::{DestFramebuffer, RendererMode, RendererOptions};
use pathfinder_renderer::gpu::renderer::Renderer;
use pathfinder_renderer::options::BuildOptions;
use pathfinder_resources::embedded::EmbeddedResourceLoader;
use pathfinder_webgl::WebGlDevice;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{self, HtmlCanvasElement, WebGl2RenderingContext};

mod utils;

#[wasm_bindgen]
pub fn rust_main() {
    utils::set_panic_hook();

    let document = web_sys::window().unwrap().document().unwrap();
    let canvas = document.get_element_by_id("c").unwrap();
    let canvas: HtmlCanvasElement = canvas.dyn_into::<HtmlCanvasElement>().unwrap();

    let context = canvas.get_context("webgl2")
                        .unwrap()
                        .unwrap()
                        .dyn_into::<WebGl2RenderingContext>()
                        .unwrap();

    // Get the real size of the window, taking HiDPI into account.
    let framebuffer_size = vec2i(canvas.width() as i32, canvas.height() as i32);

    // Create a Pathfinder GL device.
    let pathfinder_device = WebGlDevice::new(context);

    // Create a Pathfinder renderer.
    let mode = RendererMode::default_for_device(&pathfinder_device);
    let options = RendererOptions {
        dest: DestFramebuffer::full_window(framebuffer_size),
        background_color: Some(ColorF::white()),
        ..RendererOptions::default()
    };
    let resource_loader = EmbeddedResourceLoader::new();
    let mut renderer = Renderer::new(pathfinder_device, &resource_loader, mode, options);

    // Make a canvas. We're going to draw a house.
    let font_context = CanvasFontContext::from_system_source();
    let mut canvas = Canvas::new(framebuffer_size.to_f32()).get_context_2d(font_context);

    // Set line width.
    canvas.set_line_width(10.0);

    // Draw walls.
    canvas.stroke_rect(RectF::new(vec2f(75.0, 140.0), vec2f(150.0, 110.0)));

    // Draw door.
    canvas.fill_rect(RectF::new(vec2f(130.0, 190.0), vec2f(40.0, 60.0)));

    // Draw roof.
    let mut path = Path2D::new();
    path.move_to(vec2f(50.0, 140.0));
    path.line_to(vec2f(150.0, 60.0));
    path.line_to(vec2f(250.0, 140.0));
    path.close_path();
    canvas.stroke_path(path);

    // Render the canvas to screen.
    let mut scene = canvas.into_canvas().into_scene();
    scene.build_and_render(&mut renderer, BuildOptions::default(), SequentialExecutor);
}
