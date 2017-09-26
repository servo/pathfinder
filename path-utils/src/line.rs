// pathfinder/path-utils/src/line.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Geometry utilities for straight line segments.

use euclid::approxeq::ApproxEq;
use euclid::{Point2D, Vector2D};

use intersection::Intersect;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Line {
    pub endpoints: [Point2D<f32>; 2],
}

impl Line {
    #[inline]
    pub fn new(endpoint_0: &Point2D<f32>, endpoint_1: &Point2D<f32>) -> Line {
        Line {
            endpoints: [*endpoint_0, *endpoint_1],
        }
    }

    #[inline]
    pub fn sample(&self, t: f32) -> Point2D<f32> {
        self.endpoints[0].lerp(self.endpoints[1], t)
    }

    #[inline]
    pub fn solve_t_for_x(&self, x: f32) -> f32 {
        let x_span = self.endpoints[1].x - self.endpoints[0].x;
        if x_span != 0.0 {
            (x - self.endpoints[0].x) / x_span
        } else {
            0.0
        }
    }

    #[inline]
    pub fn solve_y_for_x(&self, x: f32) -> f32 {
        self.sample(self.solve_t_for_x(x)).y
    }

    #[inline]
    pub fn subdivide(&self, t: f32) -> (Line, Line) {
        let midpoint = self.sample(t);
        (Line::new(&self.endpoints[0], &midpoint), Line::new(&midpoint, &self.endpoints[1]))
    }

    pub fn subdivide_at_x(&self, x: f32) -> (Line, Line) {
        let (prev_part, next_part) = self.subdivide(self.solve_t_for_x(x));
        if self.endpoints[0].x <= self.endpoints[1].x {
            (prev_part, next_part)
        } else {
            (next_part, prev_part)
        }
    }

    #[inline]
    pub fn side(&self, point: &Point2D<f32>) -> f32 {
        self.to_vector().cross(*point - self.endpoints[0])
    }

    #[inline]
    pub(crate) fn to_vector(&self) -> Vector2D<f32> {
        self.endpoints[1] - self.endpoints[0]
    }

    #[inline]
    pub fn intersect<T>(&self, other: &T) -> Option<Point2D<f32>> where T: Intersect {
        <Line as Intersect>::intersect(self, other)
    }

    /// A faster version of `intersect` for the special case of two lines.
    ///
    /// https://stackoverflow.com/a/565282
    pub fn intersect_with_line(&self, other: &Line) -> Option<Point2D<f32>> {
        let (p, r) = (self.endpoints[0], self.to_vector());
        let (q, s) = (self.endpoints[1], other.to_vector());

        let rs = r.cross(s);
        if rs.approx_eq(&0.0) {
            return None
        }

        let t = (q - p).cross(s) / rs;
        if t < f32::approx_epsilon() || t > 1.0f32 - f32::approx_epsilon() {
            return None
        }

        let u = (q - p).cross(r) / rs;
        if u < f32::approx_epsilon() || u > 1.0f32 - f32::approx_epsilon() {
            return None
        }

        Some(p + r * t)
    }
}
