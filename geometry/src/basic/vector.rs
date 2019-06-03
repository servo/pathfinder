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

use pathfinder_simd::default::{F32x4, I32x4};
use std::ops::{Add, AddAssign, Mul, Neg, Sub};

/// 2D points with 32-bit floating point coordinates.
#[derive(Clone, Copy, Debug, Default)]
pub struct Vector2F(pub F32x4);

impl Vector2F {
    #[inline]
    pub fn new(x: f32, y: f32) -> Vector2F {
        Vector2F(F32x4::new(x, y, 0.0, 0.0))
    }

    #[inline]
    pub fn splat(value: f32) -> Vector2F {
        Vector2F(F32x4::splat(value))
    }

    #[inline]
    pub fn to_3d(self) -> Vector4F {
        Vector4F(self.0.concat_xy_xy(F32x4::new(0.0, 1.0, 0.0, 0.0)))
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
    pub fn min(&self, other: Vector2F) -> Vector2F {
        Vector2F(self.0.min(other.0))
    }

    #[inline]
    pub fn max(&self, other: Vector2F) -> Vector2F {
        Vector2F(self.0.max(other.0))
    }

    #[inline]
    pub fn clamp(&self, min_val: Vector2F, max_val: Vector2F) -> Vector2F {
        self.max(min_val).min(max_val)
    }

    #[inline]
    pub fn det(&self, other: Vector2F) -> f32 {
        self.x() * other.y() - self.y() * other.x()
    }

    #[inline]
    pub fn dot(&self, other: Vector2F) -> f32 {
        let xy = self.0 * other.0;
        xy.x() + xy.y()
    }

    #[inline]
    pub fn scale(&self, x: f32) -> Vector2F {
        Vector2F(self.0 * F32x4::splat(x))
    }

    #[inline]
    pub fn scale_xy(&self, factors: Vector2F) -> Vector2F {
        Vector2F(self.0 * factors.0)
    }

    #[inline]
    pub fn floor(&self) -> Vector2F {
        Vector2F(self.0.floor())
    }

    #[inline]
    pub fn ceil(&self) -> Vector2F {
        Vector2F(self.0.ceil())
    }

    /// Treats this point as a vector and calculates its squared length.
    #[inline]
    pub fn square_length(&self) -> f32 {
        let squared = self.0 * self.0;
        squared[0] + squared[1]
    }

    /// Treats this point as a vector and calculates its length.
    #[inline]
    pub fn length(&self) -> f32 {
        f32::sqrt(self.square_length())
    }

    /// Treats this point as a vector and normalizes it.
    #[inline]
    pub fn normalize(&self) -> Vector2F {
        self.scale(1.0 / self.length())
    }

    /// Swaps y and x.
    #[inline]
    pub fn yx(&self) -> Vector2F {
        Vector2F(self.0.yxwz())
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        *self == Vector2F::default()
    }

    #[inline]
    pub fn lerp(&self, other: Vector2F, t: f32) -> Vector2F {
        *self + (other - *self).scale(t)
    }

    #[inline]
    pub fn to_i32(&self) -> Vector2I {
        Vector2I(self.0.to_i32x4())
    }
}

impl PartialEq for Vector2F {
    #[inline]
    fn eq(&self, other: &Vector2F) -> bool {
        let results = self.0.packed_eq(other.0);
        results[0] != 0 && results[1] != 0
    }
}

impl Add<Vector2F> for Vector2F {
    type Output = Vector2F;
    #[inline]
    fn add(self, other: Vector2F) -> Vector2F {
        Vector2F(self.0 + other.0)
    }
}

impl Sub<Vector2F> for Vector2F {
    type Output = Vector2F;
    #[inline]
    fn sub(self, other: Vector2F) -> Vector2F {
        Vector2F(self.0 - other.0)
    }
}

impl Mul<Vector2F> for Vector2F {
    type Output = Vector2F;
    #[inline]
    fn mul(self, other: Vector2F) -> Vector2F {
        Vector2F(self.0 * other.0)
    }
}

impl Neg for Vector2F {
    type Output = Vector2F;
    #[inline]
    fn neg(self) -> Vector2F {
        Vector2F(-self.0)
    }
}

/// 2D points with 32-bit signed integer coordinates.
#[derive(Clone, Copy, Debug, Default)]
pub struct Vector2I(pub I32x4);

impl Vector2I {
    #[inline]
    pub fn new(x: i32, y: i32) -> Vector2I {
        Vector2I(I32x4::new(x, y, 0, 0))
    }

