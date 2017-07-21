// partitionfinder/lib.rs

#![feature(alloc_jemalloc)]

// needed to work around a problem with `heapsize`
extern crate alloc_jemalloc;
extern crate bit_vec;
extern crate env_logger;
extern crate euclid;
extern crate half;
#[macro_use]
extern crate log;

use euclid::point2d;
use std::u32;

pub mod capi;
pub mod geometry;
pub mod legalizer;
pub mod partitioner;
pub mod tessellator;

#[repr(u8)]
#[derive(debug, clone, copy, partialeq)]
pub enum shape {
    flat = 0,
    convex = 1,
    concave = 2,
}

#[repr(c)]
#[derive(debug, clone, copy)]
pub struct bquad {
    pub upper_left_vertex_index: u32,
    pub upper_control_point_vertex_index: u32,
    pub upper_right_vertex_index: u32,
    pub lower_left_vertex_index: u32,
    pub lower_control_point_vertex_index: u32,
    pub lower_right_vertex_index: u32,
    pad: [u32; 2],
}

impl BQuad {
    #[inline]
    pub fn new(upper_left_vertex_index: u32,
               upper_control_point_vertex_index: u32,
               upper_right_vertex_index: u32,
               lower_left_vertex_index: u32,
               lower_control_point_vertex_index: u32,
               lower_right_vertex_index: u32)
               -> BQuad {
        BQuad {
            upper_left_vertex_index: upper_left_vertex_index,
            upper_control_point_vertex_index: upper_control_point_vertex_index,
            upper_right_vertex_index: upper_right_vertex_index,
            lower_left_vertex_index: lower_left_vertex_index,
            lower_control_point_vertex_index: lower_control_point_vertex_index,
            lower_right_vertex_index: lower_right_vertex_index,
            pad: [0; 2],
        }
    }

    #[inline]
    pub fn upper_left_vertex_index(&self) -> u32 {
        self.start_index + 0
    }

    #[inline]
    pub fn lower_left_vertex_index(&self) -> u32 {
        self.start_index + 1
    }

    #[inline]
    pub fn upper_right_vertex_index(&self) -> u32 {
        self.start_index + 3
    }

    #[inline]
    pub fn lower_right_vertex_index(&self) -> u32 {
        self.start_index + 4
    }

    #[inline]
    pub fn upper_control_point_vertex_index(&self) -> u32 {
        match self.upper_shape {
            Shape::Flat => u32::MAX,
            Shape::Concave | Shape::Convex => self.start_index + 6,
        }
    }

    #[inline]
    pub fn lower_control_point_vertex_index(&self) -> u32 {
        match (self.upper_shape, self.lower_shape) {
            (_, Shape::Flat) => u32::MAX,
            (Shape::Flat, Shape::Convex) | (Shape::Flat, Shape::Concave) => self.start_index + 6,
            (Shape::Convex, Shape::Convex) |
            (Shape::Convex, Shape::Concave) |
            (Shape::Concave, Shape::Convex) |
            (Shape::Concave, Shape::Concave) => self.start_index + 9,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Endpoint {
    pub position: Point2D<f32>,
    /// `u32::MAX` if not present.
    pub control_point_index: u32,
    pub subpath_index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Subpath {
    pub first_endpoint_index: u32,
    pub last_endpoint_index: u32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
pub enum AntialiasingMode {
    Msaa = 0,
    Ecaa = 1,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct BVertex {
    pub position: Point2D<f32>,
    pub path_id: u32,
    pad: u32,
}

impl BVertex {
    #[inline]
    pub fn new(position: &Point2D<f32>, path_id: u32) -> BVertex {
        BVertex {
            position: *position,
            path_id: path_id,
            pad: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Vertex {
    pub left_b_vertex_index: u32,
    pub control_point_b_vertex_index: u32,
    pub right_b_vertex_index: u32,
    pub time: f32,
    pub path_id: u32,
    pad: u32,
}

impl Vertex {
    #[inline]
    pub fn new(path_id: u32,
               left_b_vertex_index: u32,
               control_point_b_vertex_index: u32,
               right_b_vertex_index: u32,
               time: f32)
               -> Vertex {
        Vertex {
            path_id: path_id,
            left_b_vertex_index: left_b_vertex_index,
            control_point_b_vertex_index: control_point_b_vertex_index,
            right_b_vertex_index: right_b_vertex_index,
            time: time,
            pad: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct EdgeInstance {
    pub left_vertex_index: u32,
    pub right_vertex_index: u32,
}

impl EdgeInstance {
    #[inline]
    pub fn new(left_vertex_index: u32, right_vertex_index: u32) -> EdgeInstance {
        EdgeInstance {
            left_vertex_index: left_vertex_index,
            right_vertex_index: right_vertex_index,
        }
    }
}
