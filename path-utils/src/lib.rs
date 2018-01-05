// pathfinder/path-utils/src/lib.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Various utilities for manipulating Bézier curves.
//!
//! On its own, the partitioner can only generate meshes for fill operations on quadratic Bézier
//! curves. Frequently, however, other vector drawing operations are desired: for example,
//! rendering cubic Béziers or stroking paths. These utilities can convert those complex operations
//! into simpler sequences of quadratic Béziers that the partitioner can handle.

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

/// A series of commands that define quadratic Bézier paths.
/// 
/// For cubics, see the `cubic` module.
#[derive(Clone, Copy, Debug)]
pub enum PathCommand {
    /// Moves the pen to the given point.
    MoveTo(Point2D<f32>),
    /// Draws a line to the given point.
    LineTo(Point2D<f32>),
    /// Draws a quadratic curve with the control point to the endpoint, respectively.
    CurveTo(Point2D<f32>, Point2D<f32>),
    /// Closes the current subpath by drawing a line from the current point to the first point of
    /// the subpath.
    ClosePath,
}

/// Holds one or more paths in memory in an efficient form.
/// 
/// This structure is generally preferable to `Vec<PathCommand>` if you need to buffer paths in
/// memory. It is both smaller and offers random access to individual subpaths.
#[derive(Clone, Debug)]
pub struct PathBuffer {
    /// All endpoints of all subpaths.
    pub endpoints: Vec<Endpoint>,
    /// All control points of all subpaths.
    pub control_points: Vec<Point2D<f32>>,
    /// A series of ranges defining each subpath.
    pub subpaths: Vec<Subpath>,
}

impl PathBuffer {
    /// Creates a new, empty path buffer.
    #[inline]
    pub fn new() -> PathBuffer {
        PathBuffer {
            endpoints: vec![],
            control_points: vec![],
            subpaths: vec![],
        }
    }

    /// Appends a sequence of path commands to this path buffer.
    pub fn add_stream<I>(&mut self, stream: I) where I: Iterator<Item = PathCommand> {
        let mut first_subpath_endpoint_index = self.endpoints.len() as u32;
        for segment in stream {
            match segment {
                PathCommand::ClosePath => self.close_subpath(&mut first_subpath_endpoint_index),

                PathCommand::MoveTo(position) => {
                    self.end_subpath(&mut first_subpath_endpoint_index);
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

        self.end_subpath(&mut first_subpath_endpoint_index)
    }

    fn close_subpath(&mut self, first_subpath_endpoint_index: &mut u32) {
        if self.endpoints.len() > *first_subpath_endpoint_index as usize {
            let first_endpoint = self.endpoints[*first_subpath_endpoint_index as usize];
            self.endpoints.push(first_endpoint);
        }

        self.do_end_subpath(first_subpath_endpoint_index, true)
    }

    fn end_subpath(&mut self, first_subpath_endpoint_index: &mut u32) {
        self.do_end_subpath(first_subpath_endpoint_index, false)
    }

    fn do_end_subpath(&mut self, first_subpath_endpoint_index: &mut u32, closed: bool) {
        let last_subpath_endpoint_index = self.endpoints.len() as u32;
        if *first_subpath_endpoint_index != last_subpath_endpoint_index {
            self.subpaths.push(Subpath {
                first_endpoint_index: *first_subpath_endpoint_index,
                last_endpoint_index: last_subpath_endpoint_index,
                closed: closed,
            })
        }

        *first_subpath_endpoint_index = last_subpath_endpoint_index;
    }

    /// Reverses the winding order of the subpath with the given index.
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

/// Converts a path buffer back into a series of path commands.
#[derive(Clone)]
pub struct PathBufferStream<'a> {
    path_buffer: &'a PathBuffer,
    endpoint_index: u32,
    subpath_index: u32,
    last_subpath_index: u32,
}

impl<'a> PathBufferStream<'a> {
    /// Prepares a path buffer stream to stream all subpaths from the given path buffer.
    #[inline]
    pub fn new<'b>(path_buffer: &'b PathBuffer) -> PathBufferStream<'b> {
        PathBufferStream {
            path_buffer: path_buffer,
            endpoint_index: 0,
            subpath_index: 0,
            last_subpath_index: path_buffer.subpaths.len() as u32,
        }
    }

    /// Prepares a path buffer stream to stream only a subset of subpaths from the given path
    /// buffer.
    #[inline]
    pub fn subpath_range<'b>(path_buffer: &'b PathBuffer, subpath_range: Range<u32>)
                             -> PathBufferStream<'b> {
        let first_endpoint_index = if subpath_range.start == subpath_range.end {
            0
        } else {
            path_buffer.subpaths[subpath_range.start as usize].first_endpoint_index
        };
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
            if subpath.closed {
                return Some(PathCommand::ClosePath)
            }
            return self.next()
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

/// Describes a path endpoint in a path buffer.
#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Endpoint {
    /// The 2D position of the endpoint.
    pub position: Point2D<f32>,
    /// The index of the control point *before* this endpoint in the `control_points` vector, or
    /// `u32::MAX` if this endpoint is the end of a line segment.
    pub control_point_index: u32,
    /// The index of the subpath that this endpoint belongs to.
    pub subpath_index: u32,
}

