// pathfinder/canvas/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A simple API for Pathfinder that mirrors a subset of HTML canvas.

use font_kit::family_name::FamilyName;
use font_kit::hinting::HintingOptions;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;
use pathfinder_geometry::basic::point::Point2DF;
use pathfinder_geometry::basic::rect::RectF;
use pathfinder_geometry::basic::transform2d::Transform2DF;
use pathfinder_geometry::color::ColorU;
use pathfinder_geometry::outline::{Contour, Outline};
use pathfinder_geometry::stroke::{LineCap, LineJoin, OutlineStrokeToFill, StrokeStyle};
use pathfinder_renderer::paint::{Paint, PaintId};
use pathfinder_renderer::scene::{PathObject, Scene};
use pathfinder_text::{SceneExt, TextRenderMode};
use skribo::{FontCollection, FontFamily, Layout, TextStyle};
use std::default::Default;
use std::mem;
use std::sync::Arc;

const HAIRLINE_STROKE_WIDTH: f32 = 0.0333;
const DEFAULT_FONT_SIZE: f32 = 10.0;

pub struct CanvasRenderingContext2D {
    scene: Scene,
    current_state: State,
    saved_states: Vec<State>,
    #[allow(dead_code)]
    font_context: CanvasFontContext,
}

impl CanvasRenderingContext2D {
    #[inline]
    pub fn new(font_context: CanvasFontContext, size: Point2DF) -> CanvasRenderingContext2D {
        let mut scene = Scene::new();
        scene.set_view_box(RectF::new(Point2DF::default(), size));
        CanvasRenderingContext2D::from_scene(font_context, scene)
    }

    pub fn from_scene(font_context: CanvasFontContext, scene: Scene) -> CanvasRenderingContext2D {
        CanvasRenderingContext2D {
            scene,
            current_state: State::default(font_context.default_font_collection.clone()),
            saved_states: vec![],
            font_context,
        }
    }

    #[inline]
    pub fn into_scene(self) -> Scene {
        self.scene
    }

    // Drawing rectangles

    #[inline]
    pub fn fill_rect(&mut self, rect: RectF) {
        let mut path = Path2D::new();
        path.rect(rect);
        self.fill_path(path);
    }

    #[inline]
    pub fn stroke_rect(&mut self, rect: RectF) {
        let mut path = Path2D::new();
        path.rect(rect);
        self.stroke_path(path);
    }

    // Drawing text

    pub fn fill_text(&mut self, string: &str, position: Point2DF) {
        let paint_id = self.scene.push_paint(&self.current_state.fill_paint);
        self.fill_or_stroke_text(string, position, paint_id, TextRenderMode::Fill);
    }

    pub fn stroke_text(&mut self, string: &str, position: Point2DF) {
        let paint_id = self.scene.push_paint(&self.current_state.stroke_paint);
        let render_mode = TextRenderMode::Stroke(self.current_state.stroke_style);
        self.fill_or_stroke_text(string, position, paint_id, render_mode);
    }

    pub fn measure_text(&self, string: &str) -> TextMetrics {
        TextMetrics { width: self.layout_text(string).width() }
    }

    fn fill_or_stroke_text(&mut self,
                           string: &str,
                           mut position: Point2DF,
                           paint_id: PaintId,
                           render_mode: TextRenderMode) {
        let layout = self.layout_text(string);

        match self.current_state.text_align {
            TextAlign::Left => {},
            TextAlign::Right => position.set_x(position.x() - layout.width()),
            TextAlign::Center => position.set_x(position.x() - layout.width() * 0.5),
        }

        let transform = Transform2DF::from_translation(position).post_mul(&self.current_state
                                                                               .transform);

        // TODO(pcwalton): Report errors.
        drop(self.scene.push_layout(&layout,
                                    &TextStyle { size: self.current_state.font_size },
                                    &transform,
                                    render_mode,
                                    HintingOptions::None,
                                    paint_id));
    }

    fn layout_text(&self, string: &str) -> Layout {
        skribo::layout(&TextStyle { size: self.current_state.font_size },
                       &self.current_state.font_collection,
                       string)
    }

    // Line styles

