// pathfinder/path-utils/src/curve.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Geometry utilities for quadratic Bézier curves.

use euclid::approxeq::ApproxEq;
use euclid::Point2D;
use std::f32;

use PathCommand;
use intersection::Intersect;
use line::Line;

/// A quadratic Bézier curve.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Curve {
    /// The start and end points of the curve, respectively.
    pub endpoints: [Point2D<f32>; 2],
    /// The control point of the curve.
    pub control_point: Point2D<f32>,
}

impl Curve {
    /// Creates a new quadratic Bézier curve from the given endpoints and control point.
    #[inline]
    pub fn new(endpoint_0: &Point2D<f32>, control_point: &Point2D<f32>, endpoint_1: &Point2D<f32>)
               -> Curve {
        Curve {
            endpoints: [*endpoint_0, *endpoint_1],
            control_point: *control_point,
        }
    }

    /// Returns the curve point at time `t` (0.0 to 1.0).
    #[inline]
    pub fn sample(&self, t: f32) -> Point2D<f32> {
        let (p0, p1, p2) = (&self.endpoints[0], &self.control_point, &self.endpoints[1]);
        Point2D::lerp(&p0.lerp(*p1, t), p1.lerp(*p2, t), t)
    }

    /// De Casteljau subdivides this curve into two at time `t` (0.0 to 1.0).
    /// 
    /// Returns the two resulting curves.
    #[inline]
    pub fn subdivide(&self, t: f32) -> (Curve, Curve) {
        let (p0, p1, p2) = (&self.endpoints[0], &self.control_point, &self.endpoints[1]);
        let (ap1, bp1) = (p0.lerp(*p1, t), p1.lerp(*p2, t));
        let ap2bp0 = ap1.lerp(bp1, t);
        (Curve::new(p0, &ap1, &ap2bp0), Curve::new(&ap2bp0, &bp1, p2))
    }

    /// Divides this curve into two at the point with *x* coordinate equal to `x`.
    /// 
    /// Results are undefined if there is not exactly one point on the curve with *x* coordinate
    /// equal to `x`.
    pub fn subdivide_at_x(&self, x: f32) -> (Curve, Curve) {
        let (prev_part, next_part) = self.subdivide(self.solve_t_for_x(x));
        if self.endpoints[0].x <= self.endpoints[1].x {
            (prev_part, next_part)
        } else {
            (next_part, prev_part)
        }
    }

    /// A convenience method that constructs a `CurveTo` path command from this curve's control
    /// point and second endpoint.
    #[inline]
    pub fn to_path_command(&self) -> PathCommand {
        PathCommand::CurveTo(self.control_point, self.endpoints[1])
    }

    /// Returns the times at which the derivative of the curve becomes 0 with respect to *x* and
    /// *y* in that order.
    pub fn inflection_points(&self) -> (Option<f32>, Option<f32>) {
        let inflection_point_x = Curve::inflection_point_x(self.endpoints[0].x,
                                                           self.control_point.x,
                                                           self.endpoints[1].x);
        let inflection_point_y = Curve::inflection_point_x(self.endpoints[0].y,
                                                           self.control_point.y,
                                                           self.endpoints[1].y);
        (inflection_point_x, inflection_point_y)
    }

    /// Returns the time of the single point on this curve with *x* coordinate equal to `x`.
    /// 
    /// Internally, this method uses the [Citardauq Formula] to avoid precision problems.
    /// 
    /// If there is not exactly one point with *x* coordinate equal to `x`, the results are
    /// undefined.
    ///
    /// [Citardauq Formula]: https://math.stackexchange.com/a/311397
    pub fn solve_t_for_x(&self, x: f32) -> f32 {
        let p0x = self.endpoints[0].x as f64;
        let p1x = self.control_point.x as f64;
        let p2x = self.endpoints[1].x as f64;
        let x = x as f64;

        let a = p0x - 2.0 * p1x + p2x;
        let b = -2.0 * p0x + 2.0 * p1x;
        let c = p0x - x;

        let t = 2.0 * c / (-b - (b * b - 4.0 * a * c).sqrt());
        t.max(0.0).min(1.0) as f32
    }

    /// A convenience method that returns the *y* coordinate of the single point on this curve with
    /// *x* coordinate equal to `x`.
    /// 
    /// Results are undefined if there is not exactly one point with *x* coordinate equal to `x`.
    #[inline]
    pub fn solve_y_for_x(&self, x: f32) -> f32 {
        self.sample(self.solve_t_for_x(x)).y
    }

    /// Returns a line segment from the start endpoint of this curve to the end of this curve.
    #[inline]
    pub fn baseline(&self) -> Line {
        Line::new(&self.endpoints[0], &self.endpoints[1])
    }

    #[inline]
    fn inflection_point_x(endpoint_x_0: f32, control_point_x: f32, endpoint_x_1: f32)
                          -> Option<f32> {
        let num = endpoint_x_0 - control_point_x;
        let denom = endpoint_x_0 - 2.0 * control_point_x + endpoint_x_1;
        let t = num / denom;
        if t > f32::approx_epsilon() && t < 1.0 - f32::approx_epsilon() {
            Some(t)
        } else {
            None
        }
    }

    /// Returns the point of intersection of this curve with the given curve.
    #[inline]
    pub fn intersect<T>(&self, other: &T) -> Option<Point2D<f32>> where T: Intersect {
        <Curve as Intersect>::intersect(self, other)
    }
}
