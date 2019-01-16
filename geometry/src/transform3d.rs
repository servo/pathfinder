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

use crate::point::Point4DF32;
use crate::simd::F32x4;

/// An transform, optimized with SIMD.
///
/// In column-major order.
#[derive(Clone, Copy)]
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
    pub fn scale(x: f32, y: f32, z: f32) -> Transform3DF32 {
        Transform3DF32::row_major(  x, 0.0, 0.0, 0.0,
                                  0.0,   y, 0.0, 0.0,
                                  0.0, 0.0,   z, 0.0,
                                  0.0, 0.0, 0.0, 1.0)
    }

    #[inline]
    pub fn translate(x: f32, y: f32, z: f32) -> Transform3DF32 {
        Transform3DF32::row_major(1.0, 0.0, 0.0, x,
                                  0.0, 1.0, 0.0, y,
                                  0.0, 0.0, 1.0, z,
                                  0.0, 0.0, 0.0, 1.0)
    }

    // TODO(pcwalton): Optimize.
    pub fn rotate(roll: f32, pitch: f32, yaw: f32) -> Transform3DF32 {
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

    /// Just like `gluPerspective()`.
    #[inline]
    pub fn perspective(fov_y: f32, aspect: f32, z_near: f32, z_far: f32) -> Transform3DF32 {
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
    pub fn post_mul(&self, other: &Transform3DF32) -> Transform3DF32 {
        return Transform3DF32 {
            c0: mul_row(self.c0, other),
            c1: mul_row(self.c1, other),
            c2: mul_row(self.c2, other),
            c3: mul_row(self.c3, other),
        };

        fn mul_row(a_row: F32x4, b: &Transform3DF32) -> F32x4 {
            let (a0, a1) = (F32x4::splat(a_row[0]), F32x4::splat(a_row[1]));
            let (a2, a3) = (F32x4::splat(a_row[2]), F32x4::splat(a_row[3]));
            a0 * b.c0 + a1 * b.c1 + a2 * b.c2 + a3 * b.c3
        }
    }

    #[inline]
    pub fn pre_mul(&self, other: &Transform3DF32) -> Transform3DF32 {
        other.post_mul(self)
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
