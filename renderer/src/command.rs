// pathfinder/renderer/src/gpu_data.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Rendering commands to be interpreted by the GPU.

use crate::manager::BoundingQuad;
use pathfinder_geometry::line_segment::{LineSegmentU4, LineSegmentU8};
use pathfinder_geometry::transform3d::Transform4F;
use pathfinder_geometry::vector::Vector2I;
use std::fmt::{Debug, Formatter, Result as DebugResult};
use std::time::Duration;

pub enum RenderCommand {
    Start { path_count: usize, bounding_quad: BoundingQuad },
    AddPaintData(PaintData),
    AddBlockFills { block: BlockKey, fills: Vec<FillBatchPrimitive> },
    FlushBlockFills,
    AddBlockTiles {
        block: BlockKey,
        alpha: Vec<AlphaTileBatchPrimitive>,
        solid: Vec<SolidTileBatchPrimitive>,
    },
    BeginComposite,
    CompositeBlock { block: BlockKey, transform: Transform4F },
    Finish { build_time: Duration },
}

#[derive(Clone, Debug)]
pub struct PaintData {
    pub size: Vector2I,
    pub texels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockKey(pub u32);

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct TileObjectPrimitive {
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
pub struct SolidTileBatchPrimitive {
    pub tile_x: i16,
    pub tile_y: i16,
    pub origin_u: u16,
    pub origin_v: u16,
    pub object_index: u16,
    pub pad: u16,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct AlphaTileBatchPrimitive {
    pub tile_x_lo: u8,
    pub tile_y_lo: u8,
    pub tile_hi: u8,
    pub backdrop: i8,
    pub object_index: u16,
    pub tile_index: u16,
    pub origin_u: u16,
    pub origin_v: u16,
}

impl BlockKey {
    #[inline]
    pub fn new(level: u32, tile_origin: Vector2I) -> BlockKey {
        debug_assert!(tile_origin.x() >= 0);
        debug_assert!(tile_origin.y() >= 0);
        debug_assert_eq!(tile_origin.x() & 0xff, 0);
        debug_assert_eq!(tile_origin.y() & 0xff, 0);
        BlockKey(level | (tile_origin.x() as u32) | ((tile_origin.y() as u32) << 16))
    }

    #[inline]
    pub fn tile_origin(self) -> Vector2I {
        Vector2I::new((self.0 & 0x000fff00) as i32, ((self.0 & 0xfff00000) >> 16) as i32)
    }

    #[inline]
    pub fn level(self) -> u32 {
        self.0 & 0xff
    }

    // TODO(pcwalton): Scales of < 1.
    #[inline]
    pub fn scale(self) -> u32 {
        1 << self.level()
    }
}

impl Debug for RenderCommand {
    fn fmt(&self, formatter: &mut Formatter) -> DebugResult {
        match *self {
            RenderCommand::Start { .. } => write!(formatter, "Start"),
            RenderCommand::AddPaintData(ref paint_data) => {
                write!(formatter, "AddPaintData({}x{})", paint_data.size.x(), paint_data.size.y())
            }
            RenderCommand::AddBlockFills { block, ref fills } => {
                write!(formatter, "AddBlockFills({:?}, x{})", block, fills.len())
            }
            RenderCommand::FlushBlockFills => write!(formatter, "FlushBlockFills"),
            RenderCommand::BeginComposite => write!(formatter, "BeginComposite"),
            RenderCommand::AddBlockTiles { block, ref alpha, ref solid } => {
                write!(formatter,
                       "AddBlockTiles({:?}, A {}, S {})",
                       block,
                       alpha.len(),
                       solid.len())
            }
            RenderCommand::CompositeBlock { block, ref transform } => {
                write!(formatter, "CompositeBlock({:?}, {:?})", block, transform)
            }
            RenderCommand::Finish { .. } => write!(formatter, "Finish"),
        }
    }
}
