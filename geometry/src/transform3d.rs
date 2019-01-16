// pathfinder/geometry/src/transform3d.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! 3D transforms that can be applied to paths.

use crate::point::{Point2DF32, Point4DF32};
use crate::segment::Segment;
use crate::simd::F32x4;
use euclid::Size2D;

/// An transform, optimized with SIMD.
///
/// In column-major order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform3DF32 {
    c0: F32x4,
    c1: F32x4,
    c2: F32x4,
    c3: F32x4,
}

impl Default for Transform3DF32 {
    #[inline]
    fn default() -> Transform3DF32 {
        Transform3DF32 {
            c0: F32x4::new(1.0, 0.0, 0.0, 0.0),
            c1: F32x4::new(0.0, 1.0, 0.0, 0.0),
            c2: F32x4::new(0.0, 0.0, 1.0, 0.0),
            c3: F32x4::new(0.0, 0.0, 0.0, 1.0),
        }
    }
}

impl Transform3DF32 {
    #[inline]
    pub fn row_major(m00: f32, m01: f32, m02: f32, m03: f32,
                     m10: f32, m11: f32, m12: f32, m13: f32,
                     m20: f32, m21: f32, m22: f32, m23: f32,
                     m30: f32, m31: f32, m32: f32, m33: f32)
                     -> Transform3DF32 {
        Transform3DF32 {
            c0: F32x4::new(m00, m10, m20, m30),
            c1: F32x4::new(m01, m11, m21, m31),
            c2: F32x4::new(m02, m12, m22, m32),
            c3: F32x4::new(m03, m13, m23, m33),
        }
    }

    #[inline]
    pub fn from_scale(x: f32, y: f32, z: f32) -> Transform3DF32 {
        Transform3DF32::row_major(  x, 0.0, 0.0, 0.0,
                                  0.0,   y, 0.0, 0.0,
                                  0.0, 0.0,   z, 0.0,
                                  0.0, 0.0, 0.0, 1.0)
    }

    #[inline]
    pub fn from_translation(x: f32, y: f32, z: f32) -> Transform3DF32 {
        Transform3DF32::row_major(1.0, 0.0, 0.0, x,
                                  0.0, 1.0, 0.0, y,
                                  0.0, 0.0, 1.0, z,
                                  0.0, 0.0, 0.0, 1.0)
    }

    // TODO(pcwalton): Optimize.
    pub fn from_rotation(roll: f32, pitch: f32, yaw: f32) -> Transform3DF32 {
        let (cos_roll, sin_roll) = (roll.cos(), roll.sin());
        let (cos_pitch, sin_pitch) = (pitch.cos(), pitch.sin());
        let (cos_yaw, sin_yaw) = (yaw.cos(), yaw.sin());
        let m00 = cos_yaw * cos_pitch;
        let m01 = cos_yaw * sin_pitch * sin_roll - sin_yaw * cos_roll;
        let m02 = cos_yaw * sin_pitch * sin_roll + sin_yaw * sin_roll;
        let m10 = sin_yaw * cos_pitch;
        let m11 = sin_yaw * sin_pitch * sin_roll + cos_yaw * cos_roll;
        let m12 = sin_yaw * sin_pitch * cos_roll + cos_yaw * sin_roll;
        let m20 = -sin_pitch;
        let m21 = cos_pitch * sin_roll;
        let m22 = cos_pitch * cos_roll;
        Transform3DF32::row_major(m00, m01, m02, 0.0,
                                  m10, m11, m12, 0.0,
                                  m20, m21, m22, 0.0,
                                  0.0, 0.0, 0.0, 1.0)
    }

