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
use otf::FontTable;
use std::mem;
use util::Jump;

const MAGIC_NUMBER: u32 = 0x5f0f3cf5;

#[derive(Clone, Debug)]
pub struct HeadTable {
    pub units_per_em: u16,
    pub index_to_loc_format: i16,
}

impl HeadTable {
    pub fn new(table: FontTable) -> Result<HeadTable, ()> {
        let mut reader = table.bytes;

        // Check the version.
        let major_version = try!(reader.read_u16::<BigEndian>().map_err(drop));
        let minor_version = try!(reader.read_u16::<BigEndian>().map_err(drop));
        if (major_version, minor_version) != (1, 0) {
            return Err(())
        }

        // Check the magic number.
        try!(reader.jump(mem::size_of::<u32>() * 2));
        let magic_number = try!(reader.read_u32::<BigEndian>().map_err(drop));
        if magic_number != MAGIC_NUMBER {
            return Err(())
        }

        // Read the units per em.
        try!(reader.jump(mem::size_of::<u16>()));
        let units_per_em = try!(reader.read_u16::<BigEndian>().map_err(drop));

        // Read the index-to-location format.
        try!(reader.jump(mem::size_of::<i64>() * 2 +
                         mem::size_of::<i16>() * 4 + 
                         mem::size_of::<u16>() * 2 +
                         mem::size_of::<i16>()));
        let index_to_loc_format = try!(reader.read_i16::<BigEndian>().map_err(drop));

        // Check the glyph data format.
        let glyph_data_format = try!(reader.read_i16::<BigEndian>().map_err(drop));
        if glyph_data_format != 0 {
            return Err(())
        }

        Ok(HeadTable {
            units_per_em: units_per_em,
            index_to_loc_format: index_to_loc_format,
        })
    }
}
