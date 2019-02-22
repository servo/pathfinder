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
use fixedbitset::FixedBitSet;
use pathfinder_geometry::basic::line_segment::{LineSegmentF32, LineSegmentU4, LineSegmentU8};
use pathfinder_geometry::basic::point::{Point2DF32, Point3DF32};
use pathfinder_geometry::basic::rect::{RectF32, RectI32};
use pathfinder_geometry::util;
use pathfinder_simd::default::{F32x4, I32x4};

#[derive(Debug)]
pub struct BuiltObject {
    pub bounds: RectF32,
    pub tile_rect: RectI32,
    pub tiles: Vec<TileObjectPrimitive>,
    pub fills: Vec<FillObjectPrimitive>,
    pub solid_tiles: FixedBitSet,
    pub shader: ShaderId,
}

#[derive(Debug)]
pub struct BuiltScene {
    pub view_box: RectF32,
    pub quad: [Point3DF32; 4],
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
    pub fn new(bounds: RectF32, shader: ShaderId) -> BuiltObject {
        // Compute the tile rect.
        let tile_rect = tiles::round_rect_out_to_tile_bounds(bounds);

        // Allocate tiles.
        let tile_count = tile_rect.size().x() as usize * tile_rect.size().y() as usize;
        let mut tiles = Vec::with_capacity(tile_count);
        for y in tile_rect.min_y()..tile_rect.max_y() {
            for x in tile_rect.min_x()..tile_rect.max_x() {
                tiles.push(TileObjectPrimitive::new(x as i16, y as i16));
            }
        }

        let mut solid_tiles = FixedBitSet::with_capacity(tile_count);
        solid_tiles.insert_range(..);

        BuiltObject { bounds, tile_rect, tiles, fills: vec![], solid_tiles, shader }
    }

    // TODO(pcwalton): SIMD-ify `tile_x` and `tile_y`.
    fn add_fill(&mut self, segment: &LineSegmentF32, tile_x: i32, tile_y: i32) {
        //println!("add_fill({:?} ({}, {}))", segment, tile_x, tile_y);
        let tile_index = match self.tile_coords_to_index(tile_x, tile_y) {
            None => return,
            Some(tile_index) => tile_index,
        };

        debug_assert_eq!(TILE_WIDTH, TILE_HEIGHT);
        let tile_size = F32x4::splat(TILE_WIDTH as f32);
        let (min, max) = (F32x4::default(), F32x4::splat((TILE_WIDTH * 256 - 1) as f32));
        let shuffle_mask = I32x4::new(0x0c08_0400, 0x0d05_0901, 0, 0).as_u8x16();

        let tile_upper_left =
            F32x4::new(tile_x as f32, tile_y as f32, tile_x as f32, tile_y as f32) * tile_size;

        let segment = (segment.0 - tile_upper_left) * F32x4::splat(256.0);
        let segment =
            segment.clamp(min, max).to_i32x4().as_u8x16().shuffle(shuffle_mask).as_i32x4();

        // Unpack whole and fractional pixels.
        let px = LineSegmentU4((segment[1] | (segment[1] >> 12)) as u16);
        let subpx = LineSegmentU8(segment[0] as u32);

        // Cull degenerate fills.
        if (px.0 & 0xf) as u8 == ((px.0 >> 8) & 0xf) as u8 &&
                (subpx.0 & 0xff) as u8 == ((subpx.0 >> 16) & 0xff) as u8 {
            //println!("... ... culling!");
            return;
        }

        //println!("... ... OK, pushing");

        self.fills.push(FillObjectPrimitive {
            px,
            subpx,
            tile_x: tile_x as i16,
            tile_y: tile_y as i16,
        });
        self.solid_tiles.set(tile_index as usize, false);
    }

    pub fn add_active_fill(
        &mut self,
        left: f32,
        right: f32,
        mut winding: i16,
        tile_x: i32,
        tile_y: i32,
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

    pub fn generate_fill_primitives_for_line(&mut self, mut segment: LineSegmentF32, tile_y: i32) {
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

        // FIXME(pcwalton): Optimize this.
        let segment_tile_left = f32::floor(segment_left) as i32 / TILE_WIDTH as i32;
        let segment_tile_right =
            util::alignup_i32(f32::ceil(segment_right) as i32, TILE_WIDTH as i32);
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

    // FIXME(pcwalton): Use a `Point2DI32` instead?
    pub fn tile_coords_to_index(&self, tile_x: i32, tile_y: i32) -> Option<u32> {
        /*println!("tile_coords_to_index(x={}, y={}, tile_rect={:?})",
        tile_x,
        tile_y,
        self.tile_rect);*/
        if tile_x < self.tile_rect.min_x() || tile_x >= self.tile_rect.max_x() ||
                tile_y < self.tile_rect.min_y() || tile_y >= self.tile_rect.max_y() {
            None
        } else {
            Some((tile_y - self.tile_rect.min_y()) as u32 * self.tile_rect.size().x() as u32
                + (tile_x - self.tile_rect.min_x()) as u32)
        }
    }

    pub fn get_tile_mut(&mut self, tile_x: i32, tile_y: i32) -> Option<&mut TileObjectPrimitive> {
        let tile_index = self.tile_coords_to_index(tile_x, tile_y);
        match tile_index {
            None => None,
            Some(tile_index) => Some(&mut self.tiles[tile_index as usize]),
        }
    }
}

impl BuiltScene {
    #[inline]
    pub fn new(view_box: RectF32, quad: &[Point3DF32; 4]) -> BuiltScene {
        BuiltScene { view_box, quad: *quad, batches: vec![], solid_tiles: vec![], shaders: vec![] }
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
