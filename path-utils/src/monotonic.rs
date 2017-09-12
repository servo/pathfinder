// pathfinder/path-utils/src/monotonic.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use arrayvec::ArrayVec;
use euclid::Point2D;
use std::mem;

use PathSegment;
use curve::Curve;

pub struct MonotonicPathSegmentStream<I> {
    inner: I,
    queue: ArrayVec<[PathSegment; 2]>,
    prev_point: Point2D<f32>,
}

impl<I> MonotonicPathSegmentStream<I> where I: Iterator<Item = PathSegment> {
    pub fn new(inner: I) -> MonotonicPathSegmentStream<I> {
        MonotonicPathSegmentStream {
            inner: inner,
            queue: ArrayVec::new(),
            prev_point: Point2D::zero(),
        }
    }
}

impl<I> Iterator for MonotonicPathSegmentStream<I> where I: Iterator<Item = PathSegment> {
    type Item = PathSegment;

    fn next(&mut self) -> Option<PathSegment> {
        if !self.queue.is_empty() {
            return Some(self.queue.remove(0))
        }

        match self.inner.next() {
            None => None,
            Some(PathSegment::ClosePath) => Some(PathSegment::ClosePath),
            Some(PathSegment::MoveTo(point)) => {
                self.prev_point = point;
                Some(PathSegment::MoveTo(point))
            }
            Some(PathSegment::LineTo(point)) => {
                self.prev_point = point;
                Some(PathSegment::LineTo(point))
            }
            Some(PathSegment::CurveTo(control_point, endpoint)) => {
                let curve = Curve::new(&self.prev_point, &control_point, &endpoint);
                match curve.inflection_points() {
                    (None, None) => {
                        self.prev_point = endpoint;
                        Some(PathSegment::CurveTo(control_point, endpoint))
                    }
                    (Some(t), None) | (None, Some(t)) => {
                        let (prev_curve, next_curve) = curve.subdivide(t);
                        self.queue.push(next_curve.to_path_segment());
                        self.prev_point = prev_curve.endpoints[1];
                        Some(prev_curve.to_path_segment())
                    }
                    (Some(mut t0), Some(mut t1)) => {
                        if t1 < t0 {
                            mem::swap(&mut t0, &mut t1)
                        }

                        let (curve_0, curve_12) = curve.subdivide(t0);
                        let (curve_1, curve_2) = curve_12.subdivide(t1);
                        self.queue.push(curve_1.to_path_segment());
                        self.queue.push(curve_2.to_path_segment());

                        self.prev_point = curve_0.endpoints[1];
                        Some(curve_0.to_path_segment())
                    }
                }
            }
        }
    }
}
