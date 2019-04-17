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

use crate::builder::{MAX_FILLS_PER_BATCH, RenderCommandListener};
use crate::cca_vec::ConcurrentCopyableArrayVec;
use crate::scene::ObjectShader;
use crate::tile_map::DenseTileMap;
use crate::tiles::{self, TILE_HEIGHT, TILE_WIDTH};
use crate::z_buffer::ZBuffer;
use pathfinder_geometry::basic::line_segment::{LineSegmentF32, LineSegmentU4, LineSegmentU8};
use pathfinder_geometry::basic::point::{Point2DF32, Point2DI32, Point3DF32};
use pathfinder_geometry::basic::rect::{RectF32, RectI32};
use pathfinder_geometry::util;
use pathfinder_simd::default::{F32x4, I32x4};
use std::fmt::{Debug, Formatter, Result as DebugResult};
use std::ops::Add;

const MAX_FILLS: u32 = 0x100000;
const MAX_ALPHA_TILES: u32 = 0x80000;

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
    pub alpha_tiles: ConcurrentCopyableArrayVec<AlphaTileBatchPrimitive>,
    pub fills: ConcurrentCopyableArrayVec<FillBatchPrimitive>,
}

pub enum RenderCommand {
    ClearMaskFramebuffer,
    Fill(Vec<FillBatchPrimitive>),
    AlphaTile(Vec<AlphaTileBatchPrimitive>),
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
    pub backdrop: i8,
}

// FIXME(pcwalton): Move `subpx` before `px` and remove `repr(packed)`.
#[derive(Clone, Copy, Debug, Default)]
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
    pub tile_x_lo: u8,
    pub tile_y_lo: u8,
    pub tile_hi: u8,
    pub backdrop: i8,
    pub object_index: u16,
    pub tile_index: u16,
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
                listener: &dyn RenderCommandListener,
                segment: &LineSegmentF32,
                tile_coords: Point2DI32) {
        //println!("add_fill({:?} ({}, {}))", segment, tile_x, tile_y);

        // Ensure this fill is in bounds. If not, cull it.
        if self.tile_coords_to_local_index(tile_coords).is_none() {
            return;
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

        let fill_index = buffers.fills.push(FillBatchPrimitive { px, subpx, alpha_tile_index });
        self.flush_render_commands_after_fill(buffers, listener, fill_index);
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

        let alpha_tile_index = buffers.alpha_tiles.push(AlphaTileBatchPrimitive::default()) as u16;
        self.tiles.data[local_tile_index].alpha_tile_index = alpha_tile_index;
        alpha_tile_index
    }

    fn flush_render_commands_after_fill(&mut self,
                                        buffers: &SharedBuffers,
                                        listener: &dyn RenderCommandListener,
                                        fill_index: u32) {
        let fill_count = fill_index + 1;
        if fill_count % MAX_FILLS_PER_BATCH == 0 {
            // Note that this doesn't guarantee that the fills arrive in order (though it's likely
            // that they will), but that doesn't matter because they're order-independent.
            let fill_range = (fill_count - MAX_FILLS_PER_BATCH)..fill_count;
            listener.send(RenderCommand::Fill(buffers.fills.range_to_vec(fill_range)))
        }
    }

    pub fn add_active_fill(&mut self,
                           buffers: &SharedBuffers,
                           listener: &dyn RenderCommandListener,
                           left: f32,
                           right: f32,
                           mut winding: i32,
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
            self.add_fill(buffers, listener, &segment, tile_coords);
            if winding < 0 {
                winding += 1
            } else {
                winding -= 1
            }
        }
    }

    pub fn generate_fill_primitives_for_line(&mut self,
                                             buffers: &SharedBuffers,
                                             listener: &dyn RenderCommandListener,
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
            let fill_tile_coords = Point2DI32::new(subsegment_tile_x, tile_y);
            self.add_fill(buffers, listener, &fill_segment, fill_tile_coords);
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

impl AlphaTileBatchPrimitive {
    #[inline]
    pub fn new(tile_coords: Point2DI32, backdrop: i8, object_index: u16, tile_index: u16)
               -> AlphaTileBatchPrimitive {
        AlphaTileBatchPrimitive {
            tile_x_lo: (tile_coords.x() & 0xff) as u8,
            tile_y_lo: (tile_coords.y() & 0xff) as u8,
            tile_hi: (((tile_coords.x() >> 8) & 0x0f) | ((tile_coords.y() >> 4) & 0xf0)) as u8,
            backdrop,
            object_index,
            tile_index,
        }
    }

    #[inline]
    pub fn tile_coords(&self) -> Point2DI32 {
        Point2DI32::new((self.tile_x_lo as i32) | (((self.tile_hi & 0xf) as i32) << 8),
                        (self.tile_y_lo as i32) | (((self.tile_hi & 0xf0) as i32) << 4))
    }
}

impl SharedBuffers {
    pub fn new(effective_view_box: RectF32) -> SharedBuffers {
        SharedBuffers {
            z_buffer: ZBuffer::new(effective_view_box),
            fills: ConcurrentCopyableArrayVec::new(MAX_FILLS),
            alpha_tiles: ConcurrentCopyableArrayVec::new(MAX_ALPHA_TILES),
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
