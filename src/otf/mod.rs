// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! OpenType fonts.

use byteorder::{BigEndian, ReadBytesExt};
use charmap::CodepointRange;
use glyph_range::GlyphRanges;
use otf::cmap::CmapTable;
use otf::glyf::{GlyfTable, Point};
use otf::head::HeadTable;
use otf::hhea::HheaTable;
use otf::hmtx::{HmtxTable, HorizontalMetrics};
use otf::loca::LocaTable;
use outline::GlyphBoundsI;
use std::mem;
use std::u16;
use util::Jump;

mod cmap;
mod glyf;
mod head;
mod hhea;
mod hmtx;
mod loca;

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
const TTCF: u32 = ((b't' as u32) << 24) |
                  ((b't' as u32) << 16) |
                  ((b'c' as u32) << 8)  |
                   (b'f' as u32);

const OTTO: u32 = ((b'O' as u32) << 24) |
                  ((b'T' as u32) << 16) |
                  ((b'T' as u32) << 8)  |
                   (b'O' as u32);
const SFNT: u32 = ((b's' as u32) << 24) |
                  ((b'f' as u32) << 16) |
                  ((b'n' as u32) << 8)  |
                   (b't' as u32);

static SFNT_VERSIONS: [u32; 2] = [
    0x10000,
    ((b't' as u32) << 24) | ((b'r' as u32) << 16) | ((b'u' as u32) << 8) | (b'e' as u32),
];

/// A handle to a font backed by a byte buffer containing the contents of the file (`.ttf`,
/// `.otf`), etc.
///
/// For optimum performance, consider using the `memmap` crate to provide the byte buffer.
pub struct Font<'a> {
    pub bytes: &'a [u8],

    cmap: CmapTable<'a>,
    head: HeadTable,
    hhea: HheaTable,
    hmtx: HmtxTable<'a>,

    glyf: Option<GlyfTable<'a>>,
    loca: Option<LocaTable<'a>>,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct FontTable<'a> {
    pub bytes: &'a [u8],
}

