// partitionfinder/lib.rs

#![feature(alloc_jemalloc)]

// Needed to work around a problem with `heapsize`
extern crate alloc_jemalloc;
extern crate bit_vec;
extern crate euclid;

use euclid::Point2D;
use std::u32;

pub mod capi;
pub mod geometry;
pub mod partitioner;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Bezieroid {
    pub upper_prev_endpoint: u32,
    pub upper_next_endpoint: u32,
    pub lower_prev_endpoint: u32,
    pub lower_next_endpoint: u32,
    pub upper_left_time: f32,
    pub upper_right_time: f32,
    pub lower_left_time: f32,
    pub lower_right_time: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Endpoint {
    pub position: Point2D<f32>,
    /// `u32::MAX` if not present.
    pub control_points_index: u32,
    pub subpath_index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ControlPoints {
    pub point1: Point2D<f32>,
    pub point2: Point2D<f32>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Subpath {
    pub first_endpoint_index: u32,
}
