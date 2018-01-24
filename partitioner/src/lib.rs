// pathfinder/partitioner/src/lib.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Processes paths into *mesh libraries*, which are vertex buffers ready to be uploaded to the
//! GPU and rendered with the supplied shaders.
//! 
//! *Partitioning* is the process of cutting up a filled Bézier path into *B-quads*. A B-quad is
//! the core primitive that Pathfinder renders; it is a trapezoid-like shape that consists of
//! vertical sides on the left and right and Bézier curve segments and/or lines on the top and
//! bottom. Path partitioning is typically O(*n* log *n*) in the number of path commands.
//! 
//! If you have a static set of paths (for example, one specific font), you may wish to run the
//! partitioner as a preprocessing step and store the resulting mesh library on disk. To aid this
//! use case, mesh libraries can be serialized into a simple binary format. Of course, meshes can
//! also be generated dynamically and rendered on the fly.

extern crate arrayvec;
extern crate bincode;
extern crate bit_vec;
extern crate byteorder;
extern crate env_logger;
extern crate euclid;
extern crate lyon_geom;
extern crate lyon_path;
extern crate serde;

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

use euclid::Point2D;
use std::u32;

pub mod mesh_library;
pub mod monotonic;
pub mod partitioner;

mod indexed_path;
mod normal;

/// The fill rule.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FillRule {
    EvenOdd = 0,
    Winding = 1,
}

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

    #[inline]
    pub fn offset(&mut self, delta: u32) {
        self.upper_left_vertex_index += delta;
        self.upper_right_vertex_index += delta;
        self.lower_left_vertex_index += delta;
        self.lower_right_vertex_index += delta;
        if self.upper_control_point_vertex_index < u32::MAX {
            self.upper_control_point_vertex_index += delta;
        }
        if self.lower_control_point_vertex_index < u32::MAX {
            self.lower_control_point_vertex_index += delta;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct BQuadVertexPositions {
    pub upper_left_vertex_position: Point2D<f32>,
    pub upper_control_point_position: Point2D<f32>,
    pub upper_right_vertex_position: Point2D<f32>,
    pub lower_right_vertex_position: Point2D<f32>,
    pub lower_control_point_position: Point2D<f32>,
    pub lower_left_vertex_position: Point2D<f32>,
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

#[derive(Clone, Copy, Debug)]
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
