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
use otf::head::HeadTable;
use util::Jump;

pub struct LocaTable<'a> {
    table: FontTable<'a>,
}

impl<'a> LocaTable<'a> {
    pub fn new(loca_table: FontTable<'a>) -> Result<LocaTable<'a>, ()> {
        Ok(LocaTable {
            table: loca_table,
        })
    }

    pub fn location_of(&self, head_table: &HeadTable, glyph_id: u16) -> Result<u32, ()> {
        let mut reader = self.table.bytes;
        match head_table.index_to_loc_format {
            0 => {
                try!(reader.jump(glyph_id as usize * 2));
                Ok(try!(reader.read_u16::<BigEndian>().map_err(drop)) as u32 * 2)
            }
            1 => {
                try!(reader.jump(glyph_id as usize * 4));
                reader.read_u32::<BigEndian>().map_err(drop)
            }
            _ => Err(()),
        }
    }
}

