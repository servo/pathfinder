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
use otf::cmap::CmapTable;
use otf::glyf::GlyfTable;
use otf::head::HeadTable;
use otf::loca::LocaTable;
use std::mem;
use std::u16;
use util::Jump;

pub mod cmap;
pub mod glyf;
pub mod head;
pub mod loca;

const CMAP: u32 = ((b'c' as u32) << 24) |
                  ((b'm' as u32) << 16) |
                  ((b'a' as u32) << 8)  |
                   (b'p' as u32);
const GLYF: u32 = ((b'g' as u32) << 24) |
                  ((b'l' as u32) << 16) |
                  ((b'y' as u32) << 8)  |
                   (b'f' as u32);
const HEAD: u32 = ((b'h' as u32) << 24) |
                  ((b'e' as u32) << 16) |
                  ((b'a' as u32) << 8)  |
                   (b'd' as u32);
const LOCA: u32 = ((b'l' as u32) << 24) |
                  ((b'o' as u32) << 16) |
                  ((b'c' as u32) << 8)  |
                   (b'a' as u32);

#[derive(Clone, Copy, Debug)]
pub struct FontData<'a> {
    pub bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct FontTable<'a> {
    pub bytes: &'a [u8],
}

impl<'a> FontData<'a> {
    #[inline]
    pub fn new<'b>(bytes: &'b [u8]) -> FontData<'b> {
        FontData {
            bytes: bytes,
        }
    }

    fn table(&self, table_id: u32) -> Result<Option<FontTable>, ()> {
        let mut reader = self.bytes;
        let sfnt_version = try!(reader.read_u32::<BigEndian>().map_err(drop));
        if sfnt_version != 0x10000 {
            return Err(())
        }

        let num_tables = try!(reader.read_u16::<BigEndian>().map_err(drop));
        try!(reader.jump(mem::size_of::<u16>() * 3));

        let (mut low, mut high) = (0, num_tables);
        while low < high {
            let mut reader = reader;
            let mid = (low + high) / 2;
            try!(reader.jump(mid as usize * mem::size_of::<u32>() * 4));

            let current_table_id = try!(reader.read_u32::<BigEndian>().map_err(drop));
            if table_id < current_table_id {
                high = mid;
                continue
            }
            if table_id > current_table_id {
                low = mid + 1;
                continue
            }

            // Skip the checksum, and slurp the offset and length.
            try!(reader.read_u32::<BigEndian>().map_err(drop));
            let offset = try!(reader.read_u32::<BigEndian>().map_err(drop)) as usize;
            let length = try!(reader.read_u32::<BigEndian>().map_err(drop)) as usize;

            let end = offset + length;
            if end > self.bytes.len() {
                return Err(())
            }
            return Ok(Some(FontTable {
                bytes: &self.bytes[offset..end],
            }))
        }

        Ok(None)
    }

    #[inline]
    pub fn cmap_table(&self) -> Result<CmapTable, ()> {
        self.table(CMAP).and_then(|table| table.ok_or(()).map(CmapTable::new))
    }

    #[inline]
    pub fn glyf_table(&self) -> Result<GlyfTable, ()> {
        self.table(GLYF).and_then(|table| table.ok_or(()).map(GlyfTable::new))
    }

    #[inline]
    pub fn head_table(&self) -> Result<HeadTable, ()> {
        self.table(HEAD).and_then(|table| table.ok_or(()).and_then(HeadTable::new))
    }

    #[inline]
    pub fn loca_table(&self, head_table: &HeadTable) -> Result<LocaTable, ()> {
        let loca_table = try!(self.table(LOCA).and_then(|table| table.ok_or(())));
        LocaTable::new(loca_table, head_table)
    }
}

