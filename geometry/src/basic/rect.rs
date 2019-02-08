// pathfinder/geometry/src/basic/rect.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! 2D axis-aligned rectangles, optimized with SIMD.

use crate::basic::point::{Point2DF32, Point2DI32};
use pathfinder_simd::default::{F32x4, I32x4};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RectF32(pub F32x4);

impl RectF32 {
    #[inline]
    pub fn new(origin: Point2DF32, size: Point2DF32) -> RectF32 {
        RectF32(origin.0.concat_xy_xy(origin.0 + size.0))
    }

    #[inline]
    pub fn from_points(origin: Point2DF32, lower_right: Point2DF32) -> RectF32 {
        RectF32(origin.0.concat_xy_xy(lower_right.0))
    }

    #[inline]
    pub fn origin(&self) -> Point2DF32 {
        Point2DF32(self.0)
    }

    #[inline]
    pub fn size(&self) -> Point2DF32 {
        Point2DF32(self.0.zwxy() - self.0.xyxy())
    }

    #[inline]
    pub fn upper_right(&self) -> Point2DF32 {
        Point2DF32(self.0.zyxw())
    }

    #[inline]
    pub fn lower_left(&self) -> Point2DF32 {
        Point2DF32(self.0.xwzy())
    }

    #[inline]
    pub fn lower_right(&self) -> Point2DF32 {
        Point2DF32(self.0.zwxy())
    }

    #[inline]
    pub fn contains_point(&self, point: Point2DF32) -> bool {
        // self.origin <= point && point <= self.lower_right
        self.0.concat_xy_xy(point.0).packed_le(point.0.concat_xy_zw(self.0)).is_all_ones()
    }

    #[inline]
    pub fn contains_rect(&self, other: RectF32) -> bool {
        // self.origin <= other.origin && other.lower_right <= self.lower_right
        self.0.concat_xy_zw(other.0).packed_le(other.0.concat_xy_zw(self.0)).is_all_ones()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.origin() == self.lower_right()
    }

    #[inline]
    pub fn union_point(&self, point: Point2DF32) -> RectF32 {
        RectF32::from_points(self.origin().min(point), self.lower_right().max(point))
    }

    #[inline]
    pub fn union_rect(&self, other: RectF32) -> RectF32 {
        RectF32::from_points(self.origin().min(other.origin()),
                             self.lower_right().max(other.lower_right()))
    }

    #[inline]
    pub fn intersects(&self, other: RectF32) -> bool {
        // self.origin < other.lower_right && other.origin < self.lower_right
        self.0.concat_xy_xy(other.0).packed_lt(other.0.concat_zw_zw(self.0)).is_all_ones()
    }

    #[inline]
    pub fn intersection(&self, other: RectF32) -> Option<RectF32> {
        if !self.intersects(other) {
            None
        } else {
            Some(RectF32::from_points(self.origin().max(other.origin()),
                                      self.lower_right().min(other.lower_right())))
        }
    }

    #[inline]
    pub fn min_x(self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub fn min_y(self) -> f32 {
        self.0[1]
    }

    #[inline]
    pub fn max_x(self) -> f32 {
        self.0[2]
    }

    #[inline]
    pub fn max_y(self) -> f32 {
        self.0[3]
    }

    #[inline]
    pub fn scale_xy(self, factors: Point2DF32) -> RectF32 {
        RectF32(self.0 * factors.0.concat_xy_xy(factors.0))
    }

    #[inline]
    pub fn round_out(self) -> RectF32 {
        RectF32::from_points(self.origin().floor(), self.lower_right().ceil())
    }

    #[inline]
    pub fn dilate(self, amount: Point2DF32) -> RectF32 {
        RectF32::from_points(self.origin() - amount, self.lower_right() + amount)
    }

    #[inline]
    pub fn to_i32(&self) -> RectI32 {
        RectI32(self.0.to_i32x4())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RectI32(pub I32x4);

impl RectI32 {
    #[inline]
    pub fn new(origin: Point2DI32, size: Point2DI32) -> RectI32 {
        RectI32(origin.0.concat_xy_xy(origin.0 + size.0))
    }

    #[inline]
    pub fn from_points(origin: Point2DI32, lower_right: Point2DI32) -> RectI32 {
        RectI32(origin.0.concat_xy_xy(lower_right.0))
    }

    #[inline]
    pub fn origin(&self) -> Point2DI32 {
        Point2DI32(self.0)
    }

    #[inline]
    pub fn size(&self) -> Point2DI32 {
        Point2DI32(self.0.zwxy() - self.0.xyxy())
    }

    #[inline]
    pub fn upper_right(&self) -> Point2DI32 {
        Point2DI32(self.0.zyxw())
    }

    #[inline]
    pub fn lower_left(&self) -> Point2DI32 {
        Point2DI32(self.0.xwzy())
    }

    #[inline]
    pub fn lower_right(&self) -> Point2DI32 {
        Point2DI32(self.0.zwxy())
    }

    #[inline]
    pub fn min_x(self) -> i32 {
        self.0[0]
    }

    #[inline]
    pub fn min_y(self) -> i32 {
        self.0[1]
    }

    #[inline]
    pub fn max_x(self) -> i32 {
        self.0[2]
    }

    #[inline]
    pub fn max_y(self) -> i32 {
        self.0[3]
    }

    #[inline]
    pub fn contains_point(&self, point: Point2DI32) -> bool {
        // self.origin <= point && point <= self.lower_right - 1
        let lower_right = self.lower_right() - Point2DI32::splat(1);
        self.0.concat_xy_xy(point.0).packed_le(point.0.concat_xy_xy(lower_right.0)).is_all_ones()
    }
}
