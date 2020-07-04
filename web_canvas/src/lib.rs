// pathfinder/web_canvas/src/lib.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use css_color_parser::Color;
use pathfinder_canvas::{Canvas, CanvasFontContext, CanvasRenderingContext2D, FillRule, Path2D};
use pathfinder_color::{ColorF, ColorU};
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::{Vector2I, vec2f, vec2i};
use pathfinder_renderer::concurrent::executor::SequentialExecutor;
use pathfinder_renderer::gpu::options::{DestFramebuffer, RendererMode, RendererOptions};
use pathfinder_renderer::gpu::renderer::Renderer;
use pathfinder_renderer::options::BuildOptions;
use pathfinder_resources::embedded::EmbeddedResourceLoader;
use pathfinder_webgl::WebGlDevice;
use std::mem;
use std::str::FromStr;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{self, console, HtmlCanvasElement, WebGl2RenderingContext};

#[wasm_bindgen]
pub struct PFCanvasRenderingContext2D {
    html_canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2D,
    resource_loader: EmbeddedResourceLoader,
    renderer: Option<Renderer<WebGlDevice>>,
    default_path: Path2D,
    // FIXME(pcwalton): Remove this once renderers are resizable.
    prev_framebuffer_size: Vector2I,
}

#[wasm_bindgen(js_name = "createContext")]
pub fn create_context(html_canvas: HtmlCanvasElement) -> PFCanvasRenderingContext2D {
    let context = html_canvas.get_context("webgl2")
                             .unwrap()
                             .unwrap()
                             .dyn_into::<WebGl2RenderingContext>()
                             .unwrap();

    // Get the real size of the window, taking HiDPI into account.
    let framebuffer_size = vec2i(html_canvas.width() as i32, html_canvas.height() as i32);

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
    let renderer = Renderer::new(pathfinder_device, &resource_loader, mode, options);

    // Make a canvas.
    let font_context = CanvasFontContext::from_system_source();
    let context = Canvas::new(framebuffer_size.to_f32()).get_context_2d(font_context);

    PFCanvasRenderingContext2D {
        html_canvas,
        context,
        resource_loader,
        renderer: Some(renderer),
        default_path: Path2D::new(),
        prev_framebuffer_size: framebuffer_size,
    }
}

#[wasm_bindgen]
impl PFCanvasRenderingContext2D {
    pub fn flush(&mut self) {
        // Update framebuffer size.
        let framebuffer_size = vec2i(self.html_canvas.width() as i32,
                                     self.html_canvas.height() as i32);
        if self.prev_framebuffer_size != framebuffer_size {
            // Recreate the Pathfinder renderer.
            //
            // FIXME(pcwalton): This shouldn't be necessary!
            let pathfinder_device = self.renderer.take().unwrap().destroy();
            let mode = RendererMode::default_for_device(&pathfinder_device);
            let options = RendererOptions {
                dest: DestFramebuffer::full_window(framebuffer_size),
                background_color: Some(ColorF::white()),
                ..RendererOptions::default()
            };
            self.renderer = Some(Renderer::new(pathfinder_device,
                                               &self.resource_loader,
                                               mode,
                                               options));
            self.prev_framebuffer_size = framebuffer_size;
        }

        let mut scene = self.context.canvas_mut().take_scene();
        scene.build_and_render(self.renderer.as_mut().unwrap(), 
                               BuildOptions::default(),
                               SequentialExecutor);
    }

    #[wasm_bindgen(getter)]
    pub fn canvas(&self) -> HtmlCanvasElement {
        self.html_canvas.clone()
    }

    #[wasm_bindgen(js_name = "fillStyle")]
    #[wasm_bindgen(setter)]
    pub fn set_fill_style(&mut self, new_style: &str) {
        let css_color = match Color::from_str(new_style) {
            Err(_) => return,
            Ok(css_color) => css_color,
        };
        let color = ColorU::new(css_color.r,
                                css_color.g,
                                css_color.b,
                                (css_color.a * 255.0).round() as u8);
        self.context.set_fill_style(color);
    }

    pub fn save(&mut self) {
        self.context.save();
    }

    pub fn restore(&mut self) {
        self.context.restore();
    }

    #[wasm_bindgen(js_name = "fillRect")]
    pub fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.context.fill_rect(RectF::new(vec2f(x, y), vec2f(width, height)));
    }

    pub fn transform(&mut self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
        let new_transform = self.context.transform() * Transform2F::row_major(a, c, e, b, d, f);
        self.context.set_transform(&new_transform)
    }

    pub fn translate(&mut self, x: f32, y: f32) {
        self.context.translate(vec2f(x, y))
    }

    pub fn scale(&mut self, x: f32, y: f32) {
        self.context.scale(vec2f(x, y))
    }

    pub fn rotate(&mut self, angle: f32) {
        self.context.rotate(angle)
    }

    #[wasm_bindgen(js_name = "beginPath")]
    pub fn begin_path(&mut self) {
        self.default_path = Path2D::new();
    }

    #[wasm_bindgen(js_name = "moveTo")]
    pub fn move_to(&mut self, x: f32, y: f32) {
        self.default_path.move_to(vec2f(x, y))
    }

    #[wasm_bindgen(js_name = "lineTo")]
    pub fn line_to(&mut self, x: f32, y: f32) {
        self.default_path.line_to(vec2f(x, y))
    }

    #[wasm_bindgen(js_name = "bezierCurveTo")]
    pub fn bezier_curve_to(&mut self, cp1x: f32, cp1y: f32, cp2x: f32, cp2y: f32, x: f32, y: f32) {
        self.default_path.bezier_curve_to(vec2f(cp1x, cp1y), vec2f(cp2x, cp2y), vec2f(x, y))
    }

    #[wasm_bindgen(js_name = "quadraticCurveTo")]
    pub fn quadratic_curve_to(&mut self, cpx: f32, cpy: f32, x: f32, y: f32) {
        self.default_path.quadratic_curve_to(vec2f(cpx, cpy), vec2f(x, y))
    }

    #[wasm_bindgen(js_name = "closePath")]
    pub fn close_path(&mut self) {
        self.default_path.close_path();
    }

    pub fn fill(&mut self) {
        let path = self.default_path.clone();
        self.context.fill_path(path, FillRule::Winding);
    }

    pub fn stroke(&mut self) {
        let path = self.default_path.clone();
        self.context.stroke_path(path);
    }
}
