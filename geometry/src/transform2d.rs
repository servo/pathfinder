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

use crate::line_segment::LineSegment2F;
use crate::vector::Vector2F;
use crate::rect::RectF;
use crate::transform3d::Transform3DF;
use crate::unit_vector::UnitVector;
use pathfinder_simd::default::F32x4;
use std::ops::Sub;

/// A 2x2 matrix, optimized with SIMD, in column-major order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix2x2F(pub F32x4);

impl Default for Matrix2x2F {
    #[inline]
    fn default() -> Matrix2x2F {
        Self::from_scale(Vector2F::splat(1.0))
    }
}

impl Matrix2x2F {
    #[inline]
    pub fn from_scale(scale: Vector2F) -> Matrix2x2F {
        Matrix2x2F(F32x4::new(scale.x(), 0.0, 0.0, scale.y()))
    }

    #[inline]
    pub fn from_rotation(theta: f32) -> Matrix2x2F {
        Matrix2x2F::from_rotation_vector(UnitVector::from_angle(theta))
    }

    #[inline]
    pub fn from_rotation_vector(vector: UnitVector) -> Matrix2x2F {
        Matrix2x2F((vector.0).0.xyyx() * F32x4::new(1.0, 1.0, -1.0, 1.0))
    }

    #[inline]
    pub fn row_major(m11: f32, m12: f32, m21: f32, m22: f32) -> Matrix2x2F {
        Matrix2x2F(F32x4::new(m11, m21, m12, m22))
    }

    #[inline]
    pub fn post_mul(&self, other: &Matrix2x2F) -> Matrix2x2F {
        Matrix2x2F(self.0.xyxy() * other.0.xxzz() + self.0.zwzw() * other.0.yyww())
    }

    #[inline]
    pub fn pre_mul(&self, other: &Matrix2x2F) -> Matrix2x2F {
        other.post_mul(self)
    }

    #[inline]
    pub fn entrywise_mul(&self, other: &Matrix2x2F) -> Matrix2x2F {
        Matrix2x2F(self.0 * other.0)
    }

    #[inline]
    pub fn adjugate(&self) -> Matrix2x2F {
        Matrix2x2F(self.0.wyzx() * F32x4::new(1.0, -1.0, -1.0, 1.0))
    }

    #[inline]
    pub fn transform_point(&self, point: Vector2F) -> Vector2F {
        let halves = self.0 * point.0.xxyy();
        Vector2F(halves + halves.zwzw())
    }

    #[inline]
    pub fn det(&self) -> f32 {
        self.0[0] * self.0[3] - self.0[2] * self.0[1]
    }

    #[inline]
    pub fn inverse(&self) -> Matrix2x2F {
        Matrix2x2F(F32x4::splat(1.0 / self.det()) * self.adjugate().0)
    }

    #[inline]
    pub fn m11(&self) -> f32 {
        self.0[0]
    }
    #[inline]
    pub fn m21(&self) -> f32 {
        self.0[1]
    }
    #[inline]
    pub fn m12(&self) -> f32 {
        self.0[2]
    }
    #[inline]
    pub fn m22(&self) -> f32 {
        self.0[3]
    }
}

impl Sub<Matrix2x2F> for Matrix2x2F {
    type Output = Matrix2x2F;
    #[inline]
    fn sub(self, other: Matrix2x2F) -> Matrix2x2F {
        Matrix2x2F(self.0 - other.0)
    }
}

/// An affine transform, optimized with SIMD.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2DF {
    // Row-major order.
    pub matrix: Matrix2x2F,
    pub vector: Vector2F,
}

impl Default for Transform2DF {
    #[inline]
    fn default() -> Transform2DF {
        Self::from_scale(Vector2F::splat(1.0))
    }
}

impl Transform2DF {
    #[inline]
    pub fn from_scale(scale: Vector2F) -> Transform2DF {
        Transform2DF {
            matrix: Matrix2x2F::from_scale(scale),
            vector: Vector2F::default(),
        }
    }

    #[inline]
    pub fn from_rotation(theta: f32) -> Transform2DF {
        Transform2DF {
            matrix: Matrix2x2F::from_rotation(theta),
            vector: Vector2F::default(),
        }
    }

