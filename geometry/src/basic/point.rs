// pathfinder/geometry/src/basic/point.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A SIMD-optimized point type.

use euclid::Point2D;
use pathfinder_simd::default::F32x4;
use std::ops::{Add, AddAssign, Mul, Sub};

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

    #[inline]
    pub fn to_3d(self) -> Point3DF32 {
        Point3DF32(self.0.concat_xy_xy(F32x4::new(0.0, 1.0, 0.0, 0.0)))
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
    pub fn set_x(&mut self, x: f32) {
        self.0[0] = x;
    }

    #[inline]
    pub fn set_y(&mut self, y: f32) {
        self.0[1] = y;
    }

    #[inline]
    pub fn min(&self, other: Point2DF32) -> Point2DF32 {
        Point2DF32(self.0.min(other.0))
    }

    #[inline]
    pub fn max(&self, other: Point2DF32) -> Point2DF32 {
        Point2DF32(self.0.max(other.0))
    }

    #[inline]
    pub fn clamp(&self, min_val: Point2DF32, max_val: Point2DF32) -> Point2DF32 {
        self.max(min_val).min(max_val)
    }

    #[inline]
    pub fn det(&self, other: Point2DF32) -> f32 {
        self.x() * other.y() - self.y() * other.x()
    }

    #[inline]
    pub fn scale(&self, x: f32) -> Point2DF32 {
        Point2DF32(self.0 * F32x4::splat(x))
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
pub struct Point3DF32(pub F32x4);

impl Point3DF32 {
    #[inline]
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Point3DF32 {
        Point3DF32(F32x4::new(x, y, z, w))
    }

    #[inline]
    pub fn from_euclid_2d(point: &Point2D<f32>) -> Point3DF32 {
        Point3DF32::new(point.x, point.y, 0.0, 1.0)
    }

    #[inline]
    pub fn splat(value: f32) -> Point3DF32 {
        Point3DF32(F32x4::splat(value))
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
    pub fn set_x(&mut self, x: f32) {
        self.0[0] = x
    }

    #[inline]
    pub fn set_y(&mut self, y: f32) {
        self.0[1] = y
    }

    #[inline]
    pub fn set_z(&mut self, z: f32) {
        self.0[2] = z
    }

    #[inline]
    pub fn set_w(&mut self, w: f32) {
        self.0[3] = w
    }

    #[inline]
    pub fn perspective_divide(self) -> Point3DF32 {
        Point3DF32(self.0 * F32x4::splat(1.0 / self.w()))
    }

    #[inline]
    pub fn approx_eq(&self, other: &Point3DF32, epsilon: f32) -> bool {
        self.0.approx_eq(other.0, epsilon)
    }

    /// Checks to see whether this *homogeneous* coordinate equals zero.
    ///
    /// Note that since this treats the coordinate as a homogeneous coordinate, the `w` is ignored.
    // TODO(pcwalton): Optimize with SIMD.
    #[inline]
    pub fn is_zero(self) -> bool {
        self.x() == 0.0 && self.y() == 0.0 && self.z() == 0.0
    }

    #[inline]
    pub fn lerp(self, other: Point3DF32, t: f32) -> Point3DF32 {
        Point3DF32(self.0 + (other.0 - self.0) * F32x4::splat(t))
    }
}

impl Add<Point3DF32> for Point3DF32 {
    type Output = Point3DF32;
    #[inline]
    fn add(self, other: Point3DF32) -> Point3DF32 {
        Point3DF32(self.0 + other.0)
    }
}

impl AddAssign for Point3DF32 {
    #[inline]
    fn add_assign(&mut self, other: Point3DF32) {
        self.0 += other.0
    }
}

impl Mul<Point3DF32> for Point3DF32 {
    type Output = Point3DF32;
    #[inline]
    fn mul(self, other: Point3DF32) -> Point3DF32 {
        Point3DF32(self.0 * other.0)
    }
}
