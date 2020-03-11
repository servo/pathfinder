// pathfinder/renderer/src/z_buffer.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Software occlusion culling.

use crate::builder::SolidTileInfo;
use crate::gpu_data::{SolidTile, SolidTileBatch, SolidTileVertex};
use crate::paint::{PaintId, PaintMetadata};
use crate::tile_map::DenseTileMap;
use crate::tiles;
use pathfinder_content::effects::{CompositeOp, Effects, Filter};
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{Vector2F, Vector2I};
use vec_map::VecMap;

pub(crate) struct ZBuffer {
    buffer: DenseTileMap<u32>,
    depth_metadata: VecMap<DepthMetadata>,
}

pub(crate) struct SolidTiles {
    pub(crate) batches: Vec<SolidTileBatch>,
}

#[derive(Clone, Copy)]
pub(crate) struct DepthMetadata {
    pub(crate) paint_id: PaintId,
}
impl ZBuffer {
    pub(crate) fn new(view_box: RectF) -> ZBuffer {
        let tile_rect = tiles::round_rect_out_to_tile_bounds(view_box);
        ZBuffer {
            buffer: DenseTileMap::from_builder(|_| 0, tile_rect),
            depth_metadata: VecMap::new(),
        }
    }

    pub(crate) fn test(&self, coords: Vector2I, depth: u32) -> bool {
        let tile_index = self.buffer.coords_to_index_unchecked(coords);
        self.buffer.data[tile_index as usize] < depth
    }

    pub(crate) fn update(&mut self,
                         solid_tiles: &[SolidTileInfo],
                         depth: u32,
                         metadata: DepthMetadata) {
        self.depth_metadata.insert(depth as usize, metadata);
        for solid_tile in solid_tiles {
            let tile_index = self.buffer.coords_to_index_unchecked(solid_tile.coords);
            let z_dest = &mut self.buffer.data[tile_index as usize];
            *z_dest = u32::max(*z_dest, depth);
        }
    }

    pub(crate) fn build_solid_tiles(&self, paint_metadata: &[PaintMetadata]) -> SolidTiles {
        let mut solid_tiles = SolidTiles { batches: vec![] };

        for tile_index in 0..self.buffer.data.len() {
            let depth = self.buffer.data[tile_index];
            if depth == 0 {
                continue;
            }

            let tile_coords = self.buffer.index_to_coords(tile_index);

            let depth_metadata = self.depth_metadata[depth as usize];
            let paint_metadata = &paint_metadata[depth_metadata.paint_id.0 as usize];

            let tile_position = tile_coords + self.buffer.rect.origin();

            // Create a batch if necessary.
            match solid_tiles.batches.last() {
                Some(ref batch) if batch.color_texture_page == paint_metadata.location.page &&
                    batch.sampling_flags == paint_metadata.sampling_flags => {}
                _ => {
                    // Batch break.
                    //
                    // TODO(pcwalton): We could be more aggressive with batching here, since we
                    // know there are no overlaps.
                    solid_tiles.batches.push(SolidTileBatch {
                        color_texture_page: paint_metadata.location.page,
                        sampling_flags: paint_metadata.sampling_flags,
                        tiles: vec![],
                        effects: Effects::new(Filter::Composite(CompositeOp::SrcOver)),
                    });
                }
            }

            let batch = solid_tiles.batches.last_mut().unwrap();
            batch.tiles.push(SolidTile::from_paint_metadata(tile_position, paint_metadata));
        }

        solid_tiles
    }
}

impl SolidTile {
    pub(crate) fn from_paint_metadata(tile_position: Vector2I, paint_metadata: &PaintMetadata)
                                      -> SolidTile {
        SolidTile {
            upper_left: SolidTileVertex::from_paint_metadata(tile_position, paint_metadata),
            upper_right: SolidTileVertex::from_paint_metadata(tile_position + Vector2I::new(1, 0),
                                                              paint_metadata),
            lower_left: SolidTileVertex::from_paint_metadata(tile_position + Vector2I::new(0, 1),
                                                             paint_metadata),
            lower_right: SolidTileVertex::from_paint_metadata(tile_position + Vector2I::new(1, 1),
                                                              paint_metadata),
        }
    }

    // The texture rect is in normalized coordinates.
    pub(crate) fn from_texture_rect(tile_position: Vector2I, texture_rect: RectF) -> SolidTile {
        SolidTile {
            upper_left: SolidTileVertex::new(tile_position, texture_rect.origin()),
            upper_right: SolidTileVertex::new(tile_position + Vector2I::new(1, 0),
                                              texture_rect.upper_right()),
            lower_left: SolidTileVertex::new(tile_position + Vector2I::new(0, 1),
                                             texture_rect.lower_left()),
            lower_right: SolidTileVertex::new(tile_position + Vector2I::new(1, 1),
                                              texture_rect.lower_right()),
        }
    }
}

impl SolidTileVertex {
    fn new(tile_position: Vector2I, color_tex_coords: Vector2F) -> SolidTileVertex {
        SolidTileVertex {
            tile_x: tile_position.x() as i16,
            tile_y: tile_position.y() as i16,
            color_u: color_tex_coords.x(),
            color_v: color_tex_coords.y(),
        }
    }

    fn from_paint_metadata(tile_position: Vector2I, paint_metadata: &PaintMetadata)
                           -> SolidTileVertex {
        let color_uv = paint_metadata.calculate_tex_coords(tile_position);
        SolidTileVertex {
            tile_x: tile_position.x() as i16,
            tile_y: tile_position.y() as i16,
            color_u: color_uv.x(),
            color_v: color_uv.y(),
        }
    }
}