    /// Just like `glOrtho()`.
    #[inline]
    pub fn from_ortho(left: f32, right: f32, bottom: f32, top: f32, near_val: f32, far_val: f32)
                      -> Transform3DF32 {
        let x_inv = 1.0 / (right - left);
        let y_inv = 1.0 / (top - bottom);
        let z_inv = 1.0 / (far_val - near_val);
        let tx = -(right + left) * x_inv;
        let ty = -(top + bottom) * y_inv;
        let tz = -(far_val + near_val) * z_inv;
        Transform3DF32::row_major(2.0 * x_inv, 0.0,         0.0,          tx,
                                  0.0,         2.0 * y_inv, 0.0,          ty,
                                  0.0,         0.0,         -2.0 * z_inv, tz,
                                  0.0,         0.0,         0.0,          1.0)
    }

    /// Just like `gluPerspective()`.
    #[inline]
    pub fn from_perspective(fov_y: f32, aspect: f32, z_near: f32, z_far: f32) -> Transform3DF32 {
        let f = 1.0 / (fov_y * 0.5).tan();
        let z_denom = 1.0 / (z_near - z_far);
        let m00 = f / aspect;
        let m11 = f;
        let m22 = (z_far + z_near) * z_denom;
        let m23 = 2.0 * z_far * z_near * z_denom;
        let m32 = -1.0;
        Transform3DF32::row_major(m00, 0.0, 0.0, 0.0,
                                  0.0, m11, 0.0, 0.0,
                                  0.0, 0.0, m22, m23,
                                  0.0, 0.0, m32, 0.0)
    }

    #[inline]
    pub fn transpose(&self) -> Transform3DF32 {
        let mut m = *self;
        F32x4::transpose_4x4(&mut m.c0, &mut m.c1, &mut m.c2, &mut m.c3);
        m
    }

    // FIXME(pcwalton): Is this right, due to transposition? I think we may have to reverse the
    // two.
    //
    // https://stackoverflow.com/a/18508113
    #[inline]
    pub fn pre_mul(&self, other: &Transform3DF32) -> Transform3DF32 {
        return Transform3DF32 {
            c0: mul_col(self.c0, other),
            c1: mul_col(self.c1, other),
            c2: mul_col(self.c2, other),
            c3: mul_col(self.c3, other),
        };

        fn mul_col(a_col: F32x4, b: &Transform3DF32) -> F32x4 {
            let (a0, a1) = (F32x4::splat(a_col[0]), F32x4::splat(a_col[1]));
            let (a2, a3) = (F32x4::splat(a_col[2]), F32x4::splat(a_col[3]));
            a0 * b.c0 + a1 * b.c1 + a2 * b.c2 + a3 * b.c3
        }
    }

    #[inline]
    pub fn post_mul(&self, other: &Transform3DF32) -> Transform3DF32 {
        other.pre_mul(self)
    }

    #[inline]
    pub fn transform_point(&self, point: Point4DF32) -> Point4DF32 {
        let term0 = self.c0 * F32x4::splat(point.x());
        let term1 = self.c1 * F32x4::splat(point.y());
        let term2 = self.c2 * F32x4::splat(point.z());
        let term3 = self.c3 * F32x4::splat(point.w());
        Point4DF32(term0 + term1 + term2 + term3)
    }
}

/// Transforms a path with a SIMD 3D transform.
pub struct Transform3DF32PathIter<I>
where
    I: Iterator<Item = Segment>,
{
    iter: I,
    transform: Transform3DF32,
    window_size: Size2D<u32>,
}

impl<I> Iterator for Transform3DF32PathIter<I>
where
    I: Iterator<Item = Segment>,
{
    type Item = Segment;

    #[inline]
    fn next(&mut self) -> Option<Segment> {
        let mut segment = self.iter.next()?;
        if !segment.is_none() {
            segment.baseline.set_from(&self.transform_point(&segment.baseline.from()));
            segment.baseline.set_to(&self.transform_point(&segment.baseline.to()));
            if !segment.is_line() {
                segment.ctrl.set_from(&self.transform_point(&segment.ctrl.from()));
                if !segment.is_quadratic() {
                    segment.ctrl.set_to(&self.transform_point(&segment.ctrl.to()));
                }
            }
        }
        Some(segment)
    }
}

