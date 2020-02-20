// pathfinder/renderer/src/gpu_data.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Packed data ready to be sent to the GPU.

use crate::gpu::renderer::PostprocessOptions;
use crate::options::BoundingQuad;
use pathfinder_color::ColorU;
use pathfinder_content::fill::FillRule;
use pathfinder_geometry::line_segment::{LineSegmentU4, LineSegmentU8};
use pathfinder_geometry::vector::Vector2I;
use std::fmt::{Debug, Formatter, Result as DebugResult};
use std::time::Duration;

pub enum RenderCommand {
    Start { path_count: usize, bounding_quad: BoundingQuad },
    AddPaintData(PaintData),
    AddFills(Vec<FillBatchPrimitive>),
    FlushFills,
    RenderMaskTiles { tiles: Vec<MaskTile>, fill_rule: FillRule },
    PushLayer { effects: PostprocessOptions },
    PopLayer,
    DrawAlphaTiles(Vec<AlphaTile>),
    DrawSolidTiles(Vec<SolidTileVertex>),
    Finish { build_time: Duration },
}

#[derive(Clone, Debug)]
pub struct PaintData {
    pub size: Vector2I,
    pub texels: Vec<ColorU>,
}

#[derive(Clone, Copy, Debug)]
pub struct FillObjectPrimitive {
    pub px: LineSegmentU4,
    pub subpx: LineSegmentU8,
    pub tile_x: i16,
    pub tile_y: i16,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct TileObjectPrimitive {
    /// If `u16::MAX`, then this is a solid tile.
    pub alpha_tile_index: u16,
    pub backdrop: i8,
}

// FIXME(pcwalton): Move `subpx` before `px` and remove `repr(packed)`.
#[derive(Clone, Copy, Debug, Default)]
#[repr(packed)]
pub struct FillBatchPrimitive {
    pub px: LineSegmentU4,
    pub subpx: LineSegmentU8,
    pub alpha_tile_index: u16,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SolidTileVertex {
    pub tile_x: i16,
    pub tile_y: i16,
    pub color_u: u16,
    pub color_v: u16,
    pub object_index: u16,
    pub pad: u16,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct MaskTile {
    pub upper_left: MaskTileVertex,
    pub upper_right: MaskTileVertex,
    pub lower_left: MaskTileVertex,
    pub lower_right: MaskTileVertex,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct AlphaTile {
    pub upper_left: AlphaTileVertex,
    pub upper_right: AlphaTileVertex,
    pub lower_left: AlphaTileVertex,
    pub lower_right: AlphaTileVertex,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct MaskTileVertex {
    pub mask_u: u16,
    pub mask_v: u16,
    pub fill_u: u16,
    pub fill_v: u16,
    pub backdrop: i16,
    pub object_index: u16,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct AlphaTileVertex {
    pub tile_x: i16,
    pub tile_y: i16,
    pub mask_u: u16,
    pub mask_v: u16,
    pub color_u: u16,
    pub color_v: u16,
    pub object_index: u16,
    pub pad: u16,
}

impl Debug for RenderCommand {
    fn fmt(&self, formatter: &mut Formatter) -> DebugResult {
        match *self {
            RenderCommand::Start { .. } => write!(formatter, "Start"),
            RenderCommand::AddPaintData(ref paint_data) => {
                write!(formatter, "AddPaintData({}x{})", paint_data.size.x(), paint_data.size.y())
            }
            RenderCommand::AddFills(ref fills) => write!(formatter, "AddFills(x{})", fills.len()),
            RenderCommand::FlushFills => write!(formatter, "FlushFills"),
            RenderCommand::RenderMaskTiles { ref tiles, fill_rule } => {
                write!(formatter, "RenderMaskTiles(x{}, {:?})", tiles.len(), fill_rule)
            }
            RenderCommand::PushLayer { .. } => write!(formatter, "PushLayer"),
            RenderCommand::PopLayer => write!(formatter, "PopLayer"),
            RenderCommand::DrawAlphaTiles(ref tiles) => {
                write!(formatter, "DrawAlphaTiles(x{})", tiles.len())
            }
            RenderCommand::DrawSolidTiles(ref tiles) => {
                write!(formatter, "DrawSolidTiles(x{})", tiles.len())
            }
            RenderCommand::Finish { .. } => write!(formatter, "Finish"),
        }
    }
}
