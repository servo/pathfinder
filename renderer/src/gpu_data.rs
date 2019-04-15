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

use atomic::Atomic;
use crate::scene::ObjectShader;
use crate::tile_map::DenseTileMap;
use crate::tiles::{self, TILE_HEIGHT, TILE_WIDTH};
use crate::z_buffer::ZBuffer;
use parking_lot::Mutex;
use pathfinder_geometry::basic::line_segment::{LineSegmentF32, LineSegmentU4, LineSegmentU8};
use pathfinder_geometry::basic::point::{Point2DF32, Point2DI32, Point3DF32};
use pathfinder_geometry::basic::rect::{RectF32, RectI32};
use pathfinder_geometry::util;
use pathfinder_simd::default::{F32x4, I32x4};
use std::fmt::{Debug, Formatter, Result as DebugResult};
use std::ops::Add;

#[derive(Debug)]
pub struct BuiltObject {
    pub bounds: RectF32,
    pub tiles: DenseTileMap<TileObjectPrimitive>,
}

#[derive(Debug)]
pub struct BuiltScene {
    pub view_box: RectF32,
    pub quad: [Point3DF32; 4],
    pub object_count: u32,
    pub shaders: Vec<ObjectShader>,
}

pub struct SharedBuffers {
    pub z_buffer: ZBuffer,
    pub alpha_tiles: Mutex<Vec<Atomic<AlphaTileBatchPrimitive>>>,
    pub fills: Mutex<Vec<Atomic<FillBatchPrimitive>>>,
}

pub enum RenderCommand {
    ClearMaskFramebuffer,
    Fill(Vec<Atomic<FillBatchPrimitive>>),
    AlphaTile(Vec<Atomic<AlphaTileBatchPrimitive>>),
    SolidTile(Vec<SolidTileBatchPrimitive>),
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
    /// If `u16::MAX`, then this is a solid tile.
    pub alpha_tile_index: u16,
    pub backdrop: i16,
}

// FIXME(pcwalton): Move `subpx` before `px` and remove `repr(packed)`.
#[derive(Clone, Copy, Debug)]
#[repr(packed)]
pub struct FillBatchPrimitive {
    pub px: LineSegmentU4,
    pub subpx: LineSegmentU8,
    pub alpha_tile_index: u16,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SolidTileBatchPrimitive {
    pub tile_x: i16,
    pub tile_y: i16,
    pub object_index: u16,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct AlphaTileBatchPrimitive {
    pub tile_x: i16,
    pub tile_y: i16,
    pub backdrop: i16,
    pub object_index: u16,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub object_count: u32,
    pub solid_tile_count: u32,
    pub alpha_tile_count: u32,
    pub fill_count: u32,
}

// Utilities for built objects

impl BuiltObject {
    pub fn new(bounds: RectF32) -> BuiltObject {
        let tile_rect = tiles::round_rect_out_to_tile_bounds(bounds);
        let tiles = DenseTileMap::new(tile_rect);
        BuiltObject { bounds, tiles }
    }

    #[inline]
    pub fn tile_rect(&self) -> RectI32 {
        self.tiles.rect
    }

    #[inline]
    pub fn tile_count(&self) -> u32 {
        self.tiles.data.len() as u32
    }

    fn add_fill(&mut self,
                buffers: &SharedBuffers,
                segment: &LineSegmentF32,
                tile_coords: Point2DI32) {
        //println!("add_fill({:?} ({}, {}))", segment, tile_x, tile_y);
        let local_tile_index = match self.tile_coords_to_local_index(tile_coords) {
            None => return,
            Some(tile_index) => tile_index,
        };

        debug_assert_eq!(TILE_WIDTH, TILE_HEIGHT);
        let tile_size = F32x4::splat(TILE_WIDTH as f32);
        let (min, max) = (F32x4::default(), F32x4::splat((TILE_WIDTH * 256 - 1) as f32));
        let shuffle_mask = I32x4::new(0x0c08_0400, 0x0d05_0901, 0, 0).as_u8x16();

        let tile_upper_left = tile_coords.to_f32().0.xyxy() * tile_size;
            //F32x4::new(tile_x as f32, tile_y as f32, tile_x as f32, tile_y as f32) * tile_size;

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

        // Allocate global tile if necessary.
        let alpha_tile_index = self.get_or_allocate_alpha_tile_index(buffers, tile_coords);

        //println!("... ... OK, pushing");

        buffers.fills.lock().push(Atomic::new(FillBatchPrimitive { px, subpx, alpha_tile_index }));
    }

    fn get_or_allocate_alpha_tile_index(&mut self,
                                        buffers: &SharedBuffers,
                                        tile_coords: Point2DI32)
                                        -> u16 {
        let local_tile_index = self.tiles.coords_to_index_unchecked(tile_coords);
        let alpha_tile_index = self.tiles.data[local_tile_index].alpha_tile_index;
        if alpha_tile_index != !0 {
            return alpha_tile_index;
        }

        let mut alpha_tiles = buffers.alpha_tiles.lock();
        let alpha_tile_index = alpha_tiles.len() as u16;
        self.tiles.data[local_tile_index].alpha_tile_index = alpha_tile_index;
        alpha_tiles.push(Atomic::new(AlphaTileBatchPrimitive::default()));
        alpha_tile_index
    }

    pub fn add_active_fill(&mut self,
                           buffers: &SharedBuffers,
                           left: f32,
                           right: f32,
                           mut winding: i16,
                           tile_coords: Point2DI32) {
        let tile_origin_y = (tile_coords.y() * TILE_HEIGHT as i32) as f32;
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
            self.add_fill(buffers, &segment, tile_coords);
            if winding < 0 {
                winding += 1
            } else {
                winding -= 1
            }
        }
    }

    pub fn generate_fill_primitives_for_line(&mut self,
                                             buffers: &SharedBuffers,
                                             mut segment: LineSegmentF32,
                                             tile_y: i32) {
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
            self.add_fill(buffers, &fill_segment, Point2DI32::new(subsegment_tile_x, tile_y));
        }
    }

