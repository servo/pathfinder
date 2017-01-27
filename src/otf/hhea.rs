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

#[derive(Clone, Debug)]
pub struct HheaTable {
    pub number_of_h_metrics: u16,
}

impl HheaTable {
    pub fn new(table: FontTable) -> Result<HheaTable, ()> {
        let mut reader = table.bytes;

        // Check the version.
        let major_version = try!(reader.read_u16::<BigEndian>().map_err(drop));
        let minor_version = try!(reader.read_u16::<BigEndian>().map_err(drop));
        if (major_version, minor_version) != (1, 0) {
            return Err(())
        }

        // Read the number of `hmtx` entries.
        try!(reader.jump(mem::size_of::<u16>() * 15));
        let number_of_h_metrics = try!(reader.read_u16::<BigEndian>().map_err(drop));

        Ok(HheaTable {
            number_of_h_metrics: number_of_h_metrics,
        })
    }
}