    #[inline]
    pub fn set_line_width(&mut self, new_line_width: f32) {
        self.current_state.stroke_style.line_width = new_line_width
    }

    #[inline]
    pub fn set_line_cap(&mut self, new_line_cap: LineCap) {
        self.current_state.stroke_style.line_cap = new_line_cap
    }

    #[inline]
    pub fn set_line_join(&mut self, new_line_join: LineJoin) {
        self.current_state.stroke_style.line_join = new_line_join
    }

    #[inline]
    pub fn set_fill_style(&mut self, new_fill_style: FillStyle) {
        self.current_state.fill_paint = new_fill_style.to_paint();
    }

    #[inline]
    pub fn set_stroke_style(&mut self, new_stroke_style: FillStyle) {
        self.current_state.stroke_paint = new_stroke_style.to_paint();
    }

    // Text styles

    #[inline]
    pub fn set_font_size(&mut self, new_font_size: f32) {
        self.current_state.font_size = new_font_size;
    }

    #[inline]
    pub fn set_text_align(&mut self, new_text_align: TextAlign) {
        self.current_state.text_align = new_text_align;
    }

    // Drawing paths

    #[inline]
    pub fn fill_path(&mut self, path: Path2D) {
        let mut outline = path.into_outline();
        outline.transform(&self.current_state.transform);

        let paint = self.current_state.resolve_paint(self.current_state.fill_paint);
        let paint_id = self.scene.push_paint(&paint);

        self.scene.push_path(PathObject::new(outline, paint_id, String::new()))
    }

    #[inline]
    pub fn stroke_path(&mut self, path: Path2D) {
        let paint = self.current_state.resolve_paint(self.current_state.stroke_paint);
        let paint_id = self.scene.push_paint(&paint);

        let mut stroke_style = self.current_state.stroke_style;
        stroke_style.line_width = f32::max(stroke_style.line_width, HAIRLINE_STROKE_WIDTH);

        let outline = path.into_outline();
        let mut stroke_to_fill = OutlineStrokeToFill::new(&outline, stroke_style);
        stroke_to_fill.offset();
        let mut outline = stroke_to_fill.into_outline();
        outline.transform(&self.current_state.transform);
        self.scene.push_path(PathObject::new(outline, paint_id, String::new()))
    }

    // Transformations

    #[inline]
    pub fn current_transform(&self) -> Transform2DF {
        self.current_state.transform
    }

    #[inline]
    pub fn set_current_transform(&mut self, new_transform: &Transform2DF) {
        self.current_state.transform = *new_transform;
    }

    #[inline]
    pub fn reset_transform(&mut self) {
        self.current_state.transform = Transform2DF::default();
    }

    // Compositing

    #[inline]
    pub fn global_alpha(&self) -> f32 {
        self.current_state.global_alpha
    }

    #[inline]
    pub fn set_global_alpha(&mut self, new_global_alpha: f32) {
        self.current_state.global_alpha = new_global_alpha;
    }

    // The canvas state

    #[inline]
    pub fn save(&mut self) {
        self.saved_states.push(self.current_state.clone());
    }

    #[inline]
    pub fn restore(&mut self) {
        if let Some(state) = self.saved_states.pop() {
            self.current_state = state;
        }
    }
}

#[derive(Clone)]
pub struct State {
    transform: Transform2DF,
    font_collection: Arc<FontCollection>,
    font_size: f32,
    text_align: TextAlign,
    fill_paint: Paint,
    stroke_paint: Paint,
    stroke_style: StrokeStyle,
    global_alpha: f32,
}

impl State {
    fn default(default_font_collection: Arc<FontCollection>) -> State {
        State {
            transform: Transform2DF::default(),
            font_collection: default_font_collection,
            font_size: DEFAULT_FONT_SIZE,
            text_align: TextAlign::Left,
            fill_paint: Paint { color: ColorU::black() },
            stroke_paint: Paint { color: ColorU::black() },
            stroke_style: StrokeStyle::default(),
            global_alpha: 1.0,
        }
    }

    fn resolve_paint(&self, mut paint: Paint) -> Paint {
        paint.color.a = (paint.color.a as f32 * self.global_alpha).round() as u8;
        paint
    }
}

