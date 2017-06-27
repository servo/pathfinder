// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use byteorder::{BigEndian, ReadBytesExt};
use error::FontError;
use font::FontTable;
use std::mem;
use tables::hhea::HheaTable;
use util::Jump;

pub const TAG: u32 = ((b'h' as u32) << 24) |
                      ((b'm' as u32) << 16) |
                      ((b't' as u32) << 8)  |
                       (b'x' as u32);

#[derive(Clone, Copy)]
pub struct HmtxTable<'a> {
    table: FontTable<'a>,
}

impl<'a> HmtxTable<'a> {
    pub fn new(table: FontTable) -> HmtxTable {
        HmtxTable {
            table: table,
        }
    }

    pub fn metrics_for_glyph(&self, hhea_table: &HheaTable, glyph_id: u16)
                             -> Result<HorizontalMetrics, FontError> {
        let mut reader = self.table.bytes;

        // Read the advance width.
        let advance_width;
        if glyph_id < hhea_table.number_of_h_metrics {
            try!(reader.jump(mem::size_of::<u16>() * 2 * glyph_id as usize).map_err(FontError::eof));
            advance_width = try!(reader.read_u16::<BigEndian>().map_err(FontError::eof))
        } else {
            try!(reader.jump(mem::size_of::<u16>() * 2 *
                             (hhea_table.number_of_h_metrics - 1) as usize).map_err(FontError::eof));
            advance_width = try!(reader.read_u16::<BigEndian>().map_err(FontError::eof));
            try!(reader.jump(mem::size_of::<i16>() * glyph_id as usize).map_err(FontError::eof));
        }

        // Read the left-side bearing.
        let lsb = try!(reader.read_i16::<BigEndian>().map_err(FontError::eof));

        Ok(HorizontalMetrics {
            advance_width: advance_width,
            lsb: lsb,
        })
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct HorizontalMetrics {
    pub advance_width: u16,
    pub lsb: i16,
}