impl<I> Transform3DF32PathIter<I>
where
    I: Iterator<Item = Segment>,
{
    #[inline]
    pub fn new(iter: I, transform: &Transform3DF32, window_size: &Size2D<u32>)
               -> Transform3DF32PathIter<I> {
        Transform3DF32PathIter {
            iter,
            transform: *transform,
            window_size: *window_size,
        }
    }

    #[inline]
    fn transform_point(&self, point: &Point2DF32) -> Point2DF32 {
        let point = self.transform.transform_point(point.to_4d()).perspective_divide().to_2d();
        let window_size = self.window_size.to_f32();
        let size_scale = Point2DF32::new(window_size.width * 0.5, window_size.height * 0.5);
        (point + Point2DF32::splat(1.0)) * size_scale
    }
}

#[cfg(test)]
mod test {
    use crate::point::Point4DF32;
    use crate::transform3d::Transform3DF32;

    #[test]
    fn test_post_mul() {
        let a = Transform3DF32::row_major(3.0, 1.0, 4.0, 5.0,
                                          9.0, 2.0, 6.0, 5.0,
                                          3.0, 5.0, 8.0, 9.0,
                                          7.0, 9.0, 3.0, 2.0);
        let b = Transform3DF32::row_major(3.0, 8.0, 4.0, 6.0,
                                          2.0, 6.0, 4.0, 3.0,
                                          3.0, 8.0, 3.0, 2.0,
                                          7.0, 9.0, 5.0, 0.0);
        let c = Transform3DF32::row_major(58.0,  107.0, 53.0,  29.0,
                                          84.0,  177.0, 87.0,  72.0,
                                          106.0, 199.0, 101.0, 49.0,
                                          62.0,  152.0, 83.0,  75.0);
        assert_eq!(a.post_mul(&b), c);
    }

    #[test]
    fn test_pre_mul() {
        let a = Transform3DF32::row_major(3.0, 1.0, 4.0, 5.0,
                                          9.0, 2.0, 6.0, 5.0,
                                          3.0, 5.0, 8.0, 9.0,
                                          7.0, 9.0, 3.0, 2.0);
        let b = Transform3DF32::row_major(3.0, 8.0, 4.0, 6.0,
                                          2.0, 6.0, 4.0, 3.0,
                                          3.0, 8.0, 3.0, 2.0,
                                          7.0, 9.0, 5.0, 0.0);
        let c = Transform3DF32::row_major(135.0, 93.0, 110.0, 103.0,
                                           93.0, 61.0,  85.0,  82.0,
                                          104.0, 52.0,  90.0,  86.0,
                                          117.0, 50.0, 122.0, 125.0);
        assert_eq!(a.pre_mul(&b), c);
    }

    #[test]
    fn test_transform_point() {
        let a = Transform3DF32::row_major(3.0, 1.0, 4.0, 5.0,
                                          9.0, 2.0, 6.0, 5.0,
                                          3.0, 5.0, 8.0, 9.0,
                                          7.0, 9.0, 3.0, 2.0);
        let p = Point4DF32::new(3.0, 8.0, 4.0, 6.0);
        let q = Point4DF32::new(63.0, 97.0, 135.0, 117.0);
        assert_eq!(a.transform_point(p), q);
    }

    #[test]
    fn test_transpose() {
        let a = Transform3DF32::row_major(3.0, 1.0, 4.0, 5.0,
                                          9.0, 2.0, 6.0, 5.0,
                                          3.0, 5.0, 8.0, 9.0,
                                          7.0, 9.0, 3.0, 2.0);
        let b = Transform3DF32::row_major(3.0, 9.0, 3.0, 7.0,
                                          1.0, 2.0, 5.0, 9.0,
                                          4.0, 6.0, 8.0, 3.0,
                                          5.0, 5.0, 9.0, 2.0);
        assert_eq!(a.transpose(), b);
    }
}
