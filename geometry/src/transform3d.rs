// pathfinder/geometry/src/basic/transform3d.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! 3D transforms that can be applied to paths.

use crate::vector::{Vector2F, Vector2I, Vector4F};
use crate::rect::RectF;
use crate::transform2d::Matrix2x2F;
use pathfinder_simd::default::F32x4;
use std::ops::{Add, Neg};

/// An transform, optimized with SIMD.
///
/// In column-major order.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Transform3DF {
    pub c0: F32x4,
    pub c1: F32x4,
    pub c2: F32x4,
    pub c3: F32x4,
}

impl Default for Transform3DF {
    #[inline]
    fn default() -> Transform3DF {
        Transform3DF {
            c0: F32x4::new(1.0, 0.0, 0.0, 0.0),
            c1: F32x4::new(0.0, 1.0, 0.0, 0.0),
            c2: F32x4::new(0.0, 0.0, 1.0, 0.0),
            c3: F32x4::new(0.0, 0.0, 0.0, 1.0),
        }
    }
}

impl Transform3DF {
    #[inline]
    pub fn row_major(
        m00: f32,
        m01: f32,
        m02: f32,
        m03: f32,
        m10: f32,
        m11: f32,
        m12: f32,
        m13: f32,
        m20: f32,
        m21: f32,
        m22: f32,
        m23: f32,
        m30: f32,
        m31: f32,
        m32: f32,
        m33: f32,
    ) -> Transform3DF {
        Transform3DF {
            c0: F32x4::new(m00, m10, m20, m30),
            c1: F32x4::new(m01, m11, m21, m31),
            c2: F32x4::new(m02, m12, m22, m32),
            c3: F32x4::new(m03, m13, m23, m33),
        }
    }