#[derive(Clone)]
pub struct Path2D {
    outline: Outline,
    current_contour: Contour,
}

// TODO(pcwalton): `ellipse`
impl Path2D {
    #[inline]
    pub fn new() -> Path2D {
        Path2D { outline: Outline::new(), current_contour: Contour::new() }
    }

    #[inline]
    pub fn close_path(&mut self) {
        self.current_contour.close();
    }

    #[inline]
    pub fn move_to(&mut self, to: Point2DF) {
        // TODO(pcwalton): Cull degenerate contours.
        self.flush_current_contour();
        self.current_contour.push_endpoint(to);
    }

    #[inline]
    pub fn line_to(&mut self, to: Point2DF) {
        self.current_contour.push_endpoint(to);
    }

    #[inline]
    pub fn quadratic_curve_to(&mut self, ctrl: Point2DF, to: Point2DF) {
        self.current_contour.push_quadratic(ctrl, to);
    }

    #[inline]
    pub fn bezier_curve_to(&mut self, ctrl0: Point2DF, ctrl1: Point2DF, to: Point2DF) {
        self.current_contour.push_cubic(ctrl0, ctrl1, to);
    }

    #[inline]
    pub fn arc(&mut self, center: Point2DF, radius: f32, start_angle: f32, end_angle: f32) {
        let mut transform = Transform2DF::from_scale(Point2DF::splat(radius));
        transform = transform.post_mul(&Transform2DF::from_translation(center));
        self.current_contour.push_arc(&transform, start_angle, end_angle);
    }

    pub fn rect(&mut self, rect: RectF) {
        self.flush_current_contour();
        self.current_contour.push_endpoint(rect.origin());
        self.current_contour.push_endpoint(rect.upper_right());
        self.current_contour.push_endpoint(rect.lower_right());
        self.current_contour.push_endpoint(rect.lower_left());
        self.current_contour.close();
    }

    fn into_outline(mut self) -> Outline {
        self.flush_current_contour();
        self.outline
    }

    fn flush_current_contour(&mut self) {
        if !self.current_contour.is_empty() {
            self.outline.push_contour(mem::replace(&mut self.current_contour, Contour::new()));
        }
    }
}

// TODO(pcwalton): Gradients.
#[derive(Clone, Copy)]
pub enum FillStyle {
    Color(ColorU),
}

impl FillStyle {
    #[inline]
    fn to_paint(&self) -> Paint {
        match *self { FillStyle::Color(color) => Paint { color } }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlign {
    Left,
    Right,
    Center,
}

// TODO(pcwalton): Support other fields.
#[derive(Clone, Copy, Debug)]
pub struct TextMetrics {
    pub width: f32,
}

#[derive(Clone)]
pub struct CanvasFontContext {
    #[allow(dead_code)]
    font_source: Arc<SystemSource>,
    #[allow(dead_code)]
    default_font_collection: Arc<FontCollection>,
}

impl CanvasFontContext {
    pub fn new() -> CanvasFontContext {
        let font_source = Arc::new(SystemSource::new());

        let mut default_font_collection = FontCollection::new();
        let default_font =
            font_source.select_best_match(&[FamilyName::SansSerif], &Properties::new())
                       .expect("Failed to select the default font!")
                       .load()
                       .expect("Failed to load the default font!");
        default_font_collection.add_family(FontFamily::new_from_font(default_font));
        let default_font_collection = Arc::new(default_font_collection);

        CanvasFontContext {
            font_source,
            default_font_collection,
        }
    }
}

// Text layout utilities

trait LayoutExt {
    fn width(&self) -> f32;
}

impl LayoutExt for Layout {
    fn width(&self) -> f32 {
        let last_glyph = match self.glyphs.last() {
            None => return 0.0,
            Some(last_glyph) => last_glyph,
        };

        let glyph_id = last_glyph.glyph_id;
        let font_metrics = last_glyph.font.font.metrics();
        let glyph_rect = last_glyph.font.font.typographic_bounds(glyph_id).unwrap();
        let scale_factor = self.size / font_metrics.units_per_em as f32;
        last_glyph.offset.x + glyph_rect.max_x() * scale_factor
    }
}
