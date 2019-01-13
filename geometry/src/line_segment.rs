// pathfinder/geometry/src/line_segment.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Line segment types, optimized with SIMD.

use crate::point::Point2DF32;
use crate::simd::F32x4;
use crate::util;
use std::ops::Sub;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct LineSegmentF32(pub F32x4);

impl LineSegmentF32 {
    #[inline]
    pub fn new(from: &Point2DF32, to: &Point2DF32) -> LineSegmentF32 {
        LineSegmentF32(from.0.as_f64x2().interleave(to.0.as_f64x2()).0.as_f32x4())
    }

    #[inline]
    pub fn from(&self) -> Point2DF32 {
        Point2DF32(self.0)
    }

    #[inline]
    pub fn to(&self) -> Point2DF32 {
        Point2DF32(self.0.zwxy())
    }

    #[inline]
    pub fn set_from(&mut self, point: &Point2DF32) {
        self.0 = point.0.as_f64x2().combine_low_high(self.0.as_f64x2()).as_f32x4()
    }

    #[inline]
    pub fn set_to(&mut self, point: &Point2DF32) {
        self.0 = self.0.as_f64x2().interleave(point.0.as_f64x2()).0.as_f32x4()
    }

    #[allow(clippy::wrong_self_convention)]
    #[inline]
    pub fn from_x(&self) -> f32 {
        self.0[0]
    }

    #[allow(clippy::wrong_self_convention)]
    #[inline]
    pub fn from_y(&self) -> f32 {
        self.0[1]
    }

    #[inline]
    pub fn to_x(&self) -> f32 {
        self.0[2]
    }

    #[inline]
    pub fn to_y(&self) -> f32 {
        self.0[3]
    }

    #[inline]
    pub fn scale(&self, factor: f32) -> LineSegmentF32 {
        LineSegmentF32(self.0 * F32x4::splat(factor))
    }

    #[inline]
    pub fn split(&self, t: f32) -> (LineSegmentF32, LineSegmentF32) {
        debug_assert!(t >= 0.0 && t <= 1.0);
        let (from_from, to_to) = (self.0.xyxy(), self.0.zwzw());
        let d_d = to_to - from_from;
        let mid_mid = from_from + d_d * F32x4::splat(t);
        (LineSegmentF32(from_from.as_f64x2().interleave(mid_mid.as_f64x2()).0.as_f32x4()),
            LineSegmentF32(mid_mid.as_f64x2().interleave(to_to.as_f64x2()).0.as_f32x4()))
    }

    // Returns the upper segment first, followed by the lower segment.
    #[inline]
    pub fn split_at_y(&self, y: f32) -> (LineSegmentF32, LineSegmentF32) {
        let (min_part, max_part) = self.split(self.solve_t_for_y(y));
        if min_part.from_y() < max_part.from_y() {
            (min_part, max_part)
        } else {
            (max_part, min_part)
        }
    }

    #[inline]
    pub fn solve_t_for_x(&self, x: f32) -> f32 {
        (x - self.from_x()) / (self.to_x() - self.from_x())
    }

    #[inline]
    pub fn solve_t_for_y(&self, y: f32) -> f32 {
        (y - self.from_y()) / (self.to_y() - self.from_y())
    }

    #[inline]
    pub fn solve_y_for_x(&self, x: f32) -> f32 {
        util::lerp(self.from_y(), self.to_y(), self.solve_t_for_x(x))
    }

    #[inline]
    pub fn reversed(&self) -> LineSegmentF32 {
        LineSegmentF32(self.0.zwxy())
    }

    #[inline]
    pub fn upper_point(&self) -> Point2DF32 {
        if self.from_y() < self.to_y() {
            self.from()
        } else {
            self.to()
        }
    }

    #[inline]
    pub fn min_y(&self) -> f32 {
        f32::min(self.from_y(), self.to_y())
    }

    #[inline]
    pub fn max_y(&self) -> f32 {
        f32::max(self.from_y(), self.to_y())
    }

    #[inline]
    pub fn y_winding(&self) -> i32 {
        if self.from_y() < self.to_y() {
            1
        } else {
            -1
        }
    }

    // Reverses if necessary so that the from point is above the to point. Calling this method
    // again will undo the transformation.
    #[inline]
    pub fn orient(&self, y_winding: i32) -> LineSegmentF32 {
        if y_winding >= 0 {
            *self
        } else {
            self.reversed()
        }
    }
}

impl Sub<Point2DF32> for LineSegmentF32 {
    type Output = LineSegmentF32;
    #[inline]
    fn sub(self, point: Point2DF32) -> LineSegmentF32 {
        LineSegmentF32(self.0 - point.0.xyxy())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LineSegmentU4(pub u16);

#[derive(Clone, Copy, Debug)]
pub struct LineSegmentU8(pub u32);
