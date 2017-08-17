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
extern crate serde;
#[macro_use]
extern crate serde_derive;

use euclid::Point2D;
use std::u32;

pub mod capi;
pub mod geometry;
pub mod legalizer;
pub mod partitioner;

#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BQuad {
    pub upper_left_vertex_index: u32,
    pub upper_right_vertex_index: u32,
    pub upper_control_point_vertex_index: u32,
    pad0: u32,
    pub lower_left_vertex_index: u32,
    pub lower_right_vertex_index: u32,
    pub lower_control_point_vertex_index: u32,
    pad1: u32,
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
            pad0: 0,
            pad1: 0,
        }
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

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
pub enum AntialiasingMode {
    Msaa = 0,
    Ecaa = 1,
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
pub(crate) enum BVertexKind {
    Endpoint0,
    Endpoint1,
    ConvexControlPoint,
    ConcaveControlPoint,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct BVertexLoopBlinnData {
    pub tex_coord: [u8; 2],
    pub sign: i8,
    pad: u8,
}

impl BVertexLoopBlinnData {
    #[inline]
    pub(crate) fn new(kind: BVertexKind) -> BVertexLoopBlinnData {
        let (tex_coord, sign) = match kind {
            BVertexKind::Endpoint0 => ([0, 0], 0),
            BVertexKind::Endpoint1 => ([2, 2], 0),
            BVertexKind::ConcaveControlPoint => ([1, 0], 1),
            BVertexKind::ConvexControlPoint => ([1, 0], -1),
        };
        BVertexLoopBlinnData {
            tex_coord: tex_coord,
            sign: sign,
            pad: 0,
        }
    }

    pub(crate) fn control_point(left_endpoint_position: &Point2D<f32>,
                                control_point_position: &Point2D<f32>,
                                right_endpoint_position: &Point2D<f32>,
                                bottom: bool)
                                -> BVertexLoopBlinnData {
        let control_point_vector = *control_point_position - *left_endpoint_position;
        let right_vector = *right_endpoint_position - *left_endpoint_position;
        let determinant = right_vector.cross(control_point_vector);
        let endpoint_kind = if (determinant < 0.0) ^ bottom {
            BVertexKind::ConvexControlPoint
        } else {
            BVertexKind::ConcaveControlPoint
        };
        BVertexLoopBlinnData::new(endpoint_kind)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
