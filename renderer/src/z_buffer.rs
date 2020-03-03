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

use crate::builder::SolidTile;
use crate::gpu_data::{SolidTileBatch, SolidTileVertex};
use crate::paint::{PaintId, PaintMetadata};
use crate::tile_map::DenseTileMap;
use crate::tiles;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2I;
use vec_map::VecMap;

pub(crate) struct ZBuffer {
    buffer: DenseTileMap<u32>,
    depth_to_paint_id: VecMap<PaintId>,
}

pub(crate) struct SolidTiles {
    pub(crate) batches: Vec<SolidTileBatch>,
}

impl ZBuffer {
    pub(crate) fn new(view_box: RectF) -> ZBuffer {
        let tile_rect = tiles::round_rect_out_to_tile_bounds(view_box);
        ZBuffer {
            buffer: DenseTileMap::from_builder(|_| 0, tile_rect),
            depth_to_paint_id: VecMap::new(),
        }
    }

    pub(crate) fn test(&self, coords: Vector2I, depth: u32) -> bool {
        let tile_index = self.buffer.coords_to_index_unchecked(coords);
        self.buffer.data[tile_index as usize] < depth
    }

    pub(crate) fn update(&mut self, solid_tiles: &[SolidTile], depth: u32, paint_id: PaintId) {
        self.depth_to_paint_id.insert(depth as usize, paint_id);
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

            let paint_id = self.depth_to_paint_id[depth as usize];
            let paint_metadata = &paint_metadata[paint_id.0 as usize];

            let tile_position = tile_coords + self.buffer.rect.origin();

            // Create a batch if necessary.
            match solid_tiles.batches.last() {
                Some(ref batch) if batch.paint_page == paint_metadata.tex_page &&
                    batch.sampling_flags == paint_metadata.sampling_flags => {}
                _ => {
                    // Batch break.
                    solid_tiles.batches.push(SolidTileBatch {
                        paint_page: paint_metadata.tex_page,
                        sampling_flags: paint_metadata.sampling_flags,
                        vertices: vec![],
                    });
                }
            }

            let batch = solid_tiles.batches.last_mut().unwrap();
            batch.vertices.extend_from_slice(&[
                SolidTileVertex::new(tile_position, paint_metadata),
                SolidTileVertex::new(tile_position + Vector2I::new(1, 0), paint_metadata),
                SolidTileVertex::new(tile_position + Vector2I::new(0, 1), paint_metadata),
                SolidTileVertex::new(tile_position + Vector2I::new(1, 1), paint_metadata),
            ]);
        }

        solid_tiles
    }
}

impl SolidTileVertex {
    fn new(tile_position: Vector2I, paint_metadata: &PaintMetadata) -> SolidTileVertex {
        let color_uv = paint_metadata.calculate_tex_coords(tile_position);
        SolidTileVertex {
            tile_x: tile_position.x() as i16,
            tile_y: tile_position.y() as i16,
            object_index: 0,
            color_u: color_uv.x(),
            color_v: color_uv.y(),
            pad: 0,
        }
    }
}
