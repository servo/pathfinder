// pathfinder/path-utils/src/intersection.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Intersections of two segments.

use euclid::approxeq::ApproxEq;
use euclid::Point2D;

use curve::Curve;
use line::Line;
use {lerp, sign};

const MAX_ITERATIONS: u8 = 32;

pub trait Intersect {
    fn min_x(&self) -> f32;
    fn max_x(&self) -> f32;
    fn solve_y_for_x(&self, t: f32) -> f32;

    /// Requires that any curves be monotonic. (See the `monotonic` module for that.)
    ///
    /// This should work for line segments, but it is inefficient.
    ///
    /// This algorithm used to be smarter (based on implicitization) but floating point error
    /// forced the adoption of this simpler, but slower, technique. Improvements are welcome.
    fn intersect<T>(&self, other: &T) -> Option<Point2D<f32>> where T: Intersect {
        let mut min_x = f32::max(self.min_x(), other.min_x());
        let mut max_x = f32::min(self.max_x(), other.max_x());
        let mut iteration = 0;

        while iteration < MAX_ITERATIONS && max_x - min_x > f32::approx_epsilon() {
            iteration += 1;

            let mid_x = lerp(min_x, max_x, 0.5);
            let min_sign = sign(self.solve_y_for_x(min_x) - other.solve_y_for_x(min_x));
            let mid_sign = sign(self.solve_y_for_x(mid_x) - other.solve_y_for_x(mid_x));
            let max_sign = sign(self.solve_y_for_x(max_x) - other.solve_y_for_x(max_x));

            if min_sign == mid_sign && mid_sign != max_sign {
                min_x = mid_x
            } else if min_sign != mid_sign && mid_sign == max_sign {
                max_x = min_x
            } else {
                break
            }
        }

        let mid_x = lerp(min_x, max_x, 0.5);
        Some(Point2D::new(mid_x, self.solve_y_for_x(mid_x)))
    }
}

impl Intersect for Line {
    #[inline]
    fn min_x(&self) -> f32 {
        f32::min(self.endpoints[0].x, self.endpoints[1].x)
    }

    #[inline]
    fn max_x(&self) -> f32 {
        f32::max(self.endpoints[0].x, self.endpoints[1].x)
    }

    #[inline]
    fn solve_y_for_x(&self, x: f32) -> f32 {
        Line::solve_y_for_x(self, x)
    }
}

impl Intersect for Curve {
    #[inline]
    fn min_x(&self) -> f32 {
        f32::min(self.endpoints[0].x, self.endpoints[1].x)
    }

    #[inline]
    fn max_x(&self) -> f32 {
        f32::max(self.endpoints[0].x, self.endpoints[1].x)
    }

    #[inline]
    fn solve_y_for_x(&self, x: f32) -> f32 {
        Curve::solve_y_for_x(self, x)
    }
}
