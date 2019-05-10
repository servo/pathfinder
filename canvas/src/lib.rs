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
use pathfinder_geometry::stroke::OutlineStrokeToFill;
use pathfinder_renderer::scene::{Paint, PathObject, Scene};
use pathfinder_text::{SceneExt, TextRenderMode};
use skribo::{FontCollection, FontFamily, TextStyle};
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
        drop(self.scene.push_text(string,
                                  &TextStyle { size: self.current_state.font_size },
                                  &self.current_state.font_collection,
                                  &Transform2DF32::from_translation(&position),
                                  TextRenderMode::Fill,
                                  HintingOptions::None,
                                  paint_id));
    }

    pub fn stroke_text(&mut self, string: &str, position: Point2DF32) {
        // TODO(pcwalton): Report errors.
        let paint_id = self.scene.push_paint(&self.current_state.stroke_paint);
        drop(self.scene.push_text(string,
                                  &TextStyle { size: self.current_state.font_size },
                                  &self.current_state.font_collection,
                                  &Transform2DF32::from_translation(&position),
                                  TextRenderMode::Stroke(self.current_state.line_width),
                                  HintingOptions::None,
                                  paint_id));
    }

    // Line styles

    #[inline]
    pub fn set_line_width(&mut self, new_line_width: f32) {
        self.current_state.line_width = new_line_width
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

    // Paths

    #[inline]
    pub fn fill_path(&mut self, path: Path2D) {
        let paint_id = self.scene.push_paint(&self.current_state.fill_paint);
        self.scene.push_path(PathObject::new(path.into_outline(), paint_id, String::new()))
    }

    #[inline]
    pub fn stroke_path(&mut self, path: Path2D) {
        let paint_id = self.scene.push_paint(&self.current_state.stroke_paint);
        let stroke_width = f32::max(self.current_state.line_width, HAIRLINE_STROKE_WIDTH);
        let mut stroke_to_fill = OutlineStrokeToFill::new(path.into_outline(), stroke_width);
        stroke_to_fill.offset();
        self.scene.push_path(PathObject::new(stroke_to_fill.outline, paint_id, String::new()))
    }

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
    font_collection: Arc<FontCollection>,
    font_size: f32,
    fill_paint: Paint,
    stroke_paint: Paint,
    line_width: f32,
}

impl State {
    fn default(default_font_collection: Arc<FontCollection>) -> State {
        State {
            font_collection: default_font_collection,
            font_size: DEFAULT_FONT_SIZE,
            fill_paint: Paint { color: ColorU::black() },
            stroke_paint: Paint { color: ColorU::black() },
            line_width: 1.0,
        }
    }
}

#[derive(Clone)]
pub struct Path2D {
    outline: Outline,
    current_contour: Contour,
}

// TODO(pcwalton): `arc`, `ellipse`
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
