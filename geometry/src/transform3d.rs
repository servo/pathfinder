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

use crate::point::{Point2DF32, Point3DF32};
use crate::segment::Segment;
use crate::transform::Matrix2x2F32;
use euclid::{Point2D, Rect, Size2D};
use pathfinder_simd::default::F32x4;
use std::ops::{Add, Neg};

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
    pub fn from_rotation(yaw: f32, pitch: f32, roll: f32) -> Transform3DF32 {
        let (cos_b, sin_b) = (yaw.cos(),   yaw.sin());
        let (cos_c, sin_c) = (pitch.cos(), pitch.sin());
        let (cos_a, sin_a) = (roll.cos(),  roll.sin());
        let m00 = cos_a * cos_b;
        let m01 = cos_a * sin_b * sin_c - sin_a * cos_c;
        let m02 = cos_a * sin_b * cos_c + sin_a * sin_c;
        let m10 = sin_a * cos_b;
        let m11 = sin_a * sin_b * sin_c + cos_a * cos_c;
        let m12 = sin_a * sin_b * cos_c - cos_a * sin_c;
        let m20 = -sin_b;
        let m21 = cos_b * sin_c;
        let m22 = cos_b * cos_c;
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

    //     +-     -+
    //     |  A B  |
    //     |  C D  |
    //     +-     -+
    #[inline]
    pub fn from_submatrices(a: Matrix2x2F32, b: Matrix2x2F32, c: Matrix2x2F32, d: Matrix2x2F32)
                            -> Transform3DF32 {
        Transform3DF32 {
            c0: a.0.concat_xy_xy(c.0),
            c1: a.0.concat_zw_zw(c.0),
            c2: b.0.concat_xy_xy(d.0),
            c3: b.0.concat_zw_zw(d.0),
        }
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
    pub fn transform_point(&self, point: Point3DF32) -> Point3DF32 {
        let term0 = self.c0 * F32x4::splat(point.x());
        let term1 = self.c1 * F32x4::splat(point.y());
        let term2 = self.c2 * F32x4::splat(point.z());
        let term3 = self.c3 * F32x4::splat(point.w());
        Point3DF32(term0 + term1 + term2 + term3)
    }

    #[inline]
    pub fn upper_left(&self) -> Matrix2x2F32 {
        Matrix2x2F32(self.c0.concat_xy_xy(self.c1))
    }

    #[inline]
    pub fn upper_right(&self) -> Matrix2x2F32 {
        Matrix2x2F32(self.c2.concat_xy_xy(self.c3))
    }

    #[inline]
    pub fn lower_left(&self) -> Matrix2x2F32 {
        Matrix2x2F32(self.c0.concat_zw_zw(self.c1))
    }

    #[inline]
    pub fn lower_right(&self) -> Matrix2x2F32 {
        Matrix2x2F32(self.c2.concat_zw_zw(self.c3))
    }

    // https://en.wikipedia.org/wiki/Invertible_matrix#Blockwise_inversion
    //
    // If A is the upper left submatrix of this matrix, this method assumes that A and the Schur
    // complement of A are invertible.
    pub fn inverse(&self) -> Transform3DF32 {
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
        let (c_new, d_new) = ((-y).post_mul(&x),                   y);

        // Construct inverse.
        Transform3DF32::from_submatrices(a_new, b_new, c_new, d_new)
    }

    pub fn approx_eq(&self, other: &Transform3DF32, epsilon: f32) -> bool {
        self.c0.approx_eq(other.c0, epsilon) &&
            self.c1.approx_eq(other.c1, epsilon) &&
            self.c2.approx_eq(other.c2, epsilon) &&
            self.c3.approx_eq(other.c3, epsilon)
    }
}

impl Add<Matrix2x2F32> for Matrix2x2F32 {
    type Output = Matrix2x2F32;
    #[inline]
    fn add(self, other: Matrix2x2F32) -> Matrix2x2F32 {
        Matrix2x2F32(self.0 + other.0)
    }
}

impl Neg for Matrix2x2F32 {
    type Output = Matrix2x2F32;
    #[inline]
    fn neg(self) -> Matrix2x2F32 {
        Matrix2x2F32(-self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Perspective {
    pub transform: Transform3DF32,
    pub window_size: Size2D<u32>,
}

impl Perspective {
    #[inline]
    pub fn new(transform: &Transform3DF32, window_size: &Size2D<u32>) -> Perspective {
        Perspective { transform: *transform, window_size: *window_size }
    }

    #[inline]
    pub fn transform_point_2d(&self, point: &Point2DF32) -> Point2DF32 {
        let point = self.transform.transform_point(point.to_4d()).perspective_divide().to_2d();
        let window_size = self.window_size.to_f32();
        let size_scale = Point2DF32::new(window_size.width * 0.5, window_size.height * 0.5);
        (point + Point2DF32::splat(1.0)) * size_scale
    }

    // TODO(pcwalton): SIMD?
    #[inline]
    pub fn transform_rect(&self, rect: &Rect<f32>) -> Rect<f32> {
        let upper_left = self.transform_point_2d(&Point2DF32::from_euclid(rect.origin));
        let upper_right = self.transform_point_2d(&Point2DF32::from_euclid(rect.top_right()));
        let lower_left = self.transform_point_2d(&Point2DF32::from_euclid(rect.bottom_left()));
        let lower_right = self.transform_point_2d(&Point2DF32::from_euclid(rect.bottom_right()));
        let min_x = upper_left.x().min(upper_right.x()).min(lower_left.x()).min(lower_right.x());
        let min_y = upper_left.y().min(upper_right.y()).min(lower_left.y()).min(lower_right.y());
        let max_x = upper_left.x().max(upper_right.x()).max(lower_left.x()).max(lower_right.x());
        let max_y = upper_left.y().max(upper_right.y()).max(lower_left.y()).max(lower_right.y());
        let (width, height) = (max_x - min_x, max_y - min_y);
        Rect::new(Point2D::new(min_x, min_y), Size2D::new(width, height))
    }
}

/// Transforms a path with a perspective projection.
pub struct PerspectivePathIter<I>
where
    I: Iterator<Item = Segment>,
{
    iter: I,
    perspective: Perspective,
}

impl<I> Iterator for PerspectivePathIter<I>
where
    I: Iterator<Item = Segment>,
{
    type Item = Segment;

    #[inline]
    fn next(&mut self) -> Option<Segment> {
        let mut segment = self.iter.next()?;
        if !segment.is_none() {
            segment.baseline.set_from(&self.perspective
                                           .transform_point_2d(&segment.baseline.from()));
            segment.baseline.set_to(&self.perspective.transform_point_2d(&segment.baseline.to()));
            if !segment.is_line() {
                segment.ctrl.set_from(&self.perspective.transform_point_2d(&segment.ctrl.from()));
                if !segment.is_quadratic() {
                    segment.ctrl.set_to(&self.perspective.transform_point_2d(&segment.ctrl.to()));
                }
            }
        }
        Some(segment)
    }
}

impl<I> PerspectivePathIter<I>
where
    I: Iterator<Item = Segment>,
{
    #[inline]
    pub fn new(iter: I, perspective: &Perspective) -> PerspectivePathIter<I> {
        PerspectivePathIter { iter, perspective: *perspective }
    }
}

#[cfg(test)]
mod test {
    use crate::point::Point3DF32;
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
        let p = Point3DF32::new(3.0, 8.0, 4.0, 6.0);
        let q = Point3DF32::new(63.0, 97.0, 135.0, 117.0);
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

    #[test]
    fn test_inverse() {
        // Random matrix.
        let m = Transform3DF32::row_major(0.86277982, 0.15986552, 0.90739898, 0.60066808,
                                          0.17386167, 0.016353  , 0.8535783 , 0.12969608,
                                          0.0946466 , 0.43248631, 0.63480505, 0.08154603,
                                          0.50305436, 0.48359687, 0.51057162, 0.24812012);
        let p0 = Point3DF32::new(0.95536648, 0.80633691, 0.16357357, 0.5477598);
        let p1 = m.transform_point(p0);
        let m_inv = m.inverse();
        let m_inv_exp =
            Transform3DF32::row_major(-2.47290136   ,  3.48865688, -6.12298336  ,  6.17536696 ,
                                       0.00124033357, -1.72561993,  2.16876606  ,  0.186227748,
                                      -0.375021729  ,  1.53883017, -0.0558194403,  0.121857058,
                                       5.78300323   , -6.87635769,  8.30196620  , -9.10374060);
        assert!(m_inv.approx_eq(&m_inv_exp, 0.0001));
        let p2 = m_inv.transform_point(p1);
        assert!(p0.approx_eq(&p2, 0.0001));
    }
}
