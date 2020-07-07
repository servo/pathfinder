// pathfinder/renderer/src/tiles.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::gpu_data::{TILE_CTRL_MASK_0_SHIFT, TILE_CTRL_MASK_EVEN_ODD};
use crate::gpu_data::{TILE_CTRL_MASK_WINDING, TileObjectPrimitive};
use crate::paint::PaintId;
use pathfinder_content::effects::BlendMode;
use pathfinder_content::fill::FillRule;
use pathfinder_geometry::rect::{RectF, RectI};
use pathfinder_geometry::vector::vec2f;

pub const TILE_WIDTH: u32 = 16;
pub const TILE_HEIGHT: u32 = 16;

#[derive(Clone, Copy)]
pub(crate) enum TilingPathInfo {
    Clip,
    Draw(DrawTilingPathInfo),
}

#[derive(Clone, Copy)]
pub(crate) struct DrawTilingPathInfo {
    pub(crate) paint_id: PaintId,
    pub(crate) blend_mode: BlendMode,
    pub(crate) fill_rule: FillRule,
}

impl TilingPathInfo {
    pub(crate) fn has_destructive_blend_mode(&self) -> bool {
        match *self {
            TilingPathInfo::Draw(ref draw_tiling_path_info) => {
                draw_tiling_path_info.blend_mode.is_destructive()
            }
            TilingPathInfo::Clip => false,
        }
    }

    pub(crate) fn to_ctrl(&self) -> u8 {
        let mut ctrl = 0;
        match *self {
            TilingPathInfo::Draw(ref draw_tiling_path_info) => {
                match draw_tiling_path_info.fill_rule {
                    FillRule::EvenOdd => {
                        ctrl |= (TILE_CTRL_MASK_EVEN_ODD << TILE_CTRL_MASK_0_SHIFT) as u8
                    }
                    FillRule::Winding => {
                        ctrl |= (TILE_CTRL_MASK_WINDING << TILE_CTRL_MASK_0_SHIFT) as u8
                    }
                }
            }
            TilingPathInfo::Clip => {}
        }
        ctrl
    }
}

pub fn round_rect_out_to_tile_bounds(rect: RectF) -> RectI {
    (rect * vec2f(1.0 / TILE_WIDTH as f32, 1.0 / TILE_HEIGHT as f32)).round_out().to_i32()
}

impl TileObjectPrimitive {
    #[inline]
    pub fn is_solid(&self) -> bool { !self.alpha_tile_id.is_valid() }
}
