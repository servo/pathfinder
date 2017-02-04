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
use otf::head::HeadTable;
use otf::{Error, FontTable};
use util::Jump;

pub struct LocaTable<'a> {
    table: FontTable<'a>,
}

impl<'a> LocaTable<'a> {
    pub fn new(loca_table: FontTable<'a>) -> Result<LocaTable<'a>, Error> {
        Ok(LocaTable {
            table: loca_table,
        })
    }

    pub fn location_of(&self, head_table: &HeadTable, glyph_id: u16)
                       -> Result<Option<u32>, Error> {
        let mut reader = self.table.bytes;
        let (this_location, next_location) = match head_table.index_to_loc_format {
            0 => {
                try!(reader.jump(glyph_id as usize * 2).map_err(Error::eof));
                let this_location =
                    try!(reader.read_u16::<BigEndian>().map_err(Error::eof)) as u32 * 2;
                let next_location = match reader.read_u16::<BigEndian>() {
                    Ok(next_location) => Ok(next_location as u32 * 2),
                    Err(_) => Err(Error::UnexpectedEof),
                };
                (this_location, next_location)
            }
            1 => {
                try!(reader.jump(glyph_id as usize * 4).map_err(Error::eof));
                let this_location = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));
                let next_location = reader.read_u32::<BigEndian>().map_err(Error::eof);
                (this_location, next_location)
            }
            _ => return Err(Error::UnknownFormat),
        };

        if next_location == Ok(this_location) {
            Ok(None)
        } else {
            Ok(Some(this_location))
        }
    }
}