    #[inline]
    pub fn tile_coords_to_local_index(&self, coords: Point2DI32) -> Option<u32> {
        self.tiles.coords_to_index(coords).map(|index| index as u32)
    }

    #[inline]
    pub fn local_tile_index_to_coords(&self, tile_index: u32) -> Point2DI32 {
        self.tiles.index_to_coords(tile_index as usize)
    }
}

impl BuiltScene {
    #[inline]
    pub fn new(view_box: RectF32, quad: &[Point3DF32; 4], object_count: u32) -> BuiltScene {
        BuiltScene { view_box, quad: *quad, object_count, shaders: vec![] }
    }

    pub fn stats(&self) -> Stats {
        Stats {
            object_count: self.object_count,
            solid_tile_count: 0,
            alpha_tile_count: 0,
            fill_count: 0,
        }
    }
}

impl Default for TileObjectPrimitive {
    #[inline]
    fn default() -> TileObjectPrimitive {
        TileObjectPrimitive { backdrop: 0, alpha_tile_index: !0 }
    }
}

impl TileObjectPrimitive {
    #[inline]
    pub fn is_solid(&self) -> bool {
        self.alpha_tile_index == !0
    }
}

impl SharedBuffers {
    pub fn new(effective_view_box: RectF32) -> SharedBuffers {
        SharedBuffers {
            z_buffer: ZBuffer::new(effective_view_box),
            fills: Mutex::new(vec![]),
            alpha_tiles: Mutex::new(vec![]),
        }
    }
}

impl Debug for RenderCommand {
    fn fmt(&self, formatter: &mut Formatter) -> DebugResult {
        match *self {
            RenderCommand::ClearMaskFramebuffer => write!(formatter, "ClearMaskFramebuffer"),
            RenderCommand::Fill(ref fills) => write!(formatter, "Fill(x{})", fills.len()),
            RenderCommand::AlphaTile(ref tiles) => {
                write!(formatter, "AlphaTile(x{})", tiles.len())
            }
            RenderCommand::SolidTile(ref tiles) => {
                write!(formatter, "SolidTile(x{})", tiles.len())
            }
        }
    }
}

impl Add<Stats> for Stats {
    type Output = Stats;
    fn add(self, other: Stats) -> Stats {
        Stats {
            object_count:     other.object_count,
            solid_tile_count: other.solid_tile_count,
            alpha_tile_count: other.alpha_tile_count,
            fill_count:       other.fill_count,
        }
    }
}
