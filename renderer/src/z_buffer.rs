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

use crate::gpu_data::{BuiltObject, SolidTileScenePrimitive};
use crate::scene;
use crate::tiles;
use pathfinder_geometry::basic::rect::{RectF32, RectI32};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

pub struct ZBuffer {
    buffer: Vec<AtomicUsize>,
    tile_rect: RectI32,
}

impl ZBuffer {
    pub fn new(view_box: RectF32) -> ZBuffer {
        let tile_rect = tiles::round_rect_out_to_tile_bounds(view_box);
        let tile_area = tile_rect.size().x() as usize * tile_rect.size().y() as usize;
        ZBuffer {
            buffer: (0..tile_area).map(|_| AtomicUsize::new(0)).collect(),
            tile_rect,
        }
    }

    pub fn test(&self, scene_tile_index: u32, object_index: u32) -> bool {
        let existing_depth = self.buffer[scene_tile_index as usize].load(AtomicOrdering::SeqCst);
        existing_depth < object_index as usize + 1
    }

    pub fn update(&self, tile_x: i32, tile_y: i32, object_index: u16) {
        let scene_tile_index = scene::scene_tile_index(tile_x, tile_y, self.tile_rect) as usize;
        let mut old_depth = self.buffer[scene_tile_index].load(AtomicOrdering::SeqCst);
        let new_depth = (object_index + 1) as usize;
        while old_depth < new_depth {
            let prev_depth = self.buffer[scene_tile_index].compare_and_swap(
                old_depth,
                new_depth,
                AtomicOrdering::SeqCst,
            );
            if prev_depth == old_depth {
                // Successfully written.
                return;
            }
            old_depth = prev_depth;
        }
    }

    pub fn build_solid_tiles(
        &self,
        objects: &[BuiltObject],
        tile_rect: RectI32,
    ) -> Vec<SolidTileScenePrimitive> {
        let mut solid_tiles = vec![];
        for scene_tile_y in 0..tile_rect.size().y() {
            for scene_tile_x in 0..tile_rect.size().x() {
                let scene_tile_index =
                    scene_tile_y as usize * tile_rect.size().x() as usize + scene_tile_x as usize;
                let depth = self.buffer[scene_tile_index].load(AtomicOrdering::Relaxed);
                if depth == 0 {
                    continue;
                }
                let object_index = (depth - 1) as usize;
                solid_tiles.push(SolidTileScenePrimitive {
                    tile_x: (scene_tile_x + tile_rect.min_x()) as i16,
                    tile_y: (scene_tile_y + tile_rect.min_y()) as i16,
                    object_index: object_index as u16,
                });
            }
        }

        solid_tiles
    }
}