    #[inline]
    pub fn from_rotation_vector(vector: UnitVector) -> Transform2DF {
        Transform2DF {
            matrix: Matrix2x2F::from_rotation_vector(vector),
            vector: Vector2F::default(),
        }
    }

    #[inline]
    pub fn from_translation(vector: Vector2F) -> Transform2DF {
        Transform2DF { matrix: Matrix2x2F::default(), vector }
    }

    #[inline]
    pub fn from_scale_rotation_translation(
        scale: Vector2F,
        theta: f32,
        translation: Vector2F,
    ) -> Transform2DF {
        let rotation = Transform2DF::from_rotation(theta);
        let translation = Transform2DF::from_translation(translation);
        Transform2DF::from_scale(scale).post_mul(&rotation).post_mul(&translation)
    }

    #[inline]
    pub fn row_major(m11: f32, m12: f32, m21: f32, m22: f32, m31: f32, m32: f32) -> Transform2DF {
        Transform2DF {
            matrix: Matrix2x2F::row_major(m11, m12, m21, m22),
            vector: Vector2F::new(m31, m32),
        }
    }

    #[inline]
    pub fn transform_point(&self, point: Vector2F) -> Vector2F {
        self.matrix.transform_point(point) + self.vector
    }

    #[inline]
    pub fn transform_line_segment(&self, line_segment: &LineSegment2F) -> LineSegment2F {
        LineSegment2F::new(self.transform_point(line_segment.from()),
                            self.transform_point(line_segment.to()))
    }

    #[inline]
    pub fn transform_rect(&self, rect: &RectF) -> RectF {
        let upper_left = self.transform_point(rect.origin());
        let upper_right = self.transform_point(rect.upper_right());
        let lower_left = self.transform_point(rect.lower_left());
        let lower_right = self.transform_point(rect.lower_right());
        let min_point = upper_left.min(upper_right).min(lower_left).min(lower_right);
        let max_point = upper_left.max(upper_right).max(lower_left).max(lower_right);
        RectF::from_points(min_point, max_point)
    }

    #[inline]
    pub fn post_mul(&self, other: &Transform2DF) -> Transform2DF {
        let matrix = self.matrix.post_mul(&other.matrix);
        let vector = other.transform_point(self.vector);
        Transform2DF { matrix, vector }
    }

    #[inline]
    pub fn pre_mul(&self, other: &Transform2DF) -> Transform2DF {
        other.post_mul(self)
    }

    // TODO(pcwalton): Optimize better with SIMD.
    #[inline]
    pub fn to_3d(&self) -> Transform3DF {
        Transform3DF::row_major(
            self.matrix.0[0],
            self.matrix.0[1],
            0.0,
            self.vector.x(),
            self.matrix.0[2],
            self.matrix.0[3],
            0.0,
            self.vector.y(),
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        )
    }

    #[inline]
    pub fn is_identity(&self) -> bool {
        *self == Transform2DF::default()
    }

    #[inline]
    pub fn m11(&self) -> f32 {
        self.matrix.m11()
    }
    #[inline]
    pub fn m21(&self) -> f32 {
        self.matrix.m21()
    }
    #[inline]
    pub fn m12(&self) -> f32 {
        self.matrix.m12()
    }
    #[inline]
    pub fn m22(&self) -> f32 {
        self.matrix.m22()
    }

    #[inline]
    pub fn post_translate(&self, vector: Vector2F) -> Transform2DF {
        self.post_mul(&Transform2DF::from_translation(vector))
    }

    #[inline]
    pub fn post_rotate(&self, theta: f32) -> Transform2DF {
        self.post_mul(&Transform2DF::from_rotation(theta))
    }

    #[inline]
    pub fn post_scale(&self, scale: Vector2F) -> Transform2DF {
        self.post_mul(&Transform2DF::from_scale(scale))
    }

    /// Returns the translation part of this matrix.
    ///
    /// This decomposition assumes that scale, rotation, and translation are applied in that order.
    #[inline]
    pub fn translation(&self) -> Vector2F {
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
        Vector2F(self.matrix.0.zwxy()).length()
    }
}
