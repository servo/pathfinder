// pathfinder/geometry/src/basic/transform2d.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! 2D affine transforms.

use crate::basic::point::Point2DF32;
use crate::basic::rect::RectF32;
use crate::basic::transform3d::Transform3DF32;
use crate::segment::Segment;
use pathfinder_simd::default::F32x4;
use std::ops::Sub;

/// A 2x2 matrix, optimized with SIMD, in column-major order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix2x2F32(pub F32x4);

impl Default for Matrix2x2F32 {
    #[inline]
    fn default() -> Matrix2x2F32 {
        Self::from_scale(&Point2DF32::splat(1.0))
    }
}

impl Matrix2x2F32 {
    #[inline]
    pub fn from_scale(scale: &Point2DF32) -> Matrix2x2F32 {
        Matrix2x2F32(F32x4::new(scale.x(), 0.0, 0.0, scale.y()))
    }

    #[inline]
    pub fn from_rotation(theta: f32) -> Matrix2x2F32 {
        let (sin_theta, cos_theta) = (theta.sin(), theta.cos());
        Matrix2x2F32(F32x4::new(cos_theta, sin_theta, -sin_theta, cos_theta))
    }

    #[inline]
    pub fn row_major(m11: f32, m12: f32, m21: f32, m22: f32) -> Matrix2x2F32 {
        Matrix2x2F32(F32x4::new(m11, m21, m12, m22))
    }

    #[inline]
    pub fn post_mul(&self, other: &Matrix2x2F32) -> Matrix2x2F32 {
        Matrix2x2F32(self.0.xyxy() * other.0.xxzz() + self.0.zwzw() * other.0.yyww())
    }

    #[inline]
    pub fn pre_mul(&self, other: &Matrix2x2F32) -> Matrix2x2F32 {
        other.post_mul(self)
    }

    #[inline]
    pub fn entrywise_mul(&self, other: &Matrix2x2F32) -> Matrix2x2F32 {
        Matrix2x2F32(self.0 * other.0)
    }

    #[inline]
    pub fn adjugate(&self) -> Matrix2x2F32 {
        Matrix2x2F32(self.0.wyzx() * F32x4::new(1.0, -1.0, -1.0, 1.0))
    }

    #[inline]
    pub fn transform_point(&self, point: &Point2DF32) -> Point2DF32 {
        let halves = self.0 * point.0.xxyy();
        Point2DF32(halves + halves.zwzw())
    }

    #[inline]
    pub fn det(&self) -> f32 {
        self.0[0] * self.0[3] - self.0[2] * self.0[1]
    }

    #[inline]
    pub fn inverse(&self) -> Matrix2x2F32 {
        Matrix2x2F32(F32x4::splat(1.0 / self.det()) * self.adjugate().0)
    }

    #[inline]
    pub fn m11(&self) -> f32 { self.0[0] }
    #[inline]
    pub fn m21(&self) -> f32 { self.0[1] }
    #[inline]
    pub fn m12(&self) -> f32 { self.0[2] }
    #[inline]
    pub fn m22(&self) -> f32 { self.0[3] }
}

impl Sub<Matrix2x2F32> for Matrix2x2F32 {
    type Output = Matrix2x2F32;
    #[inline]
    fn sub(self, other: Matrix2x2F32) -> Matrix2x2F32 {
        Matrix2x2F32(self.0 - other.0)
    }
}

/// An affine transform, optimized with SIMD.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2DF32 {
    // Row-major order.
    matrix: Matrix2x2F32,
    vector: Point2DF32,
}

impl Default for Transform2DF32 {
    #[inline]
    fn default() -> Transform2DF32 {
        Self::from_scale(&Point2DF32::splat(1.0))
    }
}

impl Transform2DF32 {
    #[inline]
    pub fn from_scale(scale: &Point2DF32) -> Transform2DF32 {
        Transform2DF32 {
            matrix: Matrix2x2F32::from_scale(scale),
            vector: Point2DF32::default(),
        }
    }

    #[inline]
    pub fn from_rotation(theta: f32) -> Transform2DF32 {
        Transform2DF32 {
            matrix: Matrix2x2F32::from_rotation(theta),
            vector: Point2DF32::default(),
        }
    }

    #[inline]
    pub fn from_translation(vector: &Point2DF32) -> Transform2DF32 {
        Transform2DF32 {
            matrix: Matrix2x2F32::default(),
            vector: *vector,
        }
    }

    #[inline]
    pub fn from_scale_rotation_translation(scale: Point2DF32, theta: f32, translation: Point2DF32)
                                           -> Transform2DF32 {
        let rotation = Transform2DF32::from_rotation(theta);
        let translation = Transform2DF32::from_translation(&translation);
        Transform2DF32::from_scale(&scale).post_mul(&rotation).post_mul(&translation)
    }

