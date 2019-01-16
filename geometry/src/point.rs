// pathfinder/geometry/src/point.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A SIMD-optimized point type.

use crate::simd::F32x4;
use euclid::Point2D;
use std::ops::{Add, Mul, Sub};

// 2D points.

#[derive(Clone, Copy, Debug, Default)]
pub struct Point2DF32(pub F32x4);

impl Point2DF32 {
    #[inline]
    pub fn new(x: f32, y: f32) -> Point2DF32 {
        Point2DF32(F32x4::new(x, y, 0.0, 0.0))
    }

    #[inline]
    pub fn splat(value: f32) -> Point2DF32 {
        Point2DF32(F32x4::splat(value))
    }

    #[inline]
    pub fn from_euclid(point: Point2D<f32>) -> Point2DF32 {
        Point2DF32::new(point.x, point.y)
    }

    #[inline]
    pub fn as_euclid(&self) -> Point2D<f32> {
        Point2D::new(self.0[0], self.0[1])
    }

    // TODO(pcwalton): Optimize this!
    #[inline]
    pub fn to_4d(self) -> Point4DF32 {
        Point4DF32::new(self.0[0], self.0[1], 0.0, 1.0)
    }

    #[inline]
    pub fn x(&self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.0[1]
    }

    #[inline]
    pub fn min(&self, other: Point2DF32) -> Point2DF32 {
        Point2DF32(self.0.min(other.0))
    }

    #[inline]
    pub fn max(&self, other: Point2DF32) -> Point2DF32 {
        Point2DF32(self.0.max(other.0))
    }
}

impl PartialEq for Point2DF32 {
    #[inline]
    fn eq(&self, other: &Point2DF32) -> bool {
        let results = self.0.packed_eq(other.0);
        results[0] != 0 && results[1] != 0
    }
}

impl Add<Point2DF32> for Point2DF32 {
    type Output = Point2DF32;
    #[inline]
    fn add(self, other: Point2DF32) -> Point2DF32 {
        Point2DF32(self.0 + other.0)
    }
}

impl Sub<Point2DF32> for Point2DF32 {
    type Output = Point2DF32;
    #[inline]
    fn sub(self, other: Point2DF32) -> Point2DF32 {
        Point2DF32(self.0 - other.0)
    }
}

impl Mul<Point2DF32> for Point2DF32 {
    type Output = Point2DF32;
    #[inline]
    fn mul(self, other: Point2DF32) -> Point2DF32 {
        Point2DF32(self.0 * other.0)
    }
}

// 3D homogeneous points.

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Point4DF32(pub F32x4);

impl Point4DF32 {
    #[inline]
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Point4DF32 {
        Point4DF32(F32x4::new(x, y, z, w))
    }

    #[inline]
    pub fn splat(value: f32) -> Point4DF32 {
        Point4DF32(F32x4::splat(value))
    }

    #[inline]
    pub fn to_2d(self) -> Point2DF32 {
        Point2DF32(self.0)
    }

    #[inline]
    pub fn x(self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub fn y(self) -> f32 {
        self.0[1]
    }

    #[inline]
    pub fn z(self) -> f32 {
        self.0[2]
    }

    #[inline]
    pub fn w(self) -> f32 {
        self.0[3]
    }

    #[inline]
    pub fn perspective_divide(self) -> Point4DF32 {
        self * Point4DF32::splat(1.0 / self.w())
    }
}

impl Mul<Point4DF32> for Point4DF32 {
    type Output = Point4DF32;
    #[inline]
    fn mul(self, other: Point4DF32) -> Point4DF32 {
        Point4DF32(self.0 * other.0)
    }
}
