// pathfinder/partitioner/src/lib.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate bincode;
extern crate bit_vec;
extern crate byteorder;
extern crate env_logger;
extern crate euclid;
extern crate pathfinder_path_utils;
extern crate serde;

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

use euclid::Point2D;
use pathfinder_path_utils::{Endpoint, Subpath};
use std::u32;

pub mod capi;
pub mod mesh_library;
pub mod partitioner;

mod bold;

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