/// Stores the endpoint indices of each subpath.
#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Subpath {
    /// The index of the first endpoint that makes up this subpath.
    pub first_endpoint_index: u32,
    /// One plus the index of the last endpoint that makes up this subpath.
    pub last_endpoint_index: u32,
    /// Whether the subpath is closed (i.e. fully connected).
    pub closed: bool,
}

impl Subpath {
    /// Returns the endpoint indices as a `Range`.
    #[inline]
    pub fn range(self) -> Range<usize> {
        (self.first_endpoint_index as usize)..(self.last_endpoint_index as usize)
    }
}

/// Represents a single path segment (i.e. a single side of a Béziergon).
#[derive(Debug, Clone, Copy)]
pub enum PathSegment {
    /// A line segment with two endpoints.
    Line(Point2D<f32>, Point2D<f32>),
    /// A quadratic Bézier curve with an endpoint, a control point, and another endpoint, in that
    /// order.
    Curve(Point2D<f32>, Point2D<f32>, Point2D<f32>),
}

/// Yields a set of `PathSegment`s corresponding to a list of `PathCommand`s.
/// 
/// For example, the path commands `[MoveTo(A), LineTo(B), LineTo(C), ClosePath]` become
/// `[Line(A, B), Line(B, C), Line(C, A)]`.
/// 
/// This representation can simplify the implementation of certain geometric algorithms, such as
/// offset paths (stroking).
pub struct PathSegmentStream<I> {
    inner: I,
    current_subpath_index: u32,
    current_point: Point2D<f32>,
    current_subpath_start_point: Point2D<f32>,
}

impl<I> PathSegmentStream<I> where I: Iterator<Item = PathCommand> {
    /// Creates a new path segment stream that will yield path segments from the given collection
    /// of path commands.
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
                    if points_are_sufficiently_far_apart(&self.current_point, &endpoint) {
                        let start_point = mem::replace(&mut self.current_point, endpoint);
                        return Some((PathSegment::Line(start_point, endpoint),
                                    self.current_subpath_index))
                    }
                }
                Some(PathCommand::CurveTo(control_point, endpoint)) => {
                    if points_are_sufficiently_far_apart(&self.current_point, &endpoint) {
                        let start_point = mem::replace(&mut self.current_point, endpoint);
                        return Some((PathSegment::Curve(start_point, control_point, endpoint),
                                    self.current_subpath_index))
                    }
                }
                Some(PathCommand::ClosePath) => {
                    let endpoint = self.current_subpath_start_point;
                    if points_are_sufficiently_far_apart(&self.current_point, &endpoint) {
                        let start_point = mem::replace(&mut self.current_point, endpoint);
                        return Some((PathSegment::Line(start_point, endpoint),
                                     self.current_subpath_index))
                    }
                }
            }
        }

        fn points_are_sufficiently_far_apart(point_a: &Point2D<f32>, point_b: &Point2D<f32>)
                                             -> bool {
            (point_a.x - point_b.x).abs() > 0.001 ||
                (point_a.y - point_b.y).abs() > 0.001
        }
    }
}

/// Applies an affine transform to a path stream and yields the resulting path stream.
pub struct Transform2DPathStream<I> {
    inner: I,
    transform: Transform2D<f32>,
}

impl<I> Transform2DPathStream<I> where I: Iterator<Item = PathCommand> {
    /// Creates a new transformed path stream from a path stream.
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

/// Linear interpolation: `lerp(a, b, t)` = `a + (b - a) * t`.
#[inline]
pub(crate) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Returns -1.0 if the value is negative, 1.0 if the value is positive, or 0.0 if the value is
/// zero.
/// 
/// Returns NaN when given NaN.
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
