// pathfinder/partitioner/src/builder.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use arrayvec::ArrayVec;
use euclid::{Angle, Point2D, Vector2D};
use lyon_geom::{CubicBezierSegment, QuadraticBezierSegment};
use lyon_path::builder::{FlatPathBuilder, PathBuilder};
use pathfinder_path_utils::cubic_to_quadratic::CubicToQuadraticSegmentIter;
use std::ops::Range;

const TANGENT_PARAMETER_TOLERANCE: f32 = 0.001;

const CUBIC_APPROX_TOLERANCE: f32 = 0.001;

// TODO(pcwalton): A better debug.
#[derive(Debug)]
pub struct Builder {
    pub endpoints: Vec<Endpoint>,
    pub subpath_ranges: Vec<Range<u32>>,
}

impl Builder {
    #[inline]
    pub fn new() -> Builder {
        Builder {
            endpoints: vec![],
            subpath_ranges: vec![],
        }
    }

    #[inline]
    fn current_subpath_index(&self) -> Option<u32> {
        if self.subpath_ranges.is_empty() {
            None
        } else {
            Some(self.subpath_ranges.len() as u32 - 1)
        }
    }

    fn add_endpoint(&mut self, ctrl: Option<Point2D<f32>>, to: Point2D<f32>) {
        let current_subpath_index = match self.current_subpath_index() {
            None => return,
            Some(current_subpath_index) => current_subpath_index,
        };

        self.endpoints.push(Endpoint {
            to: to,
            ctrl: ctrl,
            subpath_index: current_subpath_index,
        });
    }

    #[inline]
    pub fn end_subpath(&mut self) {
        let last_endpoint_index = self.endpoints.len() as u32;
        if let Some(current_subpath) = self.subpath_ranges.last_mut() {
            current_subpath.end = last_endpoint_index
        }
    }

    #[inline]
    fn first_position_of_subpath(&self) -> Option<Point2D<f32>> {
        self.subpath_ranges
            .last()
            .map(|subpath_range| self.endpoints[subpath_range.start as usize].to)
    }
}

impl FlatPathBuilder for Builder {
    type PathType = ();

    #[inline]
    fn build(self) {}

    #[inline]
    fn build_and_reset(&mut self) {
        self.endpoints.clear();
        self.subpath_ranges.clear();
    }

    #[inline]
    fn current_position(&self) -> Point2D<f32> {
        match self.endpoints.last() {
            None => Point2D::zero(),
            Some(endpoint) => endpoint.to,
        }
    }

    fn close(&mut self) {
        let first_position_of_subpath = match self.first_position_of_subpath() {
            None => return,
            Some(first_position_of_subpath) => first_position_of_subpath,
        };

        if first_position_of_subpath == self.current_position() {
            return
        }

        self.add_endpoint(None, first_position_of_subpath);
        self.end_subpath();
    }

    fn move_to(&mut self, to: Point2D<f32>) {
        self.end_subpath();
        let last_endpoint_index = self.endpoints.len() as u32;
        self.subpath_ranges.push(last_endpoint_index..last_endpoint_index);
        self.add_endpoint(None, to);
    }

    #[inline]
    fn line_to(&mut self, to: Point2D<f32>) {
        self.add_endpoint(None, to);
    }
}

impl PathBuilder for Builder {
    fn quadratic_bezier_to(&mut self, ctrl: Point2D<f32>, to: Point2D<f32>) {
        let segment = QuadraticBezierSegment {
            from: self.current_position(),
            ctrl: ctrl,
            to: to,
        };

        //self.add_endpoint(Some(ctrl), to);

        // Split at X tangent.
        let mut worklist: ArrayVec<[QuadraticBezierSegment<f32>; 2]> = ArrayVec::new();
        match segment.find_local_x_extremum() {
            Some(t) if t > TANGENT_PARAMETER_TOLERANCE &&
                    t < 1.0 - TANGENT_PARAMETER_TOLERANCE => {
                let subsegments = segment.split(t);
                worklist.push(subsegments.0);
                worklist.push(subsegments.1);
            }
            _ => worklist.push(segment),
        }

        // Split at Y tangent.
        for segment in worklist {
            match segment.find_local_y_extremum() {
                Some(t) if t > TANGENT_PARAMETER_TOLERANCE &&
                        t < 1.0 - TANGENT_PARAMETER_TOLERANCE => {
                    let subsegments = segment.split(t);
                    self.add_endpoint(Some(subsegments.0.ctrl), subsegments.0.to);
                    self.add_endpoint(Some(subsegments.1.ctrl), subsegments.1.to);
                }
                _ => self.add_endpoint(Some(segment.ctrl), segment.to),
            }
        }
    }

    fn cubic_bezier_to(&mut self, ctrl1: Point2D<f32>, ctrl2: Point2D<f32>, to: Point2D<f32>) {
        let cubic_segment = CubicBezierSegment {
            from: self.current_position(),
            ctrl1: ctrl1,
            ctrl2: ctrl2,
            to: to,
        };

        for quadratic_segment in CubicToQuadraticSegmentIter::new(&cubic_segment,
                                                                  CUBIC_APPROX_TOLERANCE) {
            self.quadratic_bezier_to(quadratic_segment.ctrl, quadratic_segment.to)
        }
    }

    fn arc(&mut self,
           _center: Point2D<f32>,
           _radii: Vector2D<f32>,
           _angle: Angle<f32>,
           _x_rotation: Angle<f32>) {
        panic!("TODO: Support arcs in the Pathfinder builder!")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Endpoint {
    pub to: Point2D<f32>,
    pub ctrl: Option<Point2D<f32>>,
    pub subpath_index: u32,
}
