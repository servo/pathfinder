// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use atlas::Atlas;
use compute_shader::buffer::{Buffer, BufferData, HostAllocatedData, Protection};
use compute_shader::device::Device;
use glyph_buffer::GlyphBufferBuilder;
use std::u16;

const POINTS_PER_SEGMENT: u32 = 32;

pub struct BatchBuilder {
    pub atlas: Atlas,
    pub indices: Vec<u16>,
    pub images: Vec<ImageDescriptor>,
    pub point_count: u32,
}

impl BatchBuilder {
    /// FIXME(pcwalton): Including the shelf height here may be a bad API.
    #[inline]
    pub fn new(available_width: u32, shelf_height: u32) -> BatchBuilder {
        BatchBuilder {
            atlas: Atlas::new(available_width, shelf_height),
            indices: vec![],
            images: vec![],
            point_count: 0,
        }
    }

    pub fn add_glyph(&mut self,
                     glyph_buffer_builder: &GlyphBufferBuilder,
                     glyph_index: u32,
                     point_size: f32)
                     -> Result<(), ()> {
        let descriptor = &glyph_buffer_builder.descriptors[glyph_index as usize];

        // FIXME(pcwalton): I think this will check for negative values and panic, which is
        // unnecessary.
        let pixel_size = descriptor.pixel_rect(point_size).size.ceil().cast().unwrap();
        let atlas_origin = try!(self.atlas.place(&pixel_size));

        if self.point_count % POINTS_PER_SEGMENT == 0 {
            self.indices.push(self.images.len() as u16)
        }

        self.images.push(ImageDescriptor {
            atlas_x: atlas_origin.x,
            atlas_y: atlas_origin.y,
            point_size: point_size,
            glyph_index: glyph_index,
            start_point_in_glyph: descriptor.start_point,
            start_point_in_batch: self.point_count,
            point_count: descriptor.point_count as u32,
        });

        self.point_count += descriptor.point_count as u32;

        Ok(())
    }

    pub fn finish(&mut self, device: &mut Device) -> Result<Batch, ()> {
        let indices = BufferData::HostAllocated(HostAllocatedData::new(&self.indices));
        let images = BufferData::HostAllocated(HostAllocatedData::new(&self.images));
        Ok(Batch {
            indices: try!(device.create_buffer(Protection::ReadOnly, indices).map_err(drop)),
            images: try!(device.create_buffer(Protection::ReadOnly, images).map_err(drop)),
        })
    }
}

pub struct Batch {
    pub indices: Buffer,
    pub images: Buffer,
}

#[derive(Clone, Copy, Debug)]
pub struct GlyphRange {
    pub start: u16,
    pub end: u16,
}

impl GlyphRange {
    #[inline]
    pub fn iter(&self) -> GlyphRangeIter {
        GlyphRangeIter {
            start: self.start,
            end: self.end,
        }
    }
}

#[derive(Clone)]
pub struct GlyphRangeIter {
    start: u16,
    end: u16,
}

impl Iterator for GlyphRangeIter {
    type Item = u16;

    #[inline]
    fn next(&mut self) -> Option<u16> {
        if self.start > self.end {
            None
        } else {
            let item = self.start;
            self.start += 1;
            Some(item)
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ImageDescriptor {
    atlas_x: u32,
    atlas_y: u32,
    point_size: f32,
    glyph_index: u32,
    start_point_in_glyph: u32,
    start_point_in_batch: u32,
    point_count: u32,
}

