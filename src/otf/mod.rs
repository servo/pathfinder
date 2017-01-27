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
use otf::hhea::HheaTable;
use otf::hmtx::HmtxTable;
use otf::loca::LocaTable;
use std::mem;
use std::u16;
use util::Jump;

pub mod cmap;
pub mod glyf;
pub mod head;
pub mod hhea;
pub mod hmtx;
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
const HHEA: u32 = ((b'h' as u32) << 24) |
                  ((b'h' as u32) << 16) |
                  ((b'e' as u32) << 8)  |
                   (b'a' as u32);
const HMTX: u32 = ((b'h' as u32) << 24) |
                  ((b'm' as u32) << 16) |
                  ((b't' as u32) << 8)  |
                   (b'x' as u32);
const LOCA: u32 = ((b'l' as u32) << 24) |
                  ((b'o' as u32) << 16) |
                  ((b'c' as u32) << 8)  |
                   (b'a' as u32);

pub struct Font<'a> {
    pub bytes: &'a [u8],

    pub cmap: CmapTable<'a>,
    pub head: HeadTable,
    pub hhea: HheaTable,
    pub hmtx: HmtxTable<'a>,

    pub glyf: Option<GlyfTable<'a>>,
    pub loca: Option<LocaTable<'a>>,
}

#[derive(Clone, Copy, Debug)]
pub struct FontTable<'a> {
    pub bytes: &'a [u8],
}

impl<'a> Font<'a> {
    #[inline]
    pub fn new<'b>(bytes: &'b [u8]) -> Result<Font<'b>, ()> {
        // Read the tables we care about.
        let mut reader = bytes;
        let sfnt_version = try!(reader.read_u32::<BigEndian>().map_err(drop));
        if sfnt_version != 0x10000 {
            return Err(())
        }

        let num_tables = try!(reader.read_u16::<BigEndian>().map_err(drop));
        try!(reader.jump(mem::size_of::<u16>() * 3));

        let (mut cmap_table, mut head_table) = (None, None);
        let (mut hhea_table, mut hmtx_table) = (None, None);
        let (mut glyf_table, mut loca_table) = (None, None);

        for _ in 0..num_tables {
            let table_id = try!(reader.read_u32::<BigEndian>().map_err(drop));

            // Skip over the checksum.
            try!(reader.read_u32::<BigEndian>().map_err(drop));

            let offset = try!(reader.read_u32::<BigEndian>().map_err(drop)) as usize;
            let length = try!(reader.read_u32::<BigEndian>().map_err(drop)) as usize;

            let mut slot = match table_id {
                CMAP => &mut cmap_table,
                HEAD => &mut head_table,
                HHEA => &mut hhea_table,
                HMTX => &mut hmtx_table,
                GLYF => &mut glyf_table,
                LOCA => &mut loca_table,
                _ => continue,
            };

            // Make sure there isn't more than one copy of the table.
            if slot.is_some() {
                return Err(())
            }

            *slot = Some(FontTable {
                bytes: &bytes[offset..offset + length],
            })
        }

        let loca_table = match loca_table {
            None => None,
            Some(loca_table) => Some(try!(LocaTable::new(loca_table))),
        };

        Ok(Font {
            bytes: bytes,

            cmap: CmapTable::new(try!(cmap_table.ok_or(()))),
            head: try!(HeadTable::new(try!(head_table.ok_or(())))),
            hhea: try!(HheaTable::new(try!(hhea_table.ok_or(())))),
            hmtx: HmtxTable::new(try!(hmtx_table.ok_or(()))),

            glyf: glyf_table.map(GlyfTable::new),
            loca: loca_table,
        })
    }
}

