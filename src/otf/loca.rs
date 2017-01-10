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
    pub long: bool,
}

impl<'a> LocaTable<'a> {
    pub fn new(loca_table: FontTable<'a>, head_table: &HeadTable) -> Result<LocaTable<'a>, ()> {
        let long = match head_table.index_to_loc_format {
            0 => false,
            1 => true,
            _ => return Err(()),
        };

        Ok(LocaTable {
            table: loca_table,
            long: long,
        })
    }

    pub fn location_of(&self, glyph_id: u32) -> Result<u32, ()> {
        let mut reader = self.table.bytes;
        if !self.long {
            try!(reader.jump(glyph_id as usize * 2));
            Ok(try!(reader.read_u16::<BigEndian>().map_err(drop)) as u32 * 2)
        } else {
            try!(reader.jump(glyph_id as usize * 4));
            reader.read_u32::<BigEndian>().map_err(drop)
        }
    }
}