    #[inline]
    pub fn from_scale(x: f32, y: f32, z: f32) -> Transform3DF {
        Transform3DF::row_major(
            x, 0.0, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 0.0, z, 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    #[inline]
    pub fn from_uniform_scale(factor: f32) -> Transform3DF {
        Transform3DF::from_scale(factor, factor, factor)
    }

    #[inline]
    pub fn from_translation(x: f32, y: f32, z: f32) -> Transform3DF {
        Transform3DF::row_major(
            1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z, 0.0, 0.0, 0.0, 1.0,
        )
    }

    // TODO(pcwalton): Optimize.
    pub fn from_rotation(yaw: f32, pitch: f32, roll: f32) -> Transform3DF {
        let (cos_b, sin_b) = (yaw.cos(), yaw.sin());
        let (cos_c, sin_c) = (pitch.cos(), pitch.sin());
        let (cos_a, sin_a) = (roll.cos(), roll.sin());
        let m00 = cos_a * cos_b;
        let m01 = cos_a * sin_b * sin_c - sin_a * cos_c;
        let m02 = cos_a * sin_b * cos_c + sin_a * sin_c;
        let m10 = sin_a * cos_b;
        let m11 = sin_a * sin_b * sin_c + cos_a * cos_c;
        let m12 = sin_a * sin_b * cos_c - cos_a * sin_c;
        let m20 = -sin_b;
        let m21 = cos_b * sin_c;
        let m22 = cos_b * cos_c;
        Transform3DF::row_major(
            m00, m01, m02, 0.0, m10, m11, m12, 0.0, m20, m21, m22, 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    /// Creates a rotation matrix from the given quaternion.
    ///
    /// The quaternion is expected to be packed into a SIMD type (x, y, z, w) corresponding to
    /// x + yi + zj + wk.
    pub fn from_rotation_quaternion(q: F32x4) -> Transform3DF {
        // TODO(pcwalton): Optimize better with more shuffles.
        let (mut sq, mut w, mut xy_xz_yz) = (q * q, q.wwww() * q, q.xxyy() * q.yzzy());
        sq += sq;
        w += w;
        xy_xz_yz += xy_xz_yz;
        let diag = F32x4::splat(1.0) - (sq.yxxy() + sq.zzyy());
        let (wx2, wy2, wz2) = (w.x(), w.y(), w.z());
        let (xy2, xz2, yz2) = (xy_xz_yz.x(), xy_xz_yz.y(), xy_xz_yz.z());
        Transform3DF::row_major(
            diag.x(),
            xy2 - wz2,
            xz2 + wy2,
            0.0,
            xy2 + wz2,
            diag.y(),
            yz2 - wx2,
            0.0,
            xz2 - wy2,
            yz2 + wx2,
            diag.z(),
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        )
    }

    /// Just like `glOrtho()`.
    #[inline]
    pub fn from_ortho(
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near_val: f32,
        far_val: f32,
    ) -> Transform3DF {
        let x_inv = 1.0 / (right - left);
        let y_inv = 1.0 / (top - bottom);
        let z_inv = 1.0 / (far_val - near_val);
        let tx = -(right + left) * x_inv;
        let ty = -(top + bottom) * y_inv;
        let tz = -(far_val + near_val) * z_inv;
        Transform3DF::row_major(
            2.0 * x_inv,
            0.0,
            0.0,
            tx,
            0.0,
            2.0 * y_inv,
            0.0,
            ty,
            0.0,
            0.0,
            -2.0 * z_inv,
            tz,
            0.0,
            0.0,
            0.0,
            1.0,
        )
    }

    /// Linearly interpolate between transforms
    pub fn lerp(&self, weight: f32, other: &Transform3DF) -> Transform3DF {
        let c0 = self.c0 * F32x4::splat(weight) + other.c0 * F32x4::splat(1.0 - weight);
        let c1 = self.c1 * F32x4::splat(weight) + other.c1 * F32x4::splat(1.0 - weight);
        let c2 = self.c2 * F32x4::splat(weight) + other.c2 * F32x4::splat(1.0 - weight);
        let c3 = self.c3 * F32x4::splat(weight) + other.c3 * F32x4::splat(1.0 - weight);
        Transform3DF { c0, c1, c2, c3 }
    }

    /// Just like `gluPerspective()`.
    #[inline]
    pub fn from_perspective(fov_y: f32, aspect: f32, z_near: f32, z_far: f32) -> Transform3DF {
        let f = 1.0 / (fov_y * 0.5).tan();
        let z_denom = 1.0 / (z_near - z_far);
        let m00 = f / aspect;
        let m11 = f;
        let m22 = (z_far + z_near) * z_denom;
        let m23 = 2.0 * z_far * z_near * z_denom;
        let m32 = -1.0;
        Transform3DF::row_major(
            m00, 0.0, 0.0, 0.0, 0.0, m11, 0.0, 0.0, 0.0, 0.0, m22, m23, 0.0, 0.0, m32, 0.0,
        )
    }

    //     +-     -+
    //     |  A B  |
    //     |  C D  |
    //     +-     -+
    #[inline]
    pub fn from_submatrices(
        a: Matrix2x2F,
        b: Matrix2x2F,
        c: Matrix2x2F,
        d: Matrix2x2F,
    ) -> Transform3DF {
        Transform3DF {
            c0: a.0.concat_xy_xy(c.0),
            c1: a.0.concat_zw_zw(c.0),
            c2: b.0.concat_xy_xy(d.0),
            c3: b.0.concat_zw_zw(d.0),
        }
    }

    // FIXME(pcwalton): Is this right, due to transposition? I think we may have to reverse the
    // two.
    //
    // https://stackoverflow.com/a/18508113
    #[inline]
    pub fn pre_mul(&self, other: &Transform3DF) -> Transform3DF {
        return Transform3DF {
            c0: mul_col(self.c0, other),
            c1: mul_col(self.c1, other),
            c2: mul_col(self.c2, other),
            c3: mul_col(self.c3, other),
        };

        fn mul_col(a_col: F32x4, b: &Transform3DF) -> F32x4 {
            let (a0, a1) = (F32x4::splat(a_col[0]), F32x4::splat(a_col[1]));
            let (a2, a3) = (F32x4::splat(a_col[2]), F32x4::splat(a_col[3]));
            a0 * b.c0 + a1 * b.c1 + a2 * b.c2 + a3 * b.c3
        }
    }

    #[inline]
    pub fn post_mul(&self, other: &Transform3DF) -> Transform3DF {
        other.pre_mul(self)
    }

    #[inline]
    pub fn transform_point(&self, point: Vector4F) -> Vector4F {
        let term0 = self.c0 * F32x4::splat(point.x());
        let term1 = self.c1 * F32x4::splat(point.y());
        let term2 = self.c2 * F32x4::splat(point.z());
        let term3 = self.c3 * F32x4::splat(point.w());
        Vector4F(term0 + term1 + term2 + term3)
    }

    #[inline]
    pub fn upper_left(&self) -> Matrix2x2F {
        Matrix2x2F(self.c0.concat_xy_xy(self.c1))
    }

    #[inline]
    pub fn upper_right(&self) -> Matrix2x2F {
        Matrix2x2F(self.c2.concat_xy_xy(self.c3))
    }

    #[inline]
    pub fn lower_left(&self) -> Matrix2x2F {
        Matrix2x2F(self.c0.concat_zw_zw(self.c1))
    }

    #[inline]
    pub fn lower_right(&self) -> Matrix2x2F {
        Matrix2x2F(self.c2.concat_zw_zw(self.c3))
    }

    // https://en.wikipedia.org/wiki/Invertible_matrix#Blockwise_inversion
    //
    // If A is the upper left submatrix of this matrix, this method assumes that A and the Schur
    // complement of A are invertible.
    pub fn inverse(&self) -> Transform3DF {
        // Extract submatrices.
        let (a, b) = (self.upper_left(), self.upper_right());
        let (c, d) = (self.lower_left(), self.lower_right());

        // Compute temporary matrices.
        let a_inv = a.inverse();
        let x = c.post_mul(&a_inv);
        let y = (d - x.post_mul(&b)).inverse();
        let z = a_inv.post_mul(&b);

        // Compute new submatrices.
        let (a_new, b_new) = (a_inv + z.post_mul(&y).post_mul(&x), (-z).post_mul(&y));
        let (c_new, d_new) = ((-y).post_mul(&x), y);

        // Construct inverse.
        Transform3DF::from_submatrices(a_new, b_new, c_new, d_new)
    }

    pub fn approx_eq(&self, other: &Transform3DF, epsilon: f32) -> bool {
        self.c0.approx_eq(other.c0, epsilon)
            && self.c1.approx_eq(other.c1, epsilon)
            && self.c2.approx_eq(other.c2, epsilon)
            && self.c3.approx_eq(other.c3, epsilon)
    }

    #[inline]
    pub fn as_ptr(&self) -> *const f32 {
        (&self.c0) as *const F32x4 as *const f32
    }

    #[inline]
    pub fn to_columns(&self) -> [F32x4; 4] {
        [self.c0, self.c1, self.c2, self.c3]
    }
}

impl Add<Matrix2x2F> for Matrix2x2F {
    type Output = Matrix2x2F;
    #[inline]
    fn add(self, other: Matrix2x2F) -> Matrix2x2F {
        Matrix2x2F(self.0 + other.0)
    }
}

impl Neg for Matrix2x2F {
    type Output = Matrix2x2F;
    #[inline]
    fn neg(self) -> Matrix2x2F {
        Matrix2x2F(-self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Perspective {
    pub transform: Transform3DF,
    pub window_size: Vector2I,
}

impl Perspective {
    #[inline]
    pub fn new(transform: &Transform3DF, window_size: Vector2I) -> Perspective {
        Perspective {
            transform: *transform,
            window_size,
        }
    }

    #[inline]
    pub fn transform_point_2d(&self, point: &Vector2F) -> Vector2F {
        let point = self
            .transform
            .transform_point(point.to_3d())
            .perspective_divide()
            .to_2d()
            * Vector2F::new(1.0, -1.0);
        (point + Vector2F::splat(1.0)) * self.window_size.to_f32().scale(0.5)
    }

    // TODO(pcwalton): SIMD?
    #[inline]
    pub fn transform_rect(&self, rect: RectF) -> RectF {
        let upper_left = self.transform_point_2d(&rect.origin());
        let upper_right = self.transform_point_2d(&rect.upper_right());
        let lower_left = self.transform_point_2d(&rect.lower_left());
        let lower_right = self.transform_point_2d(&rect.lower_right());
        let min_point = upper_left.min(upper_right).min(lower_left).min(lower_right);
        let max_point = upper_left.max(upper_right).max(lower_left).max(lower_right);
        RectF::from_points(min_point, max_point)
    }

    #[inline]
    pub fn post_mul(&self, other: &Transform3DF) -> Perspective {
        Perspective {
            transform: self.transform.post_mul(other),
            window_size: self.window_size,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::vector::Vector4F;
    use crate::transform3d::Transform3DF;

    #[test]
    fn test_post_mul() {
        let a = Transform3DF::row_major(
            3.0, 1.0, 4.0, 5.0, 9.0, 2.0, 6.0, 5.0, 3.0, 5.0, 8.0, 9.0, 7.0, 9.0, 3.0, 2.0,
        );
        let b = Transform3DF::row_major(
            3.0, 8.0, 4.0, 6.0, 2.0, 6.0, 4.0, 3.0, 3.0, 8.0, 3.0, 2.0, 7.0, 9.0, 5.0, 0.0,
        );
        let c = Transform3DF::row_major(
            58.0, 107.0, 53.0, 29.0, 84.0, 177.0, 87.0, 72.0, 106.0, 199.0, 101.0, 49.0, 62.0,
            152.0, 83.0, 75.0,
        );
        assert_eq!(a.post_mul(&b), c);
    }

    #[test]
    fn test_pre_mul() {
        let a = Transform3DF::row_major(
            3.0, 1.0, 4.0, 5.0, 9.0, 2.0, 6.0, 5.0, 3.0, 5.0, 8.0, 9.0, 7.0, 9.0, 3.0, 2.0,
        );
        let b = Transform3DF::row_major(
            3.0, 8.0, 4.0, 6.0, 2.0, 6.0, 4.0, 3.0, 3.0, 8.0, 3.0, 2.0, 7.0, 9.0, 5.0, 0.0,
        );
        let c = Transform3DF::row_major(
            135.0, 93.0, 110.0, 103.0, 93.0, 61.0, 85.0, 82.0, 104.0, 52.0, 90.0, 86.0, 117.0,
            50.0, 122.0, 125.0,
        );
        assert_eq!(a.pre_mul(&b), c);
    }

    #[test]
    fn test_transform_point() {
        let a = Transform3DF::row_major(
            3.0, 1.0, 4.0, 5.0, 9.0, 2.0, 6.0, 5.0, 3.0, 5.0, 8.0, 9.0, 7.0, 9.0, 3.0, 2.0,
        );
        let p = Vector4F::new(3.0, 8.0, 4.0, 6.0);
        let q = Vector4F::new(63.0, 97.0, 135.0, 117.0);
        assert_eq!(a.transform_point(p), q);
    }

    #[test]
    fn test_inverse() {
        // Random matrix.
        let m = Transform3DF::row_major(
            0.86277982, 0.15986552, 0.90739898, 0.60066808, 0.17386167, 0.016353, 0.8535783,
            0.12969608, 0.0946466, 0.43248631, 0.63480505, 0.08154603, 0.50305436, 0.48359687,
            0.51057162, 0.24812012,
        );
        let p0 = Vector4F::new(0.95536648, 0.80633691, 0.16357357, 0.5477598);
        let p1 = m.transform_point(p0);
        let m_inv = m.inverse();
        let m_inv_exp = Transform3DF::row_major(
            -2.47290136,
            3.48865688,
            -6.12298336,
            6.17536696,
            0.00124033357,
            -1.72561993,
            2.16876606,
            0.186227748,
            -0.375021729,
            1.53883017,
            -0.0558194403,
            0.121857058,
            5.78300323,
            -6.87635769,
            8.30196620,
            -9.10374060,
        );
        assert!(m_inv.approx_eq(&m_inv_exp, 0.0001));
        let p2 = m_inv.transform_point(p1);
        assert!(p0.approx_eq(&p2, 0.0001));
    }
}
