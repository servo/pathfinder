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

use euclid::{Point2D, Vector2D};

use intersection::{Intersect, Side};

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
    pub fn side(&self, point: &Point2D<f32>) -> f32 {
        self.to_vector().cross(*point - self.endpoints[0])
    }

    #[inline]
    pub(crate) fn to_vector(&self) -> Vector2D<f32> {
        self.endpoints[1] - self.endpoints[0]
    }

    #[inline]
    pub fn intersect<T>(&self, other: &T) -> Option<Point2D<f32>> where T: Side {
        <Line as Intersect>::intersect(self, other)
    }
}
