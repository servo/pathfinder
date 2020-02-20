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
use crate::gpu_data::SolidTileVertex;
use crate::paint::PaintMetadata;
use crate::scene::DrawPath;
use crate::tile_map::DenseTileMap;
use crate::tiles;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2I;

pub(crate) struct ZBuffer {
    buffer: DenseTileMap<u32>,
}

impl ZBuffer {
    pub(crate) fn new(view_box: RectF) -> ZBuffer {
        let tile_rect = tiles::round_rect_out_to_tile_bounds(view_box);
        ZBuffer {
            buffer: DenseTileMap::from_builder(|_| 0, tile_rect),
        }
    }

    pub(crate) fn test(&self, coords: Vector2I, object_index: u32) -> bool {
        let tile_index = self.buffer.coords_to_index_unchecked(coords);
        self.buffer.data[tile_index as usize] < object_index + 1
    }

    pub(crate) fn update(&mut self, solid_tiles: &[SolidTile], object_index: u32) {
        for solid_tile in solid_tiles {
            let tile_index = self.buffer.coords_to_index_unchecked(solid_tile.coords);
            let z_dest = &mut self.buffer.data[tile_index as usize];
            *z_dest = u32::max(*z_dest, object_index + 1);
        }
    }

    pub(crate) fn build_solid_tiles(&self, paths: &[DrawPath], paint_metadata: &[PaintMetadata])
                                    -> Vec<SolidTileVertex> {
        let mut solid_tiles = vec![];
        for tile_index in 0..self.buffer.data.len() {
            let depth = self.buffer.data[tile_index];
            if depth == 0 {
                continue;
            }

            let tile_coords = self.buffer.index_to_coords(tile_index);
            let object_index = (depth - 1) as u32;

            let paint_id = paths[object_index as usize].paint();
            let paint_metadata = &paint_metadata[paint_id.0 as usize];

            let tile_position = tile_coords + self.buffer.rect.origin();
            let object_index = object_index as u16;

            solid_tiles.extend_from_slice(&[
                SolidTileVertex::new(tile_position, object_index, paint_metadata),
                SolidTileVertex::new(tile_position + Vector2I::new(1, 0),
                                     object_index,
                                     paint_metadata),
                SolidTileVertex::new(tile_position + Vector2I::new(0, 1),
                                     object_index,
                                     paint_metadata),
                SolidTileVertex::new(tile_position + Vector2I::new(1, 1),
                                     object_index,
                                     paint_metadata),
            ]);
        }

        solid_tiles
    }
}

impl SolidTileVertex {
    fn new(tile_position: Vector2I, object_index: u16, paint_metadata: &PaintMetadata)
           -> SolidTileVertex {
        let color_uv = paint_metadata.calculate_tex_coords(tile_position).scale(65535.0).to_i32();
        SolidTileVertex {
            tile_x: tile_position.x() as i16,
            tile_y: tile_position.y() as i16,
            object_index: object_index,
            color_u: color_uv.x() as u16,
            color_v: color_uv.y() as u16,
            pad: 0,
        }
    }
}
