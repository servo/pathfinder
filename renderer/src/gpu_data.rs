// pathfinder/renderer/src/gpu_data.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Packed data ready to be sent to the GPU.

use crate::options::BoundingQuad;
use pathfinder_color::ColorU;
use pathfinder_content::effects::{BlendMode, Effects};
use pathfinder_content::fill::FillRule;
use pathfinder_content::pattern::RenderTargetId;
use pathfinder_geometry::line_segment::{LineSegmentU4, LineSegmentU8};
use pathfinder_geometry::vector::Vector2I;
use std::fmt::{Debug, Formatter, Result as DebugResult};
use std::time::Duration;

pub enum RenderCommand {
    // Starts rendering a frame.
    Start { path_count: usize, bounding_quad: BoundingQuad },

    // Uploads paint data for use with subsequent rendering commands to the GPU.
    AddPaintData(PaintData),

    // Adds fills to the queue.
    AddFills(Vec<FillBatchPrimitive>),

    // Flushes the queue of fills.
    FlushFills,

    // Render fills to a set of mask tiles.
    RenderMaskTiles { tiles: Vec<MaskTile>, fill_rule: FillRule },

    // Pushes a render target onto the stack. Draw commands go to the render target on top of the
    // stack.
    PushRenderTarget(RenderTargetId),

    // Pops a render target from the stack.
    PopRenderTarget,

    // Draws a batch of alpha tiles to the render target on top of the stack.
    DrawAlphaTiles { tiles: Vec<AlphaTile>, paint_page: PaintPageId, blend_mode: BlendMode },

    // Draws a batch of solid tiles to the render target on top of the stack.
    DrawSolidTiles(SolidTileBatch),

    // Draws an entire render target to the render target on top of the stack.
    //
    // FIXME(pcwalton): This draws the entire render target, so it's inefficient. We should get rid
    // of this command and transition all uses to `DrawAlphaTiles`/`DrawSolidTiles`. The reason it
    // exists is that we don't have logic to create tiles for blur bounding regions yet.
    DrawRenderTarget { render_target: RenderTargetId, effects: Effects },

    // Presents a rendered frame.
    Finish { build_time: Duration },
}

#[derive(Clone, Debug)]
pub struct PaintData {
    pub pages: Vec<PaintPageData>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PaintPageId(pub u32);

#[derive(Clone, Debug)]
pub struct PaintPageData {
    pub size: Vector2I,
    pub contents: PaintPageContents,
}

#[derive(Clone, Debug)]
pub enum PaintPageContents {
    Texels(Vec<ColorU>),
    RenderTarget(RenderTargetId),
}

#[derive(Clone, Debug)]
pub struct SolidTileBatch {
    pub vertices: Vec<SolidTileVertex>,
    pub paint_page: PaintPageId,
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
                write!(formatter, "AddPaintData(x{})", paint_data.pages.len())
            }
            RenderCommand::AddFills(ref fills) => write!(formatter, "AddFills(x{})", fills.len()),
            RenderCommand::FlushFills => write!(formatter, "FlushFills"),
            RenderCommand::RenderMaskTiles { ref tiles, fill_rule } => {
                write!(formatter, "RenderMaskTiles(x{}, {:?})", tiles.len(), fill_rule)
            }
            RenderCommand::PushRenderTarget(render_target_id) => {
                write!(formatter, "PushRenderTarget({:?})", render_target_id)
            }
            RenderCommand::PopRenderTarget => write!(formatter, "PopRenderTarget"),
            RenderCommand::DrawRenderTarget { render_target, .. } => {
                write!(formatter, "DrawRenderTarget({:?})", render_target)
            }
            RenderCommand::DrawAlphaTiles { ref tiles, paint_page, blend_mode } => {
                write!(formatter,
                       "DrawAlphaTiles(x{}, {:?}, {:?})",
                       tiles.len(),
                       paint_page,
                       blend_mode)
            }
            RenderCommand::DrawSolidTiles(ref batch) => {
                write!(formatter,
                       "DrawSolidTiles(x{}, {:?})",
                       batch.vertices.len(),
                       batch.paint_page)
            }
            RenderCommand::Finish { .. } => write!(formatter, "Finish"),
        }
    }
}