impl<'a> Font<'a> {
    /// Creates a new font from a byte buffer containing the contents of a file (`.ttf`, `.otf`,
    /// etc.)
    ///
    /// Returns the font on success or an error on failure.
    pub fn new<'b>(bytes: &'b [u8]) -> Result<Font<'b>, Error> {
        // Check magic number.
        let mut reader = bytes;
        let mut magic_number = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));
        match magic_number {
            TTCF => {
                // This is a font collection. Read the first font.
                //
                // TODO(pcwalton): Provide a mechanism to read others.
                let major_version = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
                let minor_version = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
                if (major_version != 1 && major_version != 2) || minor_version != 0 {
                    return Err(Error::UnsupportedVersion)
                }

                let num_fonts = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));
                if num_fonts == 0 {
                    return Err(Error::Failed)
                }

                let table_offset = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));
                Font::from_otf(&bytes, table_offset)
            }
            magic_number if SFNT_VERSIONS.contains(&magic_number) => Font::from_otf(bytes, 0),
            0x0100 => Font::from_dfont(bytes),
            OTTO => {
                // TODO(pcwalton): Support CFF outlines.
                Err(Error::UnsupportedCffOutlines)
            }
            _ => Err(Error::UnknownFormat),
        }
    }

    fn from_otf<'b>(bytes: &'b [u8], offset: u32) -> Result<Font<'b>, Error> {
        let mut reader = bytes;
        try!(reader.jump(offset as usize).map_err(Error::eof));

        let mut magic_number = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));

        // Check version.
        if magic_number == OTTO {
            // TODO(pcwalton): Support CFF outlines.
            return Err(Error::UnsupportedCffOutlines)
        } else if !SFNT_VERSIONS.contains(&magic_number) {
            return Err(Error::UnknownFormat)
        }

        let num_tables = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
        try!(reader.jump(mem::size_of::<u16>() * 3).map_err(Error::eof));

        let (mut cmap_table, mut head_table) = (None, None);
        let (mut hhea_table, mut hmtx_table) = (None, None);
        let (mut glyf_table, mut loca_table) = (None, None);

        for _ in 0..num_tables {
            let table_id = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));

            // Skip over the checksum.
            try!(reader.read_u32::<BigEndian>().map_err(Error::eof));

            let offset = try!(reader.read_u32::<BigEndian>().map_err(Error::eof)) as usize;
            let length = try!(reader.read_u32::<BigEndian>().map_err(Error::eof)) as usize;

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
                return Err(Error::Failed)
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

            cmap: CmapTable::new(try!(cmap_table.ok_or(Error::RequiredTableMissing))),
            head: try!(HeadTable::new(try!(head_table.ok_or(Error::RequiredTableMissing)))),
            hhea: try!(HheaTable::new(try!(hhea_table.ok_or(Error::RequiredTableMissing)))),
            hmtx: HmtxTable::new(try!(hmtx_table.ok_or(Error::RequiredTableMissing))),

            glyf: glyf_table.map(GlyfTable::new),
            loca: loca_table,
        })
    }

    /// https://github.com/kreativekorp/ksfl/wiki/Macintosh-Resource-File-Format
    fn from_dfont<'b>(bytes: &'b [u8]) -> Result<Font<'b>, Error> {
        let mut reader = bytes;

        // Read the Mac resource file header.
        let resource_data_offset = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));
        let resource_map_offset = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));
        let resource_data_size = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));
        let resource_map_size = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));

        // Move to the fields we care about in the resource map.
        reader = bytes;
        try!(reader.jump(resource_map_offset as usize + mem::size_of::<u32>() * 5 +
                         mem::size_of::<u16>() * 2).map_err(Error::eof));

        // Read the type list and name list offsets.
        let type_list_offset = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
        let name_list_offset = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));

        // Move to the type list.
        reader = bytes;
        try!(reader.jump(resource_map_offset as usize + type_list_offset as usize)
                   .map_err(Error::eof));

        // Find the 'sfnt' type.
        let type_count = (try!(reader.read_i16::<BigEndian>().map_err(Error::eof)) + 1) as usize;
        let mut resource_count_and_list_offset = None;
        for type_index in 0..type_count {
            let type_id = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));
            let resource_count = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
            let resource_list_offset = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
            if type_id == SFNT {
                resource_count_and_list_offset = Some((resource_count, resource_list_offset));
                break
            }
        }

        // Unpack the resource count and list offset.
        let resource_count;
        match resource_count_and_list_offset {
            None => return Err(Error::Failed),
            Some((count, resource_list_offset)) => {
                resource_count = count;
                reader = bytes;
                try!(reader.jump(resource_map_offset as usize + type_list_offset as usize +
                                 resource_list_offset as usize).map_err(Error::eof));
            }
        }

        // Find the font we're interested in.
        //
        // TODO(pcwalton): This only gets the first one. Allow the user of this library to select
        // others.
        let sfnt_id = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
        let sfnt_name_offset = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
        let sfnt_data_offset = try!(reader.read_u32::<BigEndian>().map_err(Error::eof)) &
            0x00ffffff;
        let sfnt_ptr = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));

        // Load the resource.
        reader = bytes;
        try!(reader.jump(resource_data_offset as usize + sfnt_data_offset as usize)
                   .map_err(Error::eof));
        let sfnt_size = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));
        Font::from_otf(&reader[0..sfnt_size as usize], 0)
    }

    /// Returns the glyph IDs that map to the given ranges of Unicode codepoints.
    ///
    /// The returned glyph ranges are in the same order as the codepoints.
    #[inline]
    pub fn glyph_ranges_for_codepoint_ranges(&self, codepoint_ranges: &[CodepointRange])
                                             -> Result<GlyphRanges, Error> {
        self.cmap.glyph_ranges_for_codepoint_ranges(codepoint_ranges)
    }

    /// Calls the given callback for each point in the supplied glyph's contour.
    ///
    /// This function is the primary method for accessing a glyph's outline.
    #[inline]
    pub fn for_each_point<F>(&self, glyph_id: u16, callback: F) -> Result<(), Error>
                             where F: FnMut(&Point) {
        match self.glyf {
            Some(glyf) => {
                let loca = match self.loca {
                    Some(ref loca) => loca,
                    None => return Err(Error::RequiredTableMissing),
                };

                glyf.for_each_point(&self.head, loca, glyph_id, callback)
            }
            None => Ok(()),
        }
    }

    /// Returns the boundaries of the given glyph in font units.
    #[inline]
    pub fn glyph_bounds(&self, glyph_id: u16) -> Result<GlyphBoundsI, Error> {
        match self.glyf {
            Some(glyf) => {
                let loca = match self.loca {
                    Some(ref loca) => loca,
                    None => return Err(Error::RequiredTableMissing),
                };

                glyf.glyph_bounds(&self.head, loca, glyph_id)
            }
            None => Err(Error::RequiredTableMissing),
        }
    }

    /// Returns the minimum shelf height that an atlas containing glyphs from this font will need.
    #[inline]
    pub fn shelf_height(&self, point_size: f32) -> u32 {
        // Add 2 to account for the border.
        self.head
            .max_glyph_bounds
            .pixel_rect_f(self.head.units_per_em, point_size)
            .to_i()
            .size()
            .height as u32 + 2
    }

    /// Returns the number of font units per em.
    ///
    /// An em is traditionally the width of the lowercase letter "m". A typical point size of a
    /// font is expressed in number of pixels per em. Thus, in order to convert font units to
    /// pixels, you can use an expression like `units * font_size / font.units_per_em()`.
    #[inline]
    pub fn units_per_em(&self) -> u16 {
        self.head.units_per_em
    }

    /// Returns the horizontal metrics for the glyph with the given ID.
    ///
    /// Horizontal metrics are important for text shaping, as they specify the number of units to
    /// advance the pen after typesetting a glyph.
    #[inline]
    pub fn metrics_for_glyph(&self, glyph_id: u16) -> Result<HorizontalMetrics, Error> {
        self.hmtx.metrics_for_glyph(&self.hhea, glyph_id)
    }
}

/// Errors that can occur when parsing OpenType fonts.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Error {
    /// A miscellaneous error occurred.
    Failed,
    /// The file ended unexpectedly.
    UnexpectedEof,
    /// The file declared that it was in a version of the format we don't support.
    UnsupportedVersion,
    /// The file was of a format we don't support.
    UnknownFormat,
    /// The font has CFF outlines, which we don't yet support.
    UnsupportedCffOutlines,
    /// The font had a glyph format we don't support.
    UnsupportedGlyphFormat,
    /// We don't support the declared version of the font's character map.
    UnsupportedCmapVersion,
    /// The font character map has an unsupported platform/encoding ID.
    UnsupportedCmapEncoding,
    /// The font character map has an unsupported format.
    UnsupportedCmapFormat,
    /// We don't support the declared version of the font header.
    UnsupportedHeadVersion,
    /// We don't support the declared version of the font's horizontal metrics.
    UnsupportedHheaVersion,
    /// A required table is missing.
    RequiredTableMissing,
    /// The glyph is a composite glyph.
    ///
    /// TODO(pcwalton): Support these.
    CompositeGlyph,
}

impl Error {
    #[doc(hidden)]
    #[inline]
    pub fn eof<T>(_: T) -> Error {
        Error::UnexpectedEof
    }
}

