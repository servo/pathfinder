// pathfinder/examples/canvas_glow/src/main.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use glow::HasRenderLoop;
use pathfinder_canvas::{CanvasFontContext, CanvasRenderingContext2D, Path2D};
use pathfinder_content::color::ColorF;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{Vector2F, Vector2I};
use pathfinder_glow::GLOWDevice;
use pathfinder_renderer::concurrent::executor::SequentialExecutor;
use pathfinder_renderer::gpu::options::{DestFramebuffer, RendererOptions};
use pathfinder_renderer::gpu::renderer::Renderer;
use pathfinder_renderer::gpu_data::RenderCommand;
use pathfinder_renderer::options::BuildOptions;
use std::sync::{Arc, Mutex};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{prelude::*, JsCast};

mod static_resource_loader;

// Glow (GL on Whatever) is an abstraction that allows code to run in multiple native and web
// environments.
//
// This example can either be run with SDL, or in a browser targeting wasm32-unknown-unknown, with
// web-sys & WebGL 2.
//
// To run in SDL 2:
//
// ```
// cd ./examples/canvas_glow
// cargo run
// ```
//
// To build for a web browser, first setup wasm-pack by following the instructions at
// https://rustwasm.github.io/wasm-pack/installer, then run
//
// ```
// cd ./examples/canvas_glow
// wasm-pack build
// ```
//
// To run in a web browser, you need to serve canvas_glow/. If you have npm installed, you can do
// so by running:
// ```
// npx serve .
// ```
//
// Then, load http://localhost:5000 in your web browser.
//
// In your app, consider using a template based on https://github.com/rustwasm/wasm-pack-template

/// Native-specific initialization.
///
/// Note that the GLContext should not be dropped until glow::Context is dropped.
#[cfg(not(target_arch = "wasm32"))]
fn init_sdl(
    size: &Vector2I,
) -> (
    glow::RenderLoop<sdl2::video::Window>,
    glow::Context,
    sdl2::video::GLContext,
    sdl2::EventPump,
) {
    // Set up SDL2.
    let sdl_context = sdl2::init().unwrap();
    let video = sdl_context.video().unwrap();

    // Make sure we have at least a GL 3.0 context. Pathfinder requires this.
    // Core and GLES both work.
    let gl_attributes = video.gl_attr();
    gl_attributes.set_context_profile(sdl2::video::GLProfile::GLES);
    gl_attributes.set_context_version(3, 1);

    // Open a window.
    let window = video
        .window("Minimal example", size.x() as u32, size.y() as u32)
        .opengl()
        .build()
        .unwrap();

    // The GL context should not be dropped before the glow::Context is dropped.
    let gl_context = window.gl_create_context().unwrap();
    let glow_context =
        glow::Context::from_loader_function(|s| video.gl_get_proc_address(s) as *const _);
    let render_loop = glow::RenderLoop::<sdl2::video::Window>::from_sdl_window(window);
    let event_loop = sdl_context.event_pump().unwrap();

    (render_loop, glow_context, gl_context, event_loop)
}

/// Web-specific initialization.
#[cfg(target_arch = "wasm32")]
fn init_web(size: &Vector2I) -> (glow::RenderLoop, glow::Context) {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();

    let canvas = document.get_element_by_id("canvas").unwrap();
    let canvas: web_sys::HtmlCanvasElement = canvas
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| ())
        .unwrap();

    // This doesn't handle hidpi screens. You can get the device pixel ratio (ratio of device
    // pixels to HTML pixels) via `window.device_pixel_ratio()` if you want better support on hidpi
    // screens.
    canvas.set_width(size.x() as u32);
    canvas.set_height(size.y() as u32);

    let context = canvas
        // Pathfinder depends on WebGL 2, for GLES 3.
        .get_context("webgl2")
        .unwrap()
        .unwrap()
        .dyn_into::<web_sys::WebGl2RenderingContext>()
        .unwrap();

    let glow_context = glow::Context::from_webgl2_context(context);
    let render_loop = glow::RenderLoop::from_request_animation_frame();

    (render_loop, glow_context)
}

pub fn start() {
    let size = Vector2I::new(640, 480);

    // Platform-specific initialization
    #[cfg(not(target_arch = "wasm32"))]
    let (render_loop, glow_context, _gl_context, mut event_loop) = init_sdl(&size);

    #[cfg(target_arch = "wasm32")]
    let (render_loop, glow_context) = init_web(&size);

    // Create a Pathfinder renderer.
    let mut renderer = Renderer::new(
        GLOWDevice::new(glow_context),
        // We include the resources in the binary to get away with the fact that
        // wasm32-unknown-unknown does not have a filesystem.
        &static_resource_loader::StaticResourceLoader,
        DestFramebuffer::full_window(size),
        RendererOptions {
            background_color: Some(ColorF::white()),
        },
    );

    // Make a canvas. We're going to draw a house.
    let mut canvas =
        CanvasRenderingContext2D::new(CanvasFontContext::from_system_source(), size.to_f32());

    // Set line width.
    canvas.set_line_width(10.0);

    // Draw walls.
    canvas.stroke_rect(RectF::new(
        Vector2F::new(75.0, 140.0),
        Vector2F::new(150.0, 110.0),
    ));

    // Draw door.
    canvas.fill_rect(RectF::new(
        Vector2F::new(130.0, 190.0),
        Vector2F::new(40.0, 60.0),
    ));

    // Draw roof.
    let mut path = Path2D::new();
    path.move_to(Vector2F::new(50.0, 140.0));
    path.line_to(Vector2F::new(150.0, 60.0));
    path.line_to(Vector2F::new(250.0, 140.0));
    path.close_path();
    canvas.stroke_path(path);

    let scene = canvas.into_scene();

    // The render loop applies to both SDL & web.
    // On the web, this is an animation frame.
    render_loop.run(move |running: &mut bool| {
        // Event processing is platform-specific.
        #[cfg(not(target_arch = "wasm32"))]
        for event in event_loop.poll_iter() {
            match event {
                sdl2::event::Event::Quit { .. }
                | sdl2::event::Event::KeyDown {
                    keycode: Some(sdl2::keyboard::Keycode::Escape),
                    ..
                } => *running = false,
                _ => {}
            }
        }

        // Do a full render every frame.
        //
        // Note that here, we're rendering directly, instead of using SceneProxy & RayonExecutor,
        // because threading doesn't work in wasm32-unknown-unknown.
        //
        // You can use GLOW + native SDL + SceneProxy + RayonExecutor, though. See
        // examples/canvas_minimal/src/main.rs for an example that uses SceneProxy + RayonExecutor.
        renderer.begin_scene();
        let commands = Arc::new(Mutex::new(Vec::new()));
        let write = commands.clone();
        scene.build(
            BuildOptions::default(),
            Box::new(move |cmd: RenderCommand| {
                write.lock().unwrap().push(cmd);
            }),
            &SequentialExecutor {},
        );
        for cmd in commands.lock().unwrap().iter() {
            renderer.render_command(cmd);
        }
        renderer.end_scene();
    });
}

// The entry point for the web app.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn entry() {
    start();
}
