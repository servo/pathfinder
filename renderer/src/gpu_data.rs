// pathfinder/renderer/src/gpu_data.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Packed data ready to be sent to the GPU.

use crate::paint::{ObjectShader, ShaderId};
use crate::tiles::{self, TILE_HEIGHT, TILE_WIDTH};
use euclid::Rect;
use fixedbitset::FixedBitSet;
use pathfinder_geometry::line_segment::{LineSegmentF32, LineSegmentU4, LineSegmentU8};
use pathfinder_geometry::point::Point2DF32;
use pathfinder_geometry::util;
use pathfinder_simd::default::{F32x4, I32x4};

#[derive(Debug)]
pub struct BuiltObject {
    pub bounds: Rect<f32>,
    pub tile_rect: Rect<i16>,
    pub tiles: Vec<TileObjectPrimitive>,
    pub fills: Vec<FillObjectPrimitive>,
    pub solid_tiles: FixedBitSet,
    pub shader: ShaderId,
}

#[derive(Debug)]
pub struct BuiltScene {
    pub view_box: Rect<f32>,
    pub batches: Vec<Batch>,
    pub solid_tiles: Vec<SolidTileScenePrimitive>,
    pub shaders: Vec<ObjectShader>,
}

#[derive(Debug)]
pub struct Batch {
    pub fills: Vec<FillBatchPrimitive>,
    pub mask_tiles: Vec<MaskTileBatchPrimitive>,
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
    pub tile_x: i16,
    pub tile_y: i16,
    pub backdrop: i16,
}

