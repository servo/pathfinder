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
use {det2x2, det3x3, lerp};

pub trait Side {
    fn side(&self, point: &Point2D<f32>) -> f32;
}

pub(crate) trait Intersect {
    fn sample(&self, t: f32) -> Point2D<f32>;

    /// Requires that any curves be monotonic. (See the `monotonic` module for that.)
    ///
    /// This should work for line segments, but it is inefficient.
    ///
    /// See T.W. Sederberg, "Computer Aided Geometric Design Course Notes" § 17.8.
    fn intersect<T>(&self, other: &T) -> Option<Point2D<f32>> where T: Side {
        let (mut t_min, mut t_max) = (0.0, 1.0);
        let mut iteration = 0;
        while t_max - t_min > f32::approx_epsilon() {
            let (p_min, p_max) = (self.sample(t_min), self.sample(t_max));

            let side_min = other.side(&p_min).signum();
            let side_max = other.side(&p_max).signum();
            if iteration == 0 && side_min == side_max {
                return None
            }

            let t_mid = lerp(t_min, t_max, 0.5);
            let p_mid = self.sample(t_mid);
            let side_mid = other.side(&p_mid).signum();

            if side_mid == side_min {
                t_min = t_mid
            } else if side_mid == side_max {
                t_max = t_mid
            } else {
                break
            }

            iteration += 1
        }

        Some(self.sample(lerp(t_min, t_max, 0.5)))
    }
}

impl Side for Line {
    #[inline]
    fn side(&self, point: &Point2D<f32>) -> f32 {
        Line::side(self, point)
    }
}

impl Side for Curve {
    /// See T.W. Sederberg, "Computer Aided Geometric Design Course Notes" § 17.6.1.
    fn side(&self, point: &Point2D<f32>) -> f32 {
        fn l(factor: f32, point: &Point2D<f32>, point_i: &Point2D<f32>, point_j: &Point2D<f32>)
             -> f32 {
            factor * det3x3(&[
                point.x, point.y, 1.0,
                point_i.x, point_i.y, 1.0,
                point_j.x, point_j.y, 1.0,
            ])
        }

        let l20 = l(1.0 * 1.0, point, &self.endpoints[1], &self.endpoints[0]);
        det2x2(&[
            l(2.0 * 1.0, point, &self.endpoints[1], &self.control_point), l20,
            l20, l(2.0 * 1.0, point, &self.control_point, &self.endpoints[0]),
        ])
    }
}

impl Intersect for Line {
    #[inline]
    fn sample(&self, t: f32) -> Point2D<f32> {
        Line::sample(self, t)
    }
}

impl Intersect for Curve {
    #[inline]
    fn sample(&self, t: f32) -> Point2D<f32> {
        Curve::sample(self, t)
    }
}
