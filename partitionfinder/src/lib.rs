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

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
pub enum BVertexKind {
    Endpoint0 = 0,
    Endpoint1 = 1,
    ConvexControlPoint = 2,
    ConcaveControlPoint = 3,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct BVertex {
    pub position: Point2D<f32>,
    pub path_id: u32,
    pub tex_coord: [u8; 2],
    pub kind: BVertexKind,
    pad: u8,
}

impl BVertex {
    #[inline]
    pub fn new(position: &Point2D<f32>, kind: BVertexKind, path_id: u32) -> BVertex {
        let tex_coord = match kind {
            BVertexKind::Endpoint0 => [0, 0],
            BVertexKind::Endpoint1 => [2, 2],
            BVertexKind::ConcaveControlPoint | BVertexKind::ConvexControlPoint => [1, 0],
        };
        BVertex {
            position: *position,
            path_id: path_id,
            tex_coord: tex_coord,
            kind: kind,
            pad: 0,
        }
    }

    pub(crate) fn control_point(left_endpoint_position: &Point2D<f32>,
                                control_point_position: &Point2D<f32>,
                                right_endpoint_position: &Point2D<f32>,
                                path_id: u32,
                                bottom: bool)
                                -> BVertex {
        let control_point_vector = *control_point_position - *left_endpoint_position;
        let right_vector = *right_endpoint_position - *left_endpoint_position;
        let determinant = right_vector.cross(control_point_vector);
        let endpoint_kind = if (determinant < 0.0) ^ bottom {
            BVertexKind::ConvexControlPoint
        } else {
            BVertexKind::ConcaveControlPoint
        };
        BVertex::new(control_point_position, endpoint_kind, path_id)
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

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct LineIndices {
    pub left_vertex_index: u32,
    pub right_vertex_index: u32,
}

impl LineIndices {
    #[inline]
    pub fn new(left_vertex_index: u32, right_vertex_index: u32) -> LineIndices {
        LineIndices {
            left_vertex_index: left_vertex_index,
            right_vertex_index: right_vertex_index,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct CurveIndices {
    pub left_vertex_index: u32,
    pub right_vertex_index: u32,
    pub control_point_vertex_index: u32,
    pad: u32,
}

impl CurveIndices {
    #[inline]
    pub fn new(left_vertex_index: u32, control_point_vertex_index: u32, right_vertex_index: u32)
               -> CurveIndices {
        CurveIndices {
            left_vertex_index: left_vertex_index,
            right_vertex_index: right_vertex_index,
            control_point_vertex_index: control_point_vertex_index,
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
