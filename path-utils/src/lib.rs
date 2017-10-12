// pathfinder/path-utils/src/lib.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate arrayvec;
extern crate euclid;
#[macro_use]
extern crate serde_derive;

use euclid::{Point2D, Transform2D};
use std::mem;
use std::ops::Range;
use std::u32;

pub mod cubic;
pub mod curve;
pub mod intersection;
pub mod line;
pub mod monotonic;
pub mod stroke;
pub mod svg;

#[derive(Clone, Copy, Debug)]
pub enum PathCommand {
    MoveTo(Point2D<f32>),
    LineTo(Point2D<f32>),
    /// Control point and endpoint, respectively.
    CurveTo(Point2D<f32>, Point2D<f32>),
    ClosePath,
}

#[derive(Clone, Debug)]
pub struct PathBuffer {
    pub endpoints: Vec<Endpoint>,
    pub control_points: Vec<Point2D<f32>>,
    pub subpaths: Vec<Subpath>,
}

impl PathBuffer {
    #[inline]
    pub fn new() -> PathBuffer {
        PathBuffer {
            endpoints: vec![],
            control_points: vec![],
            subpaths: vec![],
        }
    }

    pub fn add_stream<I>(&mut self, stream: I) where I: Iterator<Item = PathCommand> {
        let mut first_subpath_endpoint_index = self.endpoints.len() as u32;
        for segment in stream {
            match segment {
                PathCommand::ClosePath => self.close_subpath(&mut first_subpath_endpoint_index),

                PathCommand::MoveTo(position) => {
                    self.close_subpath(&mut first_subpath_endpoint_index);
                    self.endpoints.push(Endpoint {
                        position: position,
                        control_point_index: u32::MAX,
                        subpath_index: self.subpaths.len() as u32,
                    })
                }

                PathCommand::LineTo(position) => {
                    self.endpoints.push(Endpoint {
                        position: position,
                        control_point_index: u32::MAX,
                        subpath_index: self.subpaths.len() as u32,
                    })
                }

                PathCommand::CurveTo(control_point_position, endpoint_position) => {
                    let control_point_index = self.control_points.len() as u32;
                    self.control_points.push(control_point_position);
                    self.endpoints.push(Endpoint {
                        position: endpoint_position,
                        control_point_index: control_point_index,
                        subpath_index: self.subpaths.len() as u32,
                    })
                }
            }
        }

        self.close_subpath(&mut first_subpath_endpoint_index)
    }

    fn close_subpath(&mut self, first_subpath_endpoint_index: &mut u32) {
        let last_subpath_endpoint_index = self.endpoints.len() as u32;
        if *first_subpath_endpoint_index != last_subpath_endpoint_index {
            self.subpaths.push(Subpath {
                first_endpoint_index: *first_subpath_endpoint_index,
                last_endpoint_index: last_subpath_endpoint_index,
            })
        }

        *first_subpath_endpoint_index = last_subpath_endpoint_index;
    }

    pub fn reverse_subpath(&mut self, subpath_index: u32) {
        let subpath = &self.subpaths[subpath_index as usize];
        let endpoint_range = subpath.range();
        if endpoint_range.start == endpoint_range.end {
            return
        }

        self.endpoints[endpoint_range.clone()].reverse();

        for endpoint_index in (endpoint_range.start..(endpoint_range.end - 1)).rev() {
            let control_point_index = self.endpoints[endpoint_index].control_point_index;
            self.endpoints[endpoint_index + 1].control_point_index = control_point_index;
        }

        self.endpoints[endpoint_range.start].control_point_index = u32::MAX;
    }
}

pub struct PathBufferStream<'a> {
    path_buffer: &'a PathBuffer,
    endpoint_index: u32,
    subpath_index: u32,
    last_subpath_index: u32,
}

impl<'a> PathBufferStream<'a> {
    #[inline]
    pub fn new<'b>(path_buffer: &'b PathBuffer) -> PathBufferStream<'b> {
        PathBufferStream {
            path_buffer: path_buffer,
            endpoint_index: 0,
            subpath_index: 0,
            last_subpath_index: path_buffer.subpaths.len() as u32,
        }
    }

    #[inline]
    pub fn subpath_range<'b>(path_buffer: &'b PathBuffer, subpath_range: Range<u32>)
                             -> PathBufferStream<'b> {
        let first_endpoint_index = path_buffer.subpaths[subpath_range.start as usize]
                                              .first_endpoint_index;
        PathBufferStream {
            path_buffer: path_buffer,
            endpoint_index: first_endpoint_index,
            subpath_index: subpath_range.start,
            last_subpath_index: subpath_range.end,
        }
    }
}

impl<'a> Iterator for PathBufferStream<'a> {
    type Item = PathCommand;

