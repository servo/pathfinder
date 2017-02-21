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
use otf::{Error, FontTable};
use std::mem;
use util::Jump;

bitflags! {
    flags Coverage: u16 {
        const HORIZONTAL = 1 << 0,
        const MINIMUM = 1 << 1,
        const CROSS_STREAM = 1 << 2,
        const OVERRIDE = 1 << 3,
    }
}

#[derive(Clone, Copy)]
pub struct KernTable<'a> {
    horizontal_table: &'a [u8],
}

impl<'a> KernTable<'a> {
    pub fn new(table: FontTable) -> Result<KernTable, Error> {
        let mut kern_reader = table.bytes;
        let version = try!(kern_reader.read_u16::<BigEndian>().map_err(Error::eof));
        if version != 0 {
            return Err(Error::UnknownFormat)
        }

        let n_tables = try!(kern_reader.read_u16::<BigEndian>().map_err(Error::eof));
        let mut horizontal_table = None;
        for _ in 0..n_tables {
            let mut table_reader = kern_reader;
            let _version = try!(table_reader.read_u16::<BigEndian>().map_err(Error::eof));
            let length = try!(table_reader.read_u16::<BigEndian>().map_err(Error::eof));
            let coverage = try!(table_reader.read_u16::<BigEndian>().map_err(Error::eof));
            let coverage_flags = Coverage::from_bits_truncate(coverage);

            if coverage_flags.contains(HORIZONTAL) && !coverage_flags.contains(MINIMUM) &&
                    !coverage_flags.contains(CROSS_STREAM) && (coverage >> 8) == 0 {
                let length = length as usize - mem::size_of::<u16>() * 3;
                horizontal_table = Some(&table_reader[0..length]);
                break
            }

            try!(kern_reader.jump(length as usize).map_err(Error::eof));
        }

        match horizontal_table {
            Some(horizontal_table) => {
                Ok(KernTable {
                    horizontal_table: horizontal_table,
                })
            }
            None => Err(Error::UnknownFormat),
        }
    }

    pub fn kerning_for_glyph_pair(&self, left_glyph_id: u16, right_glyph_id: u16)
                                  -> Result<i16, Error> {
        let mut table_reader = self.horizontal_table;
        let n_pairs = try!(table_reader.read_u16::<BigEndian>().map_err(Error::eof));
        try!(table_reader.jump(mem::size_of::<[u16; 3]>()).map_err(Error::eof));

        let (mut low, mut high) = (0, n_pairs as u32);
        while low < high {
            let mut reader = table_reader;
            let mid = (low + high) / 2;

            try!(reader.jump(mid as usize * mem::size_of::<[u16; 3]>()).map_err(Error::eof));
            let left = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
            let right = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
            let value = try!(reader.read_i16::<BigEndian>().map_err(Error::eof));

            if left_glyph_id < left || (left_glyph_id == left && right_glyph_id < right) {
                high = mid
            } else if left_glyph_id > left || (left_glyph_id == left && right_glyph_id > right) {
                low = mid + 1
            } else {
                return Ok(value)
            }
        }

        Ok(0)
    }
}

