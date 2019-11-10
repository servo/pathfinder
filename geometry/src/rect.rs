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

use crate::vector::{Vector2F, Vector2I};
use pathfinder_simd::default::{F32x4, I32x4};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RectF(pub F32x4);

impl RectF {
    #[inline]
    pub fn new(origin: Vector2F, size: Vector2F) -> RectF {
        RectF(origin.0.concat_xy_xy(origin.0 + size.0))
    }

    #[inline]
    pub fn from_points(origin: Vector2F, lower_right: Vector2F) -> RectF {
        RectF(origin.0.concat_xy_xy(lower_right.0))
    }

    #[inline]
    pub fn origin(&self) -> Vector2F {
        Vector2F(self.0.xy())
    }

    #[inline]
    pub fn size(&self) -> Vector2F {
        Vector2F(self.0.zw() - self.0.xy())
    }

    #[inline]
    pub fn upper_right(&self) -> Vector2F {
        Vector2F(self.0.zy())
    }

    #[inline]
    pub fn lower_left(&self) -> Vector2F {
        Vector2F(self.0.xw())
    }

    #[inline]
    pub fn lower_right(&self) -> Vector2F {
        Vector2F(self.0.zw())
    }

    #[inline]
    pub fn contains_point(&self, point: Vector2F) -> bool {
        // self.origin <= point && point <= self.lower_right
        let point = point.0.to_f32x4();
        self.0.concat_xy_xy(point).packed_le(point.concat_xy_zw(self.0)).is_all_ones()
    }

    #[inline]
    pub fn contains_rect(&self, other: RectF) -> bool {
        // self.origin <= other.origin && other.lower_right <= self.lower_right
        self.0
            .concat_xy_zw(other.0)
            .packed_le(other.0.concat_xy_zw(self.0))
            .is_all_ones()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.origin() == self.lower_right()
    }

    #[inline]
    pub fn union_point(&self, point: Vector2F) -> RectF {
        RectF::from_points(self.origin().min(point), self.lower_right().max(point))
    }

    #[inline]
    pub fn union_rect(&self, other: RectF) -> RectF {
        RectF::from_points(
            self.origin().min(other.origin()),
            self.lower_right().max(other.lower_right()),
        )
    }

    #[inline]
    pub fn intersects(&self, other: RectF) -> bool {
        // self.origin < other.lower_right && other.origin < self.lower_right
        self.0
            .concat_xy_xy(other.0)
            .packed_lt(other.0.concat_zw_zw(self.0))
            .is_all_ones()
    }

    #[inline]
    pub fn intersection(&self, other: RectF) -> Option<RectF> {
        if !self.intersects(other) {
            None
        } else {
            Some(RectF::from_points(
                self.origin().max(other.origin()),
                self.lower_right().min(other.lower_right()),
            ))
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
    pub fn scale(self, factor: f32) -> RectF {
        RectF(self.0 * F32x4::splat(factor))
    }

    #[inline]
    pub fn scale_xy(self, factors: Vector2F) -> RectF {
        RectF(self.0 * factors.0.concat_xy_xy(factors.0))
    }

    #[inline]
    pub fn round_out(self) -> RectF {
        RectF::from_points(self.origin().floor(), self.lower_right().ceil())
    }

    #[inline]
    pub fn dilate(self, amount: Vector2F) -> RectF {
        RectF::from_points(self.origin() - amount, self.lower_right() + amount)
    }

    #[inline]
    pub fn to_i32(&self) -> RectI {
        RectI(self.0.to_i32x4())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RectI(pub I32x4);

impl RectI {
    #[inline]
    pub fn new(origin: Vector2I, size: Vector2I) -> RectI {
        RectI(origin.0.concat_xy_xy(origin.0 + size.0))
    }

    #[inline]
    pub fn from_points(origin: Vector2I, lower_right: Vector2I) -> RectI {
        RectI(origin.0.concat_xy_xy(lower_right.0))
    }

    #[inline]
    pub fn origin(&self) -> Vector2I {
        Vector2I(self.0.xy())
    }

    #[inline]
    pub fn size(&self) -> Vector2I {
        Vector2I(self.0.zw() - self.0.xy())
    }

    #[inline]
    pub fn upper_right(&self) -> Vector2I {
        Vector2I(self.0.zy())
    }

    #[inline]
    pub fn lower_left(&self) -> Vector2I {
        Vector2I(self.0.xw())
    }

    #[inline]
    pub fn lower_right(&self) -> Vector2I {
        Vector2I(self.0.zw())
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
    pub fn contains_point(&self, point: Vector2I) -> bool {
        // self.origin <= point && point <= self.lower_right - 1
        let lower_right = self.lower_right() - Vector2I::splat(1);
        self.origin()
            .0
            .concat_xy_xy(point.0)
            .packed_le(point.0.concat_xy_xy(lower_right.0))
            .is_all_ones()
    }

    #[inline]
    pub fn to_f32(&self) -> RectF {
        RectF(self.0.to_f32x4())
    }
}
