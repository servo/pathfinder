// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use compute_shader::buffer::{Buffer, BufferData, HostAllocatedData, Protection};
use compute_shader::device::Device;
use euclid::{Point2D, Rect, Size2D};
use otf::glyf::GlyfTable;
use otf::head::HeadTable;
use otf::loca::LocaTable;

pub struct GlyphBufferBuilder {
    pub coordinates: Vec<(i16, i16)>,
    pub operations: Vec<u8>,
    pub descriptors: Vec<GlyphDescriptor>,
}

impl GlyphBufferBuilder {
    #[inline]
    pub fn new() -> GlyphBufferBuilder {
        GlyphBufferBuilder {
            coordinates: vec![],
            operations: vec![],
            descriptors: vec![],
        }
    }

    pub fn add_glyph(&mut self,
                     glyph_id: u32,
                     head_table: &HeadTable,
                     loca_table: &LocaTable,
                     glyf_table: &GlyfTable)
                     -> Result<(), ()> {
        let mut point_index = self.coordinates.len() / 2;
        let start_point = point_index;
        let mut operations = if point_index % 4 == 0 {
            0
        } else {
            self.operations.pop().unwrap()
        };

        try!(glyf_table.for_each_point(loca_table, glyph_id, |point| {
            self.coordinates.push((point.position.x, point.position.y));

            let operation = if point.first_point_in_contour {
                0
            } else if point.on_curve {
                1
            } else {
                2
            };

            operations |= operation << (point_index % 4 * 2);

            point_index += 1;
            if point_index % 4 == 0 {
                self.operations.push(operation)
            }
        }));

        if point_index % 4 != 0 {
            self.operations.push(operations)
        }

        // TODO(pcwalton): Add a glyph descriptor.
        let bounding_rect = try!(glyf_table.bounding_rect(loca_table, glyph_id));
        self.descriptors.push(GlyphDescriptor {
            left: bounding_rect.origin.x,
            bottom: bounding_rect.origin.y,
            width: bounding_rect.size.width,
            height: bounding_rect.size.height,
            units_per_em: head_table.units_per_em,
            point_count: (point_index - start_point) as u16,
            start_point: start_point as u32,
        });

        Ok(())
    }

    pub fn finish(&self, device: &Device) -> Result<GlyphBuffers, ()> {
        let coordinates = BufferData::HostAllocated(HostAllocatedData::new(&self.coordinates));
        let operations = BufferData::HostAllocated(HostAllocatedData::new(&self.operations));
        let descriptors = BufferData::HostAllocated(HostAllocatedData::new(&self.descriptors));
        Ok(GlyphBuffers {
            coordinates: try!(device.create_buffer(Protection::ReadOnly, coordinates)
                                    .map_err(drop)),
            operations: try!(device.create_buffer(Protection::ReadOnly, operations).map_err(drop)),
            descriptors: try!(device.create_buffer(Protection::ReadOnly, descriptors)
                                    .map_err(drop)),
        })
    }
}

pub struct GlyphBuffers {
    pub coordinates: Buffer,
    pub operations: Buffer,
    pub descriptors: Buffer,
}

#[repr(C)]
pub struct GlyphDescriptor {
    pub left: i16,
    pub bottom: i16,
    pub width: i16,
    pub height: i16,
    pub units_per_em: u16,
    pub point_count: u16,
    pub start_point: u32,
}

impl GlyphDescriptor {
    #[inline]
    pub fn pixel_rect(&self, point_size: f32) -> Rect<f32> {
        let pixels_per_unit = point_size / self.units_per_em as f32;
        Rect::new(Point2D::new(self.left as f32, self.bottom as f32),
                  Size2D::new(self.width as f32, self.height as f32)) * pixels_per_unit
    }
}