// FIXME(pcwalton): Move `subpx` before `px` and remove `repr(packed)`.
#[derive(Clone, Copy, Debug)]
#[repr(packed)]
pub struct FillBatchPrimitive {
    pub px: LineSegmentU4,
    pub subpx: LineSegmentU8,
    pub mask_tile_index: u16,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SolidTileScenePrimitive {
    pub tile_x: i16,
    pub tile_y: i16,
    pub shader: ShaderId,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct MaskTileBatchPrimitive {
    pub tile: TileObjectPrimitive,
    pub shader: ShaderId,
}

// Utilities for built objects

impl BuiltObject {
    pub fn new(bounds: &Rect<f32>, shader: ShaderId) -> BuiltObject {
        // Compute the tile rect.
        let tile_rect = tiles::round_rect_out_to_tile_bounds(&bounds);

        // Allocate tiles.
        let tile_count = tile_rect.size.width as usize * tile_rect.size.height as usize;
        let mut tiles = Vec::with_capacity(tile_count);
        for y in tile_rect.origin.y..tile_rect.max_y() {
            for x in tile_rect.origin.x..tile_rect.max_x() {
                tiles.push(TileObjectPrimitive::new(x, y));
            }
        }

        let mut solid_tiles = FixedBitSet::with_capacity(tile_count);
        solid_tiles.insert_range(..);

        BuiltObject {
            bounds: *bounds,
            tile_rect,
            tiles,
            fills: vec![],
            solid_tiles,
            shader,
        }
    }

    // TODO(pcwalton): SIMD-ify `tile_x` and `tile_y`.
    fn add_fill(&mut self, segment: &LineSegmentF32, tile_x: i16, tile_y: i16) {
        //println!("add_fill({:?} ({}, {}))", segment, tile_x, tile_y);

        let mut segment = (segment.0 * F32x4::splat(256.0)).to_i32x4();

        let tile_origin_x = (TILE_WIDTH as i32) * 256 * (tile_x as i32);
        let tile_origin_y = (TILE_HEIGHT as i32) * 256 * (tile_y as i32);
        let tile_origin = I32x4::new(tile_origin_x, tile_origin_y, tile_origin_x, tile_origin_y);

        segment = segment - tile_origin;
        /*
        println!("... before min: {} {} {} {}",
                    segment[0], segment[1], segment[2], segment[3]);
        */
        //segment = Sse41::max_epi32(segment, Sse41::setzero_epi32());
        segment = segment.min(I32x4::splat(0x0fff));
        //println!("... after min: {} {} {} {}", segment[0], segment[1], segment[2], segment[3]);

        let shuffle_mask = I32x4::new(0x0c08_0400, 0x0d05_0901, 0, 0);
        segment = segment
            .as_u8x16()
            .shuffle(shuffle_mask.as_u8x16())
            .as_i32x4();

        // Unpack whole and fractional pixels.
        let px = LineSegmentU4((segment[1] | (segment[1] >> 12)) as u16);
        let subpx = LineSegmentU8(segment[0] as u32);

        // Cull degenerate fills.
        if (px.0 & 0xf) as u8 == ((px.0 >> 8) & 0xf) as u8 &&
                (subpx.0 & 0xff) as u8 == ((subpx.0 >> 16) & 0xff) as u8 {
            //println!("... ... culling!");
            return;
        }

        let tile_index = self.tile_coords_to_index(tile_x, tile_y);

        //println!("... ... OK, pushing");

        self.fills.push(FillObjectPrimitive {
            px,
            subpx,
            tile_x,
            tile_y,
        });
        self.solid_tiles.set(tile_index as usize, false);
    }

    pub fn add_active_fill(
        &mut self,
        left: f32,
        right: f32,
        mut winding: i16,
        tile_x: i16,
        tile_y: i16,
    ) {
        let tile_origin_y = (i32::from(tile_y) * TILE_HEIGHT as i32) as f32;
        let left = Point2DF32::new(left, tile_origin_y);
        let right = Point2DF32::new(right, tile_origin_y);

        let segment = if winding < 0 {
            LineSegmentF32::new(&left, &right)
        } else {
            LineSegmentF32::new(&right, &left)
        };

        /*println!("... emitting active fill {} -> {} winding {} @ tile {},{}",
                 left.x(),
                 right.x(),
                 winding,
                 tile_x,
                 tile_y);*/

        while winding != 0 {
            self.add_fill(&segment, tile_x, tile_y);
            if winding < 0 {
                winding += 1
            } else {
                winding -= 1
            }
        }
    }

    // TODO(pcwalton): Optimize this better with SIMD!
    pub fn generate_fill_primitives_for_line(&mut self, mut segment: LineSegmentF32, tile_y: i16) {
        /*println!("... generate_fill_primitives_for_line(): segment={:?} tile_y={} ({}-{})",
                    segment,
                    tile_y,
                    tile_y as f32 * TILE_HEIGHT as f32,
                    (tile_y + 1) as f32 * TILE_HEIGHT as f32);*/

        let winding = segment.from_x() > segment.to_x();
        let (segment_left, segment_right) = if !winding {
            (segment.from_x(), segment.to_x())
        } else {
            (segment.to_x(), segment.from_x())
        };

        let segment_tile_left = (f32::floor(segment_left) as i32 / TILE_WIDTH as i32) as i16;
        let segment_tile_right =
            util::alignup_i32(f32::ceil(segment_right) as i32, TILE_WIDTH as i32) as i16;
        /*println!("segment_tile_left={} segment_tile_right={} tile_rect={:?}",
                 segment_tile_left, segment_tile_right, self.tile_rect);*/

        for subsegment_tile_x in segment_tile_left..segment_tile_right {
            let (mut fill_from, mut fill_to) = (segment.from(), segment.to());
            let subsegment_tile_right =
                ((i32::from(subsegment_tile_x) + 1) * TILE_HEIGHT as i32) as f32;
            if subsegment_tile_right < segment_right {
                let x = subsegment_tile_right;
                let point = Point2DF32::new(x, segment.solve_y_for_x(x));
                if !winding {
                    fill_to = point;
                    segment = LineSegmentF32::new(&point, &segment.to());
                } else {
                    fill_from = point;
                    segment = LineSegmentF32::new(&segment.from(), &point);
                }
            }

            let fill_segment = LineSegmentF32::new(&fill_from, &fill_to);
            self.add_fill(&fill_segment, subsegment_tile_x, tile_y);
        }
    }

    // FIXME(pcwalton): Use a `Point2D<i16>` instead?
    pub fn tile_coords_to_index(&self, tile_x: i16, tile_y: i16) -> u32 {
        /*println!("tile_coords_to_index(x={}, y={}, tile_rect={:?})",
        tile_x,
        tile_y,
        self.tile_rect);*/
        (tile_y - self.tile_rect.origin.y) as u32 * self.tile_rect.size.width as u32
            + (tile_x - self.tile_rect.origin.x) as u32
    }

    pub fn get_tile_mut(&mut self, tile_x: i16, tile_y: i16) -> &mut TileObjectPrimitive {
        let tile_index = self.tile_coords_to_index(tile_x, tile_y);
        &mut self.tiles[tile_index as usize]
    }
}

impl BuiltScene {
    #[inline]
    pub fn new(view_box: &Rect<f32>) -> BuiltScene {
        BuiltScene {
            view_box: *view_box,
            batches: vec![],
            solid_tiles: vec![],
            shaders: vec![],
        }
    }
}

impl Batch {
    #[inline]
    pub fn new() -> Batch {
        Batch {
            fills: vec![],
            mask_tiles: vec![],
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.mask_tiles.is_empty()
    }
}

impl TileObjectPrimitive {
    #[inline]
    fn new(tile_x: i16, tile_y: i16) -> TileObjectPrimitive {
        TileObjectPrimitive {
            tile_x,
            tile_y,
            backdrop: 0,
        }
    }
}
