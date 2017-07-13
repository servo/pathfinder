// partitionfinder/lib.rs

#![feature(alloc_jemalloc)]

// Needed to work around a problem with `heapsize`
extern crate alloc_jemalloc;
extern crate bit_vec;
extern crate env_logger;
extern crate euclid;
extern crate half;
#[macro_use]
extern crate log;

use euclid::Point2D;
use std::u32;

pub mod capi;
pub mod geometry;
pub mod legalizer;
pub mod partitioner;
pub mod tessellator;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BQuad {
    pub upper_left_vertex: u32,
    pub upper_control_point: u32,
    pub upper_right_vertex: u32,
    pub lower_left_vertex: u32,
    pub lower_control_point: u32,
    pub lower_right_vertex: u32,
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
    Levien = 1,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Vertex {
    pub left_b_vertex_index: u32,
    pub right_b_vertex_index: u32,
    pub control_point_b_vertex_index: u32,
    pub time: f32,
}

impl Vertex {
    #[inline]
    pub fn new(left_b_vertex_index: u32,
               control_point_b_vertex_index: u32,
               right_b_vertex_index: u32,
               time: f32) -> Vertex {
        Vertex {
            left_b_vertex_index: left_b_vertex_index,
            control_point_b_vertex_index: control_point_b_vertex_index,
            right_b_vertex_index: right_b_vertex_index,
            time: time,
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct EdgeInstance {
    pub left_b_vertex_index: u32,
    pub control_point_b_vertex_index: u32,
    pub right_b_vertex_index: u32,
    pub left_time: f32,
    pub right_time: f32,
    padding: u32,
}

impl EdgeInstance {
    #[inline]
    pub fn new(left_b_vertex_index: u32,
               control_point_b_vertex_index: u32,
               right_b_vertex_index: u32,
               left_time: f32,
               right_time: f32)
               -> EdgeInstance {
        EdgeInstance {
            left_b_vertex_index: left_b_vertex_index,
            control_point_b_vertex_index: control_point_b_vertex_index,
            right_b_vertex_index: right_b_vertex_index,
            left_time: left_time,
            right_time: right_time,
            padding: 0,
        }
    }
}
