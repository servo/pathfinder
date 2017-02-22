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

pub const KNOWN_TABLE_COUNT: usize = 9;

pub static KNOWN_TABLES: [u32; KNOWN_TABLE_COUNT] = [
    cff::TAG,
    os_2::TAG,
    cmap::TAG,
    glyf::TAG,
    head::TAG,
    hhea::TAG,
    hmtx::TAG,
    kern::TAG,
    loca::TAG,
];

// This must agree with the above.
const TABLE_INDEX_CFF:  usize = 0;
const TABLE_INDEX_OS_2: usize = 1;
const TABLE_INDEX_CMAP: usize = 2;
const TABLE_INDEX_GLYF: usize = 3;
const TABLE_INDEX_HEAD: usize = 4;
const TABLE_INDEX_HHEA: usize = 5;
const TABLE_INDEX_HMTX: usize = 6;
const TABLE_INDEX_KERN: usize = 7;
const TABLE_INDEX_LOCA: usize = 8;

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

        let mut tables = [None; KNOWN_TABLE_COUNT];
        for _ in 0..num_tables {
            let table_id = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
            let _checksum = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
            let offset = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof)) as usize;
            let length = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof)) as usize;

            // Find the table ID in our list of known IDs, which must be sorted.
            debug_assert!(KNOWN_TABLES.windows(2).all(|w| w[0] < w[1]));
            let slot = match KNOWN_TABLES.binary_search(&table_id) {
                Err(_) => continue,
                Ok(table_index) => &mut tables[table_index],
            };

            // Make sure there isn't more than one copy of the table.
            if slot.is_some() {
                return Err(FontError::Failed)
            }

            *slot = Some(FontTable {
                bytes: &bytes[offset..offset + length],
            })
        }

        Font::from_table_list(bytes, &tables)
    }

    #[doc(hidden)]
    pub fn from_table_list<'b>(bytes: &'b [u8],
                               tables: &[Option<FontTable<'b>>; KNOWN_TABLE_COUNT])
                               -> Result<Font<'b>, FontError> {
        let cff_table = match tables[TABLE_INDEX_CFF] {
            None => None,
            Some(cff_table) => Some(try!(CffTable::new(cff_table))),
        };

        let loca_table = match tables[TABLE_INDEX_LOCA] {
            None => None,
            Some(loca_table) => Some(try!(LocaTable::new(loca_table))),
        };

        // For brevity below…
        let missing = FontError::RequiredTableMissing;

        let tables = FontTables {
            cmap: CmapTable::new(try!(tables[TABLE_INDEX_CMAP].ok_or(missing))),
            head: try!(HeadTable::new(try!(tables[TABLE_INDEX_HEAD].ok_or(missing)))),
            hhea: try!(HheaTable::new(try!(tables[TABLE_INDEX_HHEA].ok_or(missing)))),
            hmtx: HmtxTable::new(try!(tables[TABLE_INDEX_HMTX].ok_or(missing))),
            os_2: try!(Os2Table::new(try!(tables[TABLE_INDEX_OS_2].ok_or(missing)))),

            cff: cff_table,
            glyf: tables[TABLE_INDEX_GLYF].map(GlyfTable::new),
            loca: loca_table,
            kern: tables[TABLE_INDEX_KERN].and_then(|table| KernTable::new(table).ok()),
        };

        Ok(Font::from_tables(bytes, tables))
    }
}

