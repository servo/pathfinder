// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use compute_shader::buffer::Buffer;
use otf::glyf::GlyfTable;
use otf::loca::LocaTable;

pub struct GlyphBuffers {
    pub coordinates: Vec<(i16, i16)>,
    pub operations: Vec<u8>,
    pub descriptors: Vec<GlyphDescriptor>,
    pub cached_coordinates_buffer: Option<Buffer>,
    pub cached_operations_buffer: Option<Buffer>,
    pub cached_descriptors_buffer: Option<Buffer>,
}

impl GlyphBuffers {
    #[inline]
    pub fn new() -> GlyphBuffers {
        GlyphBuffers {
            coordinates: vec![],
            operations: vec![],
            descriptors: vec![],
            cached_coordinates_buffer: None,
            cached_operations_buffer: None,
            cached_descriptors_buffer: None,
        }
    }

    pub fn add_glyph(&mut self, glyph_id: u32, loca_table: &LocaTable, glyf_table: &GlyfTable)
                     -> Result<(), ()> {
        self.cached_coordinates_buffer = None;
        self.cached_operations_buffer = None;
        self.cached_descriptors_buffer = None;

        let mut point_index = self.coordinates.len() / 2;
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

        Ok(())
    }
}

#[repr(C)]
pub struct GlyphDescriptor {
    pub left: i16,
    pub bottom: i16,
    pub width: i16,
    pub height: i16,
    pub units_per_em: u16,
    pub point_count: u16,
    pub index_of_first_point: u32,
}

