// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! OpenType `.otf` files.
//!
//! See Microsoft's spec: https://www.microsoft.com/typography/otspec/otff.htm

use byteorder::{BigEndian, ReadBytesExt};
use error::FontError;
use font::{Font, FontTable};
use std::mem;
use tables::cff::{self, CffTable};
use tables::cmap::{self, CmapTable};
use tables::glyf::{self, GlyfTable};
use tables::head::{self, HeadTable};
use tables::hhea::{self, HheaTable};
use tables::hmtx::{self, HmtxTable};
use tables::kern::{self, KernTable};
use tables::loca::{self, LocaTable};
use tables::os_2::{self, Os2Table};
use util::Jump;

const OTTO: u32 = ((b'O' as u32) << 24) |
                  ((b'T' as u32) << 16) |
                  ((b'T' as u32) << 8)  |
                   (b'O' as u32);

pub static SFNT_VERSIONS: [u32; 3] = [
    0x10000,
    ((b't' as u32) << 24) | ((b'r' as u32) << 16) | ((b'u' as u32) << 8) | (b'e' as u32),
    OTTO,
];

#[doc(hidden)]
pub struct FontTables<'a> {
    pub cmap: CmapTable<'a>,
    pub head: HeadTable,
    pub hhea: HheaTable,
    pub hmtx: HmtxTable<'a>,
    pub os_2: Os2Table,

    pub cff: Option<CffTable<'a>>,
    pub glyf: Option<GlyfTable<'a>>,
    pub loca: Option<LocaTable<'a>>,
    pub kern: Option<KernTable<'a>>,
}

impl<'a> Font<'a> {
    pub fn from_otf<'b>(bytes: &'b [u8], offset: u32) -> Result<Font<'b>, FontError> {
        let mut reader = bytes;
        try!(reader.jump(offset as usize).map_err(FontError::eof));

        // Check the magic number.
        if !SFNT_VERSIONS.contains(&try!(reader.read_u32::<BigEndian>().map_err(FontError::eof))) {
            return Err(FontError::UnknownFormat)
        }

        let num_tables = try!(reader.read_u16::<BigEndian>().map_err(FontError::eof));
        try!(reader.jump(mem::size_of::<u16>() * 3).map_err(FontError::eof));

        let (mut cff_table, mut cmap_table) = (None, None);
        let (mut glyf_table, mut head_table) = (None, None);
        let (mut hhea_table, mut hmtx_table) = (None, None);
        let (mut kern_table, mut loca_table) = (None, None);
        let mut os_2_table = None;

        for _ in 0..num_tables {
            let table_id = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));

            // Skip over the checksum.
            try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));

            let offset = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof)) as usize;
            let length = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof)) as usize;

            let mut slot = match table_id {
                cff::TAG => &mut cff_table,
                cmap::TAG => &mut cmap_table,
                glyf::TAG => &mut glyf_table,
                head::TAG => &mut head_table,
                hhea::TAG => &mut hhea_table,
                hmtx::TAG => &mut hmtx_table,
                kern::TAG => &mut kern_table,
                loca::TAG => &mut loca_table,
                os_2::TAG => &mut os_2_table,
                _ => continue,
            };

            // Make sure there isn't more than one copy of the table.
            if slot.is_some() {
                return Err(FontError::Failed)
            }

            *slot = Some(FontTable {
                bytes: &bytes[offset..offset + length],
            })
        }

        let cff_table = match cff_table {
            None => None,
            Some(cff_table) => Some(try!(CffTable::new(cff_table))),
        };

        let loca_table = match loca_table {
            None => None,
            Some(loca_table) => Some(try!(LocaTable::new(loca_table))),
        };

        let tables = FontTables {
            cmap: CmapTable::new(try!(cmap_table.ok_or(FontError::RequiredTableMissing))),
            head: try!(HeadTable::new(try!(head_table.ok_or(FontError::RequiredTableMissing)))),
            hhea: try!(HheaTable::new(try!(hhea_table.ok_or(FontError::RequiredTableMissing)))),
            hmtx: HmtxTable::new(try!(hmtx_table.ok_or(FontError::RequiredTableMissing))),
            os_2: try!(Os2Table::new(try!(os_2_table.ok_or(FontError::RequiredTableMissing)))),

            cff: cff_table,
            glyf: glyf_table.map(GlyfTable::new),
            loca: loca_table,
            kern: kern_table.and_then(|table| KernTable::new(table).ok()),
        };

        Ok(Font::from_tables(bytes, tables))
    }
}

