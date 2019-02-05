// pathfinder/renderer/src/builder.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Packs data onto the GPU.

use crate::gpu_data::{Batch, BuiltObject, FillBatchPrimitive};
use crate::gpu_data::{MaskTileBatchPrimitive, SolidTileScenePrimitive};
use crate::scene;
use crate::tiles;
use crate::z_buffer::ZBuffer;
use euclid::Rect;
use pathfinder_geometry::basic::rect::RectF32;
use std::iter;
use std::u16;

const MAX_FILLS_PER_BATCH: usize = 0x0002_0000;
const MAX_MASKS_PER_BATCH: u16 = 0xffff;

pub struct SceneBuilder {
    objects: Vec<BuiltObject>,
    z_buffer: ZBuffer,
    tile_rect: Rect<i16>,

    current_object_index: usize,
}

impl SceneBuilder {
    pub fn new(objects: Vec<BuiltObject>, z_buffer: ZBuffer, view_box: RectF32) -> SceneBuilder {
        let tile_rect = tiles::round_rect_out_to_tile_bounds(view_box);
        SceneBuilder {
            objects,
            z_buffer,
            tile_rect,
            current_object_index: 0,
        }
    }

    pub fn build_solid_tiles(&self) -> Vec<SolidTileScenePrimitive> {
        self.z_buffer
            .build_solid_tiles(&self.objects, &self.tile_rect)
    }

    pub fn build_batch(&mut self) -> Option<Batch> {
        let mut batch = Batch::new();

        let mut object_tile_index_to_batch_mask_tile_index = vec![];
        while self.current_object_index < self.objects.len() {
            let object = &self.objects[self.current_object_index];

            if batch.fills.len() + object.fills.len() > MAX_FILLS_PER_BATCH {
                break;
            }

            object_tile_index_to_batch_mask_tile_index.clear();
            object_tile_index_to_batch_mask_tile_index
                .extend(iter::repeat(u16::MAX).take(object.tiles.len()));

            // Copy mask tiles.
            for (tile_index, tile) in object.tiles.iter().enumerate() {
                // Skip solid tiles, since we handled them above already.
                if object.solid_tiles[tile_index] {
                    continue;
                }

                // Cull occluded tiles.
                let scene_tile_index =
                    scene::scene_tile_index(tile.tile_x, tile.tile_y, self.tile_rect);
                if !self
                    .z_buffer
                    .test(scene_tile_index, self.current_object_index as u32)
                {
                    continue;
                }

                // Visible mask tile.
                let batch_mask_tile_index = batch.mask_tiles.len() as u16;
                if batch_mask_tile_index == MAX_MASKS_PER_BATCH {
                    break;
                }

                object_tile_index_to_batch_mask_tile_index[tile_index] = batch_mask_tile_index;

                batch.mask_tiles.push(MaskTileBatchPrimitive {
                    tile: *tile,
                    shader: object.shader,
                });
            }

            // Remap and copy fills, culling as necessary.
            for fill in &object.fills {
                let object_tile_index = object.tile_coords_to_index(fill.tile_x, fill.tile_y);
                let mask_tile_index =
                    object_tile_index_to_batch_mask_tile_index[object_tile_index as usize];
                if mask_tile_index < u16::MAX {
                    batch.fills.push(FillBatchPrimitive {
                        px: fill.px,
                        subpx: fill.subpx,
                        mask_tile_index,
                    });
                }
            }

            self.current_object_index += 1;
        }

        if batch.is_empty() {
            None
        } else {
            Some(batch)
        }
    }
}
