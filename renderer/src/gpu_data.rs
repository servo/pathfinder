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
use crate::paint::PaintCompositeOp;
use pathfinder_color::ColorU;
use pathfinder_content::effects::{BlendMode, Filter};
use pathfinder_content::fill::FillRule;
use pathfinder_content::render_target::RenderTargetId;
use pathfinder_geometry::line_segment::{LineSegmentU4, LineSegmentU8};
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::Vector2I;
use pathfinder_gpu::TextureSamplingFlags;
use std::fmt::{Debug, Formatter, Result as DebugResult};
use std::sync::Arc;
use std::time::Duration;

pub enum RenderCommand {
    // Starts rendering a frame.
    Start {
        /// The number of paths that will be rendered.
        path_count: usize,

        /// A bounding quad for the scene.
        bounding_quad: BoundingQuad,

        /// Whether the framebuffer we're rendering to must be readable.
        ///
        /// This is needed if a path that renders directly to the output framebuffer (i.e. not to a
        /// render target) uses one of the more exotic blend modes.
        needs_readable_framebuffer: bool,
    },

    // Allocates texture pages for the frame.
    AllocateTexturePages(Vec<TexturePageDescriptor>),

    // Uploads data to a texture page.
    UploadTexelData { texels: Arc<Vec<ColorU>>, location: TextureLocation },

    // Associates a render target with a texture page.
    //
    // TODO(pcwalton): Add a rect to this so we can render to subrects of a page.
    DeclareRenderTarget { id: RenderTargetId, location: TextureLocation },

    // Upload texture metadata.
    UploadTextureMetadata(Vec<TextureMetadataEntry>),

    // Adds fills to the queue.
    AddFills(Vec<FillBatchEntry>),

    // Flushes the queue of fills.
    FlushFills,

    // Pushes a render target onto the stack. Draw commands go to the render target on top of the
    // stack.
    PushRenderTarget(RenderTargetId),

    // Pops a render target from the stack.
    PopRenderTarget,

    // Marks that tile compositing is about to begin.
    BeginTileDrawing,

    // Draws a batch of tiles to the render target on top of the stack.
    DrawTiles(TileBatch),

    // Presents a rendered frame.
    Finish { cpu_build_time: Duration },
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct TexturePageId(pub u32);

#[derive(Clone, Debug)]
pub struct TexturePageDescriptor {
    pub size: Vector2I,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct TextureLocation {
    pub page: TexturePageId,
    pub rect: RectI,
}

#[derive(Clone, Debug)]
pub struct TileBatch {
    pub tiles: Vec<Tile>,
    pub color_texture: Option<TileBatchTexture>,
    pub mask_0_fill_rule: Option<FillRule>,
    pub mask_1_fill_rule: Option<FillRule>,
    pub filter: Filter,
    pub blend_mode: BlendMode,
    pub tile_page: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileBatchTexture {
    pub page: TexturePageId,
    pub sampling_flags: TextureSamplingFlags,
    pub composite_op: PaintCompositeOp,
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
    pub alpha_tile_id: AlphaTileId,
    pub backdrop: i8,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct TextureMetadataEntry {
    pub color_0_transform: Transform2F,
    pub base_color: ColorU,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FillBatchEntry {
    pub fill: Fill,
    pub page: u16,
}

// FIXME(pcwalton): Move `subpx` before `px` and remove `repr(packed)`.
#[derive(Clone, Copy, Debug, Default)]
#[repr(packed)]
pub struct Fill {
    pub px: LineSegmentU4,
    pub subpx: LineSegmentU8,
    pub alpha_tile_index: u16,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Tile {
    pub tile_x: i16,
    pub tile_y: i16,
    pub mask_0_u: u8,
    pub mask_0_v: u8,
    pub mask_1_u: u8,
    pub mask_1_v: u8,
    pub mask_0_backdrop: i8,
    pub mask_1_backdrop: i8,
    pub color: u16,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AlphaTileId(pub u32);

impl AlphaTileId {
    #[inline]
    pub fn invalid() -> AlphaTileId {
        AlphaTileId(!0)
    }

    #[inline]
    pub fn page(self) -> u16 {
        (self.0 >> 16) as u16
    }

    #[inline]
    pub fn tile(self) -> u16 {
        (self.0 & 0xffff) as u16
    }

    #[inline]
    pub fn is_valid(self) -> bool {
        self.0 < !0
    }
}

impl Debug for RenderCommand {
    fn fmt(&self, formatter: &mut Formatter) -> DebugResult {
        match *self {
            RenderCommand::Start { .. } => write!(formatter, "Start"),
            RenderCommand::AllocateTexturePages(ref pages) => {
                write!(formatter, "AllocateTexturePages(x{})", pages.len())
            }
            RenderCommand::UploadTexelData { ref texels, location } => {
                write!(formatter, "UploadTexelData(x{:?}, {:?})", texels.len(), location)
            }
            RenderCommand::DeclareRenderTarget { id, location } => {
                write!(formatter, "DeclareRenderTarget({:?}, {:?})", id, location)
            }
            RenderCommand::UploadTextureMetadata(ref metadata) => {
                write!(formatter, "UploadTextureMetadata(x{})", metadata.len())
            }
            RenderCommand::AddFills(ref fills) => {
                write!(formatter, "AddFills(x{})", fills.len())
            }
            RenderCommand::FlushFills => write!(formatter, "FlushFills"),
            RenderCommand::PushRenderTarget(render_target_id) => {
                write!(formatter, "PushRenderTarget({:?})", render_target_id)
            }
            RenderCommand::PopRenderTarget => write!(formatter, "PopRenderTarget"),
            RenderCommand::BeginTileDrawing => write!(formatter, "BeginTileDrawing"),
            RenderCommand::DrawTiles(ref batch) => {
                write!(formatter,
                       "DrawTiles(x{}, C0 {:?}, M0 {:?}, {:?})",
                       batch.tiles.len(),
                       batch.color_texture,
                       batch.mask_0_fill_rule,
                       batch.blend_mode)
            }
            RenderCommand::Finish { cpu_build_time } => {
                write!(formatter, "Finish({} ms)", cpu_build_time.as_secs_f64() * 1000.0)
            }
        }
    }
}