    fn next(&mut self) -> Option<PathCommand> {
        if self.subpath_index == self.last_subpath_index {
            return None
        }

        let subpath = &self.path_buffer.subpaths[self.subpath_index as usize];
        if self.endpoint_index == subpath.last_endpoint_index {
            self.subpath_index += 1;
            return Some(PathCommand::ClosePath)
        }

        let endpoint_index = self.endpoint_index;
        self.endpoint_index += 1;

        let endpoint = &self.path_buffer.endpoints[endpoint_index as usize];

        if endpoint_index == subpath.first_endpoint_index {
            return Some(PathCommand::MoveTo(endpoint.position))
        }

        if endpoint.control_point_index == u32::MAX {
            return Some(PathCommand::LineTo(endpoint.position))
        }

        let control_point = &self.path_buffer
                                 .control_points[endpoint.control_point_index as usize];
        Some(PathCommand::CurveTo(*control_point, endpoint.position))
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Endpoint {
    pub position: Point2D<f32>,
    /// `u32::MAX` if not present.
    pub control_point_index: u32,
    pub subpath_index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Subpath {
    pub first_endpoint_index: u32,
    pub last_endpoint_index: u32,
}

impl Subpath {
    #[inline]
    pub fn range(self) -> Range<usize> {
        (self.first_endpoint_index as usize)..(self.last_endpoint_index as usize)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PathSegment {
    /// First endpoint and second endpoint, respectively.
    Line(Point2D<f32>, Point2D<f32>),
    /// First endpoint, control point, and second endpoint, in that order.
    Curve(Point2D<f32>, Point2D<f32>, Point2D<f32>),
}

pub struct PathSegmentStream<I> {
    inner: I,
    current_subpath_index: u32,
    current_point: Point2D<f32>,
    current_subpath_start_point: Point2D<f32>,
}

impl<I> PathSegmentStream<I> where I: Iterator<Item = PathCommand> {
    pub fn new(inner: I) -> PathSegmentStream<I> {
        PathSegmentStream {
            inner: inner,
            current_subpath_index: u32::MAX,
            current_point: Point2D::zero(),
            current_subpath_start_point: Point2D::zero(),
        }
    }
}

impl<I> Iterator for PathSegmentStream<I> where I: Iterator<Item = PathCommand> {
    type Item = (PathSegment, u32);

    fn next(&mut self) -> Option<(PathSegment, u32)> {
        loop {
            match self.inner.next() {
                None => return None,
                Some(PathCommand::MoveTo(point)) => {
                    self.current_subpath_index = self.current_subpath_index.wrapping_add(1);
                    self.current_point = point;
                    self.current_subpath_start_point = point;
                }
                Some(PathCommand::LineTo(endpoint)) => {
                    let start_point = mem::replace(&mut self.current_point, endpoint);
                    return Some((PathSegment::Line(start_point, endpoint),
                                 self.current_subpath_index))
                }
                Some(PathCommand::CurveTo(control_point, endpoint)) => {
                    let start_point = mem::replace(&mut self.current_point, endpoint);
                    return Some((PathSegment::Curve(start_point, control_point, endpoint),
                                 self.current_subpath_index))
                }
                Some(PathCommand::ClosePath) => {
                    let start_point = mem::replace(&mut self.current_point,
                                                   self.current_subpath_start_point);
                    return Some((PathSegment::Line(start_point, self.current_subpath_start_point),
                                 self.current_subpath_index))
                }
            }
        }
    }
}

pub struct Transform2DPathStream<I> {
    inner: I,
    transform: Transform2D<f32>,
}

impl<I> Transform2DPathStream<I> where I: Iterator<Item = PathCommand> {
    pub fn new(inner: I, transform: &Transform2D<f32>) -> Transform2DPathStream<I> {
        Transform2DPathStream {
            inner: inner,
            transform: *transform,
        }
    }
}

impl<I> Iterator for Transform2DPathStream<I> where I: Iterator<Item = PathCommand> {
    type Item = PathCommand;

    fn next(&mut self) -> Option<PathCommand> {
        match self.inner.next() {
            None => None,
            Some(PathCommand::MoveTo(position)) => {
                Some(PathCommand::MoveTo(self.transform.transform_point(&position)))
            }
            Some(PathCommand::LineTo(position)) => {
                Some(PathCommand::LineTo(self.transform.transform_point(&position)))
            }
            Some(PathCommand::CurveTo(control_point_position, endpoint_position)) => {
                Some(PathCommand::CurveTo(self.transform.transform_point(&control_point_position),
                                          self.transform.transform_point(&endpoint_position)))
            }
            Some(PathCommand::ClosePath) => Some(PathCommand::ClosePath),
        }
    }
}

#[inline]
pub(crate) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
pub(crate) fn sign(x: f32) -> f32 {
    if x < 0.0 {
        -1.0
    } else if x > 0.0 {
        1.0
    } else {
        x
    }
}
