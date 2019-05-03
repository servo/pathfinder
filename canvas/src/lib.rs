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

use pathfinder_geometry::basic::point::Point2DF32;
use pathfinder_geometry::basic::rect::RectF32;
use pathfinder_geometry::color::ColorU;
use pathfinder_geometry::outline::{Contour, Outline};
use pathfinder_geometry::stroke::OutlineStrokeToFill;
use pathfinder_renderer::scene::{Paint, PathObject, Scene};
use std::mem;

const HAIRLINE_STROKE_WIDTH: f32 = 0.0333;

pub struct CanvasRenderingContext2D {
    scene: Scene,
    current_paint: Paint,
    current_line_width: f32,
}

impl CanvasRenderingContext2D {
    #[inline]
    pub fn new(size: Point2DF32) -> CanvasRenderingContext2D {
        let mut scene = Scene::new();
        scene.set_view_box(RectF32::new(Point2DF32::default(), size));
        CanvasRenderingContext2D::from_scene(scene)
    }

    #[inline]
    pub fn from_scene(scene: Scene) -> CanvasRenderingContext2D {
        CanvasRenderingContext2D {
            scene,
            current_paint: Paint { color: ColorU::black() },
            current_line_width: 1.0,
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

    #[inline]
    pub fn set_line_width(&mut self, new_line_width: f32) {
        self.current_line_width = new_line_width
    }

    #[inline]
    pub fn fill_path(&mut self, path: Path2D) {
        let paint_id = self.scene.push_paint(&self.current_paint);
        self.scene.push_object(PathObject::new(path.into_outline(), paint_id, String::new()))
    }

    #[inline]
    pub fn stroke_path(&mut self, path: Path2D) {
        let paint_id = self.scene.push_paint(&self.current_paint);
        let stroke_width = f32::max(self.current_line_width, HAIRLINE_STROKE_WIDTH);
        let mut stroke_to_fill = OutlineStrokeToFill::new(path.into_outline(), stroke_width);
        stroke_to_fill.offset();
        self.scene.push_object(PathObject::new(stroke_to_fill.outline, paint_id, String::new()))
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