    #[inline]
    pub fn splat(value: i32) -> Vector2I {
        Vector2I(I32x4::splat(value))
    }

    #[inline]
    pub fn x(&self) -> i32 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> i32 {
        self.0[1]
    }

    #[inline]
    pub fn set_x(&mut self, x: i32) {
        self.0[0] = x;
    }

    #[inline]
    pub fn set_y(&mut self, y: i32) {
        self.0[1] = y;
    }

    #[inline]
    pub fn scale(&self, factor: i32) -> Vector2I {
        Vector2I(self.0 * I32x4::splat(factor))
    }

    #[inline]
    pub fn scale_xy(&self, factors: Vector2I) -> Vector2I {
        Vector2I(self.0 * factors.0)
    }

    #[inline]
    pub fn to_f32(&self) -> Vector2F {
        Vector2F(self.0.to_f32x4())
    }
}

impl Add<Vector2I> for Vector2I {
    type Output = Vector2I;
    #[inline]
    fn add(self, other: Vector2I) -> Vector2I {
        Vector2I(self.0 + other.0)
    }
}

impl AddAssign<Vector2I> for Vector2I {
    #[inline]
    fn add_assign(&mut self, other: Vector2I) {
        self.0 += other.0
    }
}

impl Sub<Vector2I> for Vector2I {
    type Output = Vector2I;
    #[inline]
    fn sub(self, other: Vector2I) -> Vector2I {
        Vector2I(self.0 - other.0)
    }
}

impl PartialEq for Vector2I {
    #[inline]
    fn eq(&self, other: &Vector2I) -> bool {
        let results = self.0.packed_eq(other.0);
        results[0] != 0 && results[1] != 0
    }
}

/// 3D homogeneous points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector4F(pub F32x4);

impl Vector4F {
    #[inline]
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Vector4F {
        Vector4F(F32x4::new(x, y, z, w))
    }

    #[inline]
    pub fn splat(value: f32) -> Vector4F {
        Vector4F(F32x4::splat(value))
    }

    #[inline]
    pub fn to_2d(self) -> Vector2F {
        Vector2F(self.0)
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
    pub fn scale(&self, x: f32) -> Vector4F {
        let mut factors = F32x4::splat(x);
        factors[3] = 1.0;
        Vector4F(self.0 * factors)
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
    pub fn perspective_divide(self) -> Vector4F {
        Vector4F(self.0 * F32x4::splat(1.0 / self.w()))
    }

    #[inline]
    pub fn approx_eq(&self, other: &Vector4F, epsilon: f32) -> bool {
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
    pub fn lerp(self, other: Vector4F, t: f32) -> Vector4F {
        Vector4F(self.0 + (other.0 - self.0) * F32x4::splat(t))
    }
}

impl Add<Vector4F> for Vector4F {
    type Output = Vector4F;
    #[inline]
    fn add(self, other: Vector4F) -> Vector4F {
        Vector4F(self.0 + other.0)
    }
}

impl AddAssign for Vector4F {
    #[inline]
    fn add_assign(&mut self, other: Vector4F) {
        self.0 += other.0
    }
}

impl Mul<Vector4F> for Vector4F {
    type Output = Vector4F;
    #[inline]
    fn mul(self, other: Vector4F) -> Vector4F {
        Vector4F(self.0 * other.0)
    }
}

impl Default for Vector4F {
    #[inline]
    fn default() -> Vector4F {
        let mut point = F32x4::default();
        point.set_w(1.0);
        Vector4F(point)
    }
}
