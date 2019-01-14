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
use crate::simd::F32x4;
use euclid::Transform2D;
use lyon_path::PathEvent;

/// An affine transform, optimized with SIMD.
#[derive(Clone, Copy)]
pub struct Transform2DF32 {
    // Row-major order.
    matrix: F32x4,
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
            matrix: F32x4::new(scale.x(), 0.0, 0.0, scale.y()),
            vector: Point2DF32::default(),
        }
    }

    #[inline]
    pub fn row_major(m11: f32, m12: f32, m21: f32, m22: f32, m31: f32, m32: f32)
                     -> Transform2DF32 {
        Transform2DF32 {
            matrix: F32x4::new(m11, m12, m21, m22),
            vector: Point2DF32::new(m31, m32),
        }
    }

    #[inline]
    pub fn transform_point(&self, point: &Point2DF32) -> Point2DF32 {
        let x11x12y21y22 = point.0.xxyy() * self.matrix;
        Point2DF32(x11x12y21y22 + x11x12y21y22.zwzw() + self.vector.0)
    }

    #[inline]
    pub fn post_mul(&self, other: &Transform2DF32) -> Transform2DF32 {
        let lhs = self.matrix.xzxz() * other.matrix.xxyy();
        let rhs = self.matrix.ywyw() * other.matrix.zzww();
        let matrix = lhs + rhs;
        let vector = other.transform_point(&self.vector) + other.vector;
        Transform2DF32 { matrix, vector }
    }

    #[inline]
    pub fn pre_mul(&self, other: &Transform2DF32) -> Transform2DF32 {
        other.post_mul(self)
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
