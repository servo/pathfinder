// pathfinder/renderer/src/tiles.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::builder::{BuiltPath, ObjectBuilder};
use crate::gpu_data::{AlphaTileId, TileObjectPrimitive};
use crate::paint::{PaintId, PaintMetadata};
use pathfinder_content::effects::BlendMode;
use pathfinder_content::fill::FillRule;
use pathfinder_geometry::rect::{RectF, RectI};
use pathfinder_geometry::vector::{Vector2I, vec2f};

pub const TILE_WIDTH: u32 = 16;
pub const TILE_HEIGHT: u32 = 16;

#[derive(Clone, Copy)]
pub(crate) enum TilingPathInfo<'a> {
    Clip,
    Draw(DrawTilingPathInfo<'a>),
}

#[derive(Clone, Copy)]
pub(crate) struct DrawTilingPathInfo<'a> {
    pub(crate) paint_id: PaintId,
    pub(crate) paint_metadata: &'a PaintMetadata,
    pub(crate) blend_mode: BlendMode,
    pub(crate) built_clip_path: Option<&'a BuiltPath>,
    pub(crate) fill_rule: FillRule,
}

impl<'a> TilingPathInfo<'a> {
    pub(crate) fn has_destructive_blend_mode(&self) -> bool {
        match *self {
            TilingPathInfo::Draw(ref draw_tiling_path_info) => {
                draw_tiling_path_info.blend_mode.is_destructive()
            }
            TilingPathInfo::Clip => false,
        }
    }
}

pub(crate) struct PackedTile<'a> {
    pub(crate) tile_type: TileType,
    pub(crate) tile_coords: Vector2I,
    pub(crate) draw_tile: &'a TileObjectPrimitive,
    pub(crate) clip_tile: Option<&'a TileObjectPrimitive>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum TileType {
    Solid,
    Empty,
    SingleMask,
}

impl<'a> PackedTile<'a> {
    pub(crate) fn new(draw_tile_index: u32,
                      draw_tile: &'a TileObjectPrimitive,
                      draw_tiling_path_info: &DrawTilingPathInfo<'a>,
                      object_builder: &ObjectBuilder)
                      -> PackedTile<'a> {
        let tile_coords = object_builder.local_tile_index_to_coords(draw_tile_index as u32);

        // First, if the draw tile is empty, cull it regardless of clip.
        if draw_tile.is_solid() {
            match (object_builder.built_path.fill_rule, draw_tile.backdrop) {
                (FillRule::Winding, 0) => {
                    return PackedTile {
                        tile_type: TileType::Empty,
                        tile_coords,
                        draw_tile,
                        clip_tile: None,
                    };
                }
                (FillRule::Winding, _) => {}
                (FillRule::EvenOdd, backdrop) if backdrop % 2 == 0 => {
                    return PackedTile {
                        tile_type: TileType::Empty,
                        tile_coords,
                        draw_tile,
                        clip_tile: None,
                    };
                }
                (FillRule::EvenOdd, _) => {}
            }
        }

        // Figure out what clip tile we need, if any.
        let clip_tile = match draw_tiling_path_info.built_clip_path {
            None => None,
            Some(built_clip_path) => {
                match built_clip_path.tiles.get(tile_coords) {
                    None => {
                        // This tile is outside of the bounds of the clip path entirely. We can
                        // cull it.
                        return PackedTile {
                            tile_type: TileType::Empty,
                            tile_coords,
                            draw_tile,
                            clip_tile: None,
                        };
                    }
                    Some(clip_tile) if clip_tile.is_solid() => {
                        if clip_tile.backdrop != 0 {
                            // The clip tile is fully opaque, so this tile isn't clipped at
                            // all.
                            None
                        } else {
                            // This tile is completely clipped out. Cull it.
                            return PackedTile {
                                tile_type: TileType::Empty,
                                tile_coords,
                                draw_tile,
                                clip_tile: None,
                            };
                        }
                    }
                    Some(clip_tile) => Some(clip_tile),
                }
            }
        };

        // Choose a tile type.
        match clip_tile {
            None if draw_tile.is_solid() => {
                // This is a solid tile that completely occludes the background.
                PackedTile { tile_type: TileType::Solid, tile_coords, draw_tile, clip_tile }
            }
            None => {
                // We have a draw tile and no clip tile.
                PackedTile {
                    tile_type: TileType::SingleMask,
                    tile_coords,
                    draw_tile,
                    clip_tile: None,
                }
            }
            Some(clip_tile) if draw_tile.is_solid() => {
                // We have a solid draw tile and a clip tile. This is effectively the same as
                // having a draw tile and no clip tile.
                //
                // FIXME(pcwalton): This doesn't preserve the fill rule of the clip path!
                PackedTile {
                    tile_type: TileType::SingleMask,
                    tile_coords,
                    draw_tile: clip_tile,
                    clip_tile: None,
                }
            }
            Some(clip_tile) => {
                // We have both a draw and clip mask. Composite them together.
                PackedTile {
                    tile_type: TileType::SingleMask,
                    tile_coords,
                    draw_tile,
                    clip_tile: Some(clip_tile),
                }
            }
        }
    }
}

pub fn round_rect_out_to_tile_bounds(rect: RectF) -> RectI {
    (rect * vec2f(1.0 / TILE_WIDTH as f32, 1.0 / TILE_HEIGHT as f32)).round_out().to_i32()
}

impl Default for TileObjectPrimitive {
    #[inline]
    fn default() -> TileObjectPrimitive {
        TileObjectPrimitive { backdrop: 0, alpha_tile_id: AlphaTileId::invalid() }
    }
}

impl TileObjectPrimitive {
    #[inline]
    pub fn is_solid(&self) -> bool { !self.alpha_tile_id.is_valid() }
}
