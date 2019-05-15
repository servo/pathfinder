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
use pathfinder_geometry::basic::point::Point2DF32;
use pathfinder_geometry::basic::rect::RectF32;
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_geometry::color::ColorU;
use pathfinder_geometry::outline::{Contour, Outline};
use pathfinder_geometry::stroke::{LineCap, LineJoin, OutlineStrokeToFill, StrokeStyle};
use pathfinder_renderer::paint::Paint;
use pathfinder_renderer::scene::{PathObject, Scene};
use pathfinder_text::{SceneExt, TextRenderMode};
use skribo::{FontCollection, FontFamily, TextStyle};
use std::borrow::Cow;
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
    font_source: SystemSource,
    #[allow(dead_code)]
    default_font_collection: Arc<FontCollection>,
}

impl CanvasRenderingContext2D {
    #[inline]
    pub fn new(size: Point2DF32) -> CanvasRenderingContext2D {
        let mut scene = Scene::new();
        scene.set_view_box(RectF32::new(Point2DF32::default(), size));
        CanvasRenderingContext2D::from_scene(scene)
    }

    pub fn from_scene(scene: Scene) -> CanvasRenderingContext2D {
        // TODO(pcwalton): Allow the user to cache this?
        let font_source = SystemSource::new();

        let mut default_font_collection = FontCollection::new();
        let default_font =
            font_source.select_best_match(&[FamilyName::SansSerif], &Properties::new())
                       .expect("Failed to select the default font!")
                       .load()
                       .expect("Failed to load the default font!");
        default_font_collection.add_family(FontFamily::new_from_font(default_font));
        let default_font_collection = Arc::new(default_font_collection);

        CanvasRenderingContext2D {
            scene,
            current_state: State::default(default_font_collection.clone()),
            saved_states: vec![],

            font_source,
            default_font_collection,
        }
    }

    #[inline]
    pub fn into_scene(self) -> Scene {
        self.scene
    }

    #[inline]
    pub fn fill_rect(&mut self, rect: RectF32) {
        let mut path = Path2D::new();
        path.rect(rect);
        self.fill_path(path);
    }

    #[inline]
    pub fn stroke_rect(&mut self, rect: RectF32) {
        let mut path = Path2D::new();
        path.rect(rect);
        self.stroke_path(path);
    }

    pub fn fill_text(&mut self, string: &str, position: Point2DF32) {
        // TODO(pcwalton): Report errors.
        let paint_id = self.scene.push_paint(&self.current_state.fill_paint);
        let transform = Transform2DF32::from_translation(position).post_mul(&self.current_state
                                                                                 .transform);
        drop(self.scene.push_text(string,
                                  &TextStyle { size: self.current_state.font_size },
                                  &self.current_state.font_collection,
                                  &transform,
                                  TextRenderMode::Fill,
                                  HintingOptions::None,
                                  paint_id));
    }

    pub fn stroke_text(&mut self, string: &str, position: Point2DF32) {
        // TODO(pcwalton): Report errors.
        let paint_id = self.scene.push_paint(&self.current_state.stroke_paint);
        let transform = Transform2DF32::from_translation(position).post_mul(&self.current_state
                                                                                 .transform);
        drop(self.scene.push_text(string,
                                  &TextStyle { size: self.current_state.font_size },
                                  &self.current_state.font_collection,
                                  &transform,
                                  TextRenderMode::Stroke(self.current_state.stroke_style),
                                  HintingOptions::None,
                                  paint_id));
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
    pub fn set_fill_style(&mut self, new_fill_style: Paint) {
        self.current_state.fill_paint = new_fill_style;
    }

    #[inline]
    pub fn set_stroke_style(&mut self, new_stroke_style: Paint) {
        self.current_state.stroke_paint = new_stroke_style;
    }

    // Text styles

    #[inline]
    pub fn set_font_size(&mut self, new_font_size: f32) {
        self.current_state.font_size = new_font_size;
    }

    // Drawing paths

    #[inline]
    pub fn fill_path(&mut self, path: Path2D) {
        let mut outline = path.into_outline();
        outline.transform(&self.current_state.transform);

        let paint = self.current_state.resolve_paint(&self.current_state.fill_paint);
        let paint_id = self.scene.push_paint(&paint);

        self.scene.push_path(PathObject::new(outline, paint_id, String::new()))
    }

    #[inline]
    pub fn stroke_path(&mut self, path: Path2D) {
        let paint = self.current_state.resolve_paint(&self.current_state.stroke_paint);
        let paint_id = self.scene.push_paint(&paint);

        let mut stroke_style = self.current_state.stroke_style;
        stroke_style.line_width = f32::max(stroke_style.line_width, HAIRLINE_STROKE_WIDTH);

        let mut stroke_to_fill = OutlineStrokeToFill::new(path.into_outline(), stroke_style);
        stroke_to_fill.offset();
        stroke_to_fill.outline.transform(&self.current_state.transform);
        self.scene.push_path(PathObject::new(stroke_to_fill.outline, paint_id, String::new()))
    }

    // Transformations

    #[inline]
    pub fn current_transform(&self) -> Transform2DF32 {
        self.current_state.transform
    }

    #[inline]
    pub fn set_current_transform(&mut self, new_transform: &Transform2DF32) {
        self.current_state.transform = *new_transform;
    }

    #[inline]
    pub fn reset_transform(&mut self) {
        self.current_state.transform = Transform2DF32::default();
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
    transform: Transform2DF32,
    font_collection: Arc<FontCollection>,
    font_size: f32,
    fill_paint: Paint,
    stroke_paint: Paint,
    stroke_style: StrokeStyle,
    global_alpha: f32,
}

impl State {
    fn default(default_font_collection: Arc<FontCollection>) -> State {
        State {
            transform: Transform2DF32::default(),
            font_collection: default_font_collection,
            font_size: DEFAULT_FONT_SIZE,
            fill_paint: Paint { color: ColorU::black() },
            stroke_paint: Paint { color: ColorU::black() },
            stroke_style: StrokeStyle::default(),
            global_alpha: 1.0,
        }
    }

    fn resolve_paint<'p>(&self, paint: &'p Paint) -> Cow<'p, Paint> {
        if self.global_alpha == 1.0 {
            return Cow::Borrowed(paint);
        }

        let mut paint = (*paint).clone();
        match paint {
            Paint::Color(ref mut color) => {
                color.a = (color.a as f32 * self.global_alpha).round() as u8;
            }
            Paint::LinearGradient(ref mut gradient) => {
                // TODO(pcwalton)
            }
        }
        Cow::Owned(paint)
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
    pub fn move_to(&mut self, to: Point2DF32) {
        // TODO(pcwalton): Cull degenerate contours.
        self.flush_current_contour();
        self.current_contour.push_endpoint(to);
    }

    #[inline]
    pub fn line_to(&mut self, to: Point2DF32) {
        self.current_contour.push_endpoint(to);
    }

    #[inline]
    pub fn quadratic_curve_to(&mut self, ctrl: Point2DF32, to: Point2DF32) {
        self.current_contour.push_quadratic(ctrl, to);
    }

    #[inline]
    pub fn bezier_curve_to(&mut self, ctrl0: Point2DF32, ctrl1: Point2DF32, to: Point2DF32) {
        self.current_contour.push_cubic(ctrl0, ctrl1, to);
    }

    #[inline]
    pub fn arc(&mut self, center: Point2DF32, radius: f32, start_angle: f32, end_angle: f32) {
        self.current_contour.push_arc(center, radius, start_angle, end_angle);
    }

    pub fn rect(&mut self, rect: RectF32) {
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
