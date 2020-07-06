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
use pathfinder_canvas::{Canvas, CanvasFontContext, CanvasRenderingContext2D, FillRule, FillStyle};
use pathfinder_canvas::{LineCap, Path2D};
use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::{vec2f, vec2i};
use pathfinder_renderer::concurrent::executor::SequentialExecutor;
use pathfinder_renderer::gpu::options::{DestFramebuffer, RendererMode, RendererOptions};
use pathfinder_renderer::gpu::renderer::Renderer;
use pathfinder_renderer::options::BuildOptions;
use pathfinder_resources::embedded::EmbeddedResourceLoader;
use pathfinder_webgl::WebGlDevice;
use std::str::FromStr;
use std::sync::Arc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{self, HtmlCanvasElement, WebGl2RenderingContext};

#[wasm_bindgen]
pub struct PFCanvasRenderingContext2D {
    html_canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2D,
    renderer: Renderer<WebGlDevice>,
    default_path: Path2D,
    current_state: WebCanvasState,
    saved_states: Vec<WebCanvasState>,
}

#[derive(Clone)]
struct WebCanvasState {
    fill_style_string: Arc<String>,
    stroke_style_string: Arc<String>,
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
        background_color: None,
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
        renderer,
        default_path: Path2D::new(),
        current_state: WebCanvasState {
            fill_style_string: Arc::new("black".to_owned()),
            stroke_style_string: Arc::new("black".to_owned()),
        },
        saved_states: vec![],
    }
}

#[wasm_bindgen]
impl PFCanvasRenderingContext2D {
    #[wasm_bindgen(js_name = "pfFlush")]
    pub fn pf_flush(&mut self) {
        // Update framebuffer size.
        let framebuffer_size = vec2i(self.html_canvas.width() as i32,
                                     self.html_canvas.height() as i32);
        self.renderer.options_mut().dest = DestFramebuffer::full_window(framebuffer_size);
        self.renderer.options_mut().background_color = None;
        self.renderer.dest_framebuffer_size_changed();

        // TODO(pcwalton): This is inefficient!
        let mut scene = (*self.context.canvas_mut().scene()).clone();
        scene.build_and_render(&mut self.renderer, BuildOptions::default(), SequentialExecutor);

        self.context.canvas_mut().set_size(framebuffer_size);
    }

    #[wasm_bindgen(js_name = "pfClear")]
    pub fn pf_clear(&mut self) {
        self.context.clear();
    }

    #[wasm_bindgen(getter)]
    pub fn canvas(&self) -> HtmlCanvasElement {
        self.html_canvas.clone()
    }

    // Drawing rectangles

    #[wasm_bindgen(js_name = "clearRect")]
    pub fn clear_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.context.clear_rect(RectF::new(vec2f(x, y), vec2f(width, height)));
    }

    #[wasm_bindgen(js_name = "fillRect")]
    pub fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.context.fill_rect(RectF::new(vec2f(x, y), vec2f(width, height)));
    }

    #[wasm_bindgen(js_name = "strokeRect")]
    pub fn stroke_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.context.stroke_rect(RectF::new(vec2f(x, y), vec2f(width, height)));
    }

    // TODO(pcwalton): Drawing text

    // Line styles

    #[wasm_bindgen(js_name = "lineWidth")]
    #[wasm_bindgen(getter)]
    pub fn line_width(&self) -> f32 {
        self.context.line_width()
    }

    #[wasm_bindgen(js_name = "lineWidth")]
    #[wasm_bindgen(setter)]
    pub fn set_line_width(&mut self, new_line_width: f32) {
        self.context.set_line_width(new_line_width);
    }

    #[wasm_bindgen(js_name = "lineCap")]
    #[wasm_bindgen(getter)]
    pub fn line_cap(&self) -> String {
        match self.context.line_cap() {
            LineCap::Butt => "butt".to_owned(),
            LineCap::Round => "round".to_owned(),
            LineCap::Square => "square".to_owned(),
        }
    }

    #[wasm_bindgen(js_name = "lineCap")]
    #[wasm_bindgen(setter)]
    pub fn set_line_cap(&mut self, new_line_cap: &str) {
        if new_line_cap == "butt" {
            self.context.set_line_cap(LineCap::Butt)
        } else if new_line_cap == "round" {
            self.context.set_line_cap(LineCap::Round)
        } else if new_line_cap == "square" {
            self.context.set_line_cap(LineCap::Square)
        }
    }

    #[wasm_bindgen(js_name = "fillStyle")]
    #[wasm_bindgen(setter)]
    pub fn set_fill_style(&mut self, new_style_string: &str) {
        if let Some(new_style) = parse_fill_or_stroke_style(new_style_string) {
            self.context.set_fill_style(new_style);
            self.current_state.fill_style_string = Arc::new(new_style_string.to_owned());
        }
    }

    #[wasm_bindgen(js_name = "strokeStyle")]
    #[wasm_bindgen(setter)]
    pub fn set_stroke_style(&mut self, new_style_string: &str) {
        if let Some(new_style) = parse_fill_or_stroke_style(new_style_string) {
            self.context.set_stroke_style(new_style);
            self.current_state.stroke_style_string = Arc::new(new_style_string.to_owned());
        }
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

    pub fn save(&mut self) {
        self.context.save();
        self.saved_states.push(self.current_state.clone());
    }

    pub fn restore(&mut self) {
        if let Some(saved_state) = self.saved_states.pop() {
            self.current_state = saved_state;
        }
        self.context.restore();
    }
}

fn parse_fill_or_stroke_style(string: &str) -> Option<FillStyle> {
    let css_color = match Color::from_str(string) {
        Err(_) => return None,
        Ok(css_color) => css_color,
    };
    let color = ColorU::new(css_color.r,
                            css_color.g,
                            css_color.b,
                            (css_color.a * 255.0).round() as u8);
    Some(FillStyle::Color(color))
}
