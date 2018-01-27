// pathfinder/path-utils/src/transform.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Applies a transform to paths.

use euclid::Transform2D;
use lyon_path::PathEvent;

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
