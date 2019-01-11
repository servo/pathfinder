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

use crate::SimdImpl;
use crate::point::Point2DF32;
use crate::util;
use simdeez::Simd;
use std::ops::Sub;

#[derive(Clone, Copy, Debug)]
pub struct LineSegmentF32(pub <SimdImpl as Simd>::Vf32);

impl LineSegmentF32 {
    #[inline]
    pub fn new(from: &Point2DF32, to: &Point2DF32) -> LineSegmentF32 {
        unsafe {
            LineSegmentF32(SimdImpl::castpd_ps(SimdImpl::unpacklo_pd(
                SimdImpl::castps_pd(from.0),
                SimdImpl::castps_pd(to.0),
            )))
        }
    }

    #[inline]
    pub fn from(&self) -> Point2DF32 {
        unsafe {
            Point2DF32(SimdImpl::castpd_ps(SimdImpl::unpacklo_pd(
                SimdImpl::castps_pd(self.0),
                SimdImpl::setzero_pd(),
            )))
        }
    }

    #[inline]
    pub fn to(&self) -> Point2DF32 {
        unsafe {
            Point2DF32(SimdImpl::castpd_ps(SimdImpl::unpackhi_pd(
                SimdImpl::castps_pd(self.0),
                SimdImpl::setzero_pd(),
            )))
        }
    }

    #[inline]
    pub fn set_from(&mut self, point: &Point2DF32) {
        unsafe {
            let (mut this, point) = (SimdImpl::castps_pd(self.0), SimdImpl::castps_pd(point.0));
            this[0] = point[0];
            self.0 = SimdImpl::castpd_ps(this);
        }
    }

    #[inline]
    pub fn set_to(&mut self, point: &Point2DF32) {
        unsafe {
            let (mut this, point) = (SimdImpl::castps_pd(self.0), SimdImpl::castps_pd(point.0));
            this[1] = point[0];
            self.0 = SimdImpl::castpd_ps(this);
        }
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
        unsafe { LineSegmentF32(SimdImpl::mul_ps(self.0, SimdImpl::set1_ps(factor))) }
    }

    #[inline]
    pub fn split(&self, t: f32) -> (LineSegmentF32, LineSegmentF32) {
        debug_assert!(t >= 0.0 && t <= 1.0);
        unsafe {
            let from_from = SimdImpl::castpd_ps(SimdImpl::unpacklo_pd(
                SimdImpl::castps_pd(self.0),
                SimdImpl::castps_pd(self.0),
            ));
            let to_to = SimdImpl::castpd_ps(SimdImpl::unpackhi_pd(
                SimdImpl::castps_pd(self.0),
                SimdImpl::castps_pd(self.0),
            ));
            let d_d = to_to - from_from;
            let mid_mid = from_from + d_d * SimdImpl::set1_ps(t);
            (
                LineSegmentF32(SimdImpl::castpd_ps(SimdImpl::unpacklo_pd(
                    SimdImpl::castps_pd(from_from),
                    SimdImpl::castps_pd(mid_mid),
                ))),
                LineSegmentF32(SimdImpl::castpd_ps(SimdImpl::unpackhi_pd(
                    SimdImpl::castps_pd(mid_mid),
                    SimdImpl::castps_pd(to_to),
                ))),
            )
        }
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
        unsafe { LineSegmentF32(SimdImpl::shuffle_ps(self.0, self.0, 0b0100_1110)) }
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

impl PartialEq for LineSegmentF32 {
    #[inline]
    fn eq(&self, other: &LineSegmentF32) -> bool {
        unsafe {
            let results = SimdImpl::castps_epi32(SimdImpl::cmpeq_ps(self.0, other.0));
            // FIXME(pcwalton): Is there a better way to do this?
            results[0] == -1 && results[1] == -1 && results[2] == -1 && results[3] == -1
        }
    }
}

impl Default for LineSegmentF32 {
    #[inline]
    fn default() -> LineSegmentF32 {
        unsafe { LineSegmentF32(SimdImpl::setzero_ps()) }
    }
}

impl Sub<Point2DF32> for LineSegmentF32 {
    type Output = LineSegmentF32;
    #[inline]
    fn sub(self, point: Point2DF32) -> LineSegmentF32 {
        unsafe {
            let point_point = SimdImpl::castpd_ps(SimdImpl::unpacklo_pd(
                SimdImpl::castps_pd(point.0),
                SimdImpl::castps_pd(point.0),
            ));
            LineSegmentF32(self.0 - point_point)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LineSegmentU4(pub u16);

#[derive(Clone, Copy, Debug)]
pub struct LineSegmentU8(pub u32);
