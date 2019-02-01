// pathfinder/geometry/src/transform.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Applies a transform to paths.

use crate::point::Point2DF32;
use crate::segment::Segment;
use crate::transform3d::Transform3DF32;
use euclid::{Point2D, Rect, Size2D, Transform2D};
use lyon_path::PathEvent;
use pathfinder_simd::default::F32x4;
use std::ops::Sub;

/// A 2x2 matrix, optimized with SIMD, in column-major order.
#[derive(Clone, Copy, Debug)]
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
}

impl Sub<Matrix2x2F32> for Matrix2x2F32 {
    type Output = Matrix2x2F32;
    #[inline]
    fn sub(self, other: Matrix2x2F32) -> Matrix2x2F32 {
        Matrix2x2F32(self.0 - other.0)
    }
}

/// An affine transform, optimized with SIMD.
#[derive(Clone, Copy, Debug)]
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

    // TODO(pcwalton): SIMD.
    #[inline]
    pub fn transform_rect(&self, rect: &Rect<f32>) -> Rect<f32> {
        let upper_left = self.transform_point(&Point2DF32::from_euclid(rect.origin));
        let upper_right = self.transform_point(&Point2DF32::from_euclid(rect.top_right()));
        let lower_left = self.transform_point(&Point2DF32::from_euclid(rect.bottom_left()));
        let lower_right = self.transform_point(&Point2DF32::from_euclid(rect.bottom_right()));
        let min_x = upper_left.x().min(upper_right.x()).min(lower_left.x()).min(lower_right.x());
        let min_y = upper_left.y().min(upper_right.y()).min(lower_left.y()).min(lower_right.y());
        let max_x = upper_left.x().max(upper_right.x()).max(lower_left.x()).max(lower_right.x());
        let max_y = upper_left.y().max(upper_right.y()).max(lower_left.y()).max(lower_right.y());
        let (width, height) = (max_x - min_x, max_y - min_y);
        Rect::new(Point2D::new(min_x, min_y), Size2D::new(width, height))
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

/// Transforms a path with a Euclid 2D transform.
pub struct Transform2DPathIter<I> where I: Iterator<Item = PathEvent> {
    inner: I,
    transform: Transform2D<f32>,
}

impl<I> Transform2DPathIter<I> where I: Iterator<Item = PathEvent> {
    #[inline]
    pub fn new(inner: I, transform: &Transform2D<f32>) -> Transform2DPathIter<I> {
        Transform2DPathIter {
            inner: inner,
            transform: *transform,
        }
    }
}

impl<I> Iterator for Transform2DPathIter<I> where I: Iterator<Item = PathEvent> {
    type Item = PathEvent;

    fn next(&mut self) -> Option<PathEvent> {
        match self.inner.next() {
            Some(PathEvent::MoveTo(to)) => {
                Some(PathEvent::MoveTo(self.transform.transform_point(&to)))
            }
            Some(PathEvent::LineTo(to)) => {
                Some(PathEvent::LineTo(self.transform.transform_point(&to)))
            }
            Some(PathEvent::QuadraticTo(ctrl, to)) => {
                Some(PathEvent::QuadraticTo(self.transform.transform_point(&ctrl),
                                            self.transform.transform_point(&to)))
            }
            Some(PathEvent::CubicTo(ctrl1, ctrl2, to)) => {
                Some(PathEvent::CubicTo(self.transform.transform_point(&ctrl1),
                                        self.transform.transform_point(&ctrl2),
                                        self.transform.transform_point(&to)))
            }
            Some(PathEvent::Arc(center, radius, start, end)) => {
                Some(PathEvent::Arc(self.transform.transform_point(&center),
                                    self.transform.transform_vector(&radius),
                                    start,
                                    end))
            }
            event => event,
        }
    }
}