    #[inline]
    pub fn row_major(m11: f32, m12: f32, m21: f32, m22: f32, m31: f32, m32: f32)
                     -> Transform2DF32 {
        Transform2DF32 {
            matrix: Matrix2x2F32::row_major(m11, m12, m21, m22),
            vector: Point2DF32::new(m31, m32),
        }
    }

    #[inline]
    pub fn transform_point(&self, point: &Point2DF32) -> Point2DF32 {
        self.matrix.transform_point(point) + self.vector
    }

    #[inline]
    pub fn transform_rect(&self, rect: &RectF32) -> RectF32 {
        let upper_left = self.transform_point(&rect.origin());
        let upper_right = self.transform_point(&rect.upper_right());
        let lower_left = self.transform_point(&rect.lower_left());
        let lower_right = self.transform_point(&rect.lower_right());
        let min_point = upper_left.min(upper_right).min(lower_left).min(lower_right);
        let max_point = upper_left.max(upper_right).max(lower_left).max(lower_right);
        RectF32::from_points(min_point, max_point)
    }

    #[inline]
    pub fn post_mul(&self, other: &Transform2DF32) -> Transform2DF32 {
        let matrix = self.matrix.post_mul(&other.matrix);
        let vector = other.transform_point(&self.vector);
        Transform2DF32 { matrix, vector }
    }

    #[inline]
    pub fn pre_mul(&self, other: &Transform2DF32) -> Transform2DF32 {
        other.post_mul(self)
    }

    // TODO(pcwalton): Optimize better with SIMD.
    #[inline]
    pub fn to_3d(&self) -> Transform3DF32 {
        Transform3DF32::row_major(self.matrix.0[0], self.matrix.0[1], 0.0, self.vector.x(),
                                  self.matrix.0[2], self.matrix.0[3], 0.0, self.vector.y(),
                                  0.0,              0.0,              0.0, 0.0,
                                  0.0,              0.0,              0.0, 1.0)
    }

    #[inline]
    pub fn is_identity(&self) -> bool {
        *self == Transform2DF32::default()
    }

    #[inline]
    pub fn m11(&self) -> f32 { self.matrix.m11() }
    #[inline]
    pub fn m21(&self) -> f32 { self.matrix.m21() }
    #[inline]
    pub fn m12(&self) -> f32 { self.matrix.m12() }
    #[inline]
    pub fn m22(&self) -> f32 { self.matrix.m22() }

    #[inline]
    pub fn post_translate(&self, vector: Point2DF32) -> Transform2DF32 {
        self.post_mul(&Transform2DF32::from_translation(&vector))
    }

    #[inline]
    pub fn post_rotate(&self, theta: f32) -> Transform2DF32 {
        self.post_mul(&Transform2DF32::from_rotation(theta))
    }

    #[inline]
    pub fn post_scale(&self, scale: Point2DF32) -> Transform2DF32 {
        self.post_mul(&Transform2DF32::from_scale(&scale))
    }

    /// Returns the translation part of this matrix.
    ///
    /// This decomposition assumes that scale, rotation, and translation are applied in that order.
    #[inline]
    pub fn translation(&self) -> Point2DF32 {
        self.vector
    }

    /// Returns the rotation angle of this matrix.
    ///
    /// This decomposition assumes that scale, rotation, and translation are applied in that order.
    #[inline]
    pub fn rotation(&self) -> f32 {
        f32::atan2(self.m21(), self.m11())
    }

    /// Returns the scale factor of this matrix.
    ///
    /// This decomposition assumes that scale, rotation, and translation are applied in that order.
    #[inline]
    pub fn scale_factor(&self) -> f32 {
        Point2DF32(self.matrix.0.zwxy()).length()
    }
}

/// Transforms a path with a SIMD 2D transform.
pub struct Transform2DF32PathIter<I>
where
    I: Iterator<Item = Segment>,
{
    iter: I,
    transform: Transform2DF32,
}

impl<I> Iterator for Transform2DF32PathIter<I>
where
    I: Iterator<Item = Segment>,
{
    type Item = Segment;

    #[inline]
    fn next(&mut self) -> Option<Segment> {
        // TODO(pcwalton): Can we go faster by transforming an entire line segment with SIMD?
        let mut segment = self.iter.next()?;
        if !segment.is_none() {
            segment
                .baseline
                .set_from(&self.transform.transform_point(&segment.baseline.from()));
            segment
                .baseline
                .set_to(&self.transform.transform_point(&segment.baseline.to()));
            if !segment.is_line() {
                segment
                    .ctrl
                    .set_from(&self.transform.transform_point(&segment.ctrl.from()));
                if !segment.is_quadratic() {
                    segment
                        .ctrl
                        .set_to(&self.transform.transform_point(&segment.ctrl.to()));
                }
            }
        }
        Some(segment)
    }
}

impl<I> Transform2DF32PathIter<I>
where
    I: Iterator<Item = Segment>,
{
    #[inline]
    pub fn new(iter: I, transform: &Transform2DF32) -> Transform2DF32PathIter<I> {
        Transform2DF32PathIter {
            iter,
            transform: *transform,
        }
    }
}
