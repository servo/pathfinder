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

pub use otf::glyf::{Point, PointKind};

use byteorder::{BigEndian, ReadBytesExt};
use charmap::{CodepointRange, GlyphMapping};
use otf::cff::CffTable;
use otf::cmap::CmapTable;
use otf::glyf::GlyfTable;
use otf::head::HeadTable;
use otf::hhea::HheaTable;
use otf::hmtx::{HmtxTable, HorizontalMetrics};
use otf::kern::KernTable;
use otf::loca::LocaTable;
use otf::os_2::Os2Table;
use outline::GlyphBounds;
use std::mem;
use std::u16;
use util::Jump;

mod cff;
mod cmap;
mod glyf;
mod head;
mod hhea;
mod hmtx;
mod kern;
mod loca;
mod os_2;

const CFF:  u32 = ((b'C' as u32) << 24) |
                  ((b'F' as u32) << 16) |
                  ((b'F' as u32) << 8)  |
                   (b' ' as u32);
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
const KERN: u32 = ((b'k' as u32) << 24) |
                  ((b'e' as u32) << 16) |
                  ((b'r' as u32) << 8)  |
                   (b'n' as u32);
const LOCA: u32 = ((b'l' as u32) << 24) |
                  ((b'o' as u32) << 16) |
                  ((b'c' as u32) << 8)  |
                   (b'a' as u32);
const OS_2: u32 = ((b'O' as u32) << 24) |
                  ((b'S' as u32) << 16) |
                  ((b'/' as u32) << 8)  |
                   (b'2' as u32);
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

static SFNT_VERSIONS: [u32; 3] = [
    0x10000,
    ((b't' as u32) << 24) | ((b'r' as u32) << 16) | ((b'u' as u32) << 8) | (b'e' as u32),
    OTTO,
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
    os_2: Os2Table,

    cff: Option<CffTable<'a>>,
    glyf: Option<GlyfTable<'a>>,
    loca: Option<LocaTable<'a>>,
    kern: Option<KernTable<'a>>,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct FontTable<'a> {
    pub bytes: &'a [u8],
}

impl<'a> Font<'a> {
    /// Creates a new font from a byte buffer containing the contents of a file or font collection
    /// (`.ttf`, `.ttc`, `.otf`, etc.)
    ///
    /// If this is a `.ttc` or `.dfont` collection, this returns the first font within it. If you
    /// want to read another one, use the `Font::from_collection_index` API.
    ///
    /// Returns the font on success or an error on failure.
    pub fn new<'b>(bytes: &'b [u8]) -> Result<Font<'b>, Error> {
        Font::from_collection_index(bytes, 0)
    }

    /// Creates a new font from a single font within a byte buffer containing the contents of a
    /// file or a font collection (`.ttf`, `.ttc`, `.otf`, etc.)
    ///
    /// If this is a `.ttc` or `.dfont` collection, this returns the appropriate font within it.
    ///
    /// Returns the font on success or an error on failure.
    pub fn from_collection_index<'b>(bytes: &'b [u8], index: u32) -> Result<Font<'b>, Error> {
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
                if index >= num_fonts {
                    return Err(Error::FontIndexOutOfBounds)
                }

                try!(reader.jump(index as usize * mem::size_of::<u32>()).map_err(Error::eof));
                let table_offset = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));
                Font::from_otf(&bytes, table_offset)
            }
            magic_number if SFNT_VERSIONS.contains(&magic_number) => Font::from_otf(bytes, 0),
            0x0100 => Font::from_dfont_index(bytes, index),
            _ => Err(Error::UnknownFormat),
        }
    }

    fn from_otf<'b>(bytes: &'b [u8], offset: u32) -> Result<Font<'b>, Error> {
        let mut reader = bytes;
        try!(reader.jump(offset as usize).map_err(Error::eof));

        let mut magic_number = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));

        // Check version.
        if !SFNT_VERSIONS.contains(&magic_number) {
            return Err(Error::UnknownFormat)
        }

        let num_tables = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
        try!(reader.jump(mem::size_of::<u16>() * 3).map_err(Error::eof));

        let (mut cff_table, mut cmap_table) = (None, None);
        let (mut glyf_table, mut head_table) = (None, None);
        let (mut hhea_table, mut hmtx_table) = (None, None);
        let (mut kern_table, mut loca_table) = (None, None);
        let mut os_2_table = None;

        for _ in 0..num_tables {
            let table_id = try!(reader.read_u32::<BigEndian>().map_err(Error::eof));

            // Skip over the checksum.
            try!(reader.read_u32::<BigEndian>().map_err(Error::eof));

            let offset = try!(reader.read_u32::<BigEndian>().map_err(Error::eof)) as usize;
            let length = try!(reader.read_u32::<BigEndian>().map_err(Error::eof)) as usize;

            let mut slot = match table_id {
                CFF => &mut cff_table,
                CMAP => &mut cmap_table,
                HEAD => &mut head_table,
                HHEA => &mut hhea_table,
                HMTX => &mut hmtx_table,
                GLYF => &mut glyf_table,
                KERN => &mut kern_table,
                LOCA => &mut loca_table,
                OS_2 => &mut os_2_table,
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

        let cff_table = match cff_table {
            None => None,
            Some(cff_table) => Some(try!(CffTable::new(cff_table))),
        };

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
            os_2: try!(Os2Table::new(try!(os_2_table.ok_or(Error::RequiredTableMissing)))),

            cff: cff_table,
            glyf: glyf_table.map(GlyfTable::new),
            loca: loca_table,
            kern: kern_table.and_then(|table| KernTable::new(table).ok()),
        })
    }

    /// https://github.com/kreativekorp/ksfl/wiki/Macintosh-Resource-File-Format
    fn from_dfont_index<'b>(bytes: &'b [u8], index: u32) -> Result<Font<'b>, Error> {
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

        // Check whether the index is in bounds.
        if index >= resource_count as u32 + 1 {
            return Err(Error::FontIndexOutOfBounds)
        }

        // Find the font we're interested in.
        try!(reader.jump(index as usize * (mem::size_of::<u16>() * 2 + mem::size_of::<u32>() * 2))
                   .map_err(Error::eof));
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
    pub fn glyph_mapping_for_codepoint_ranges(&self, codepoint_ranges: &[CodepointRange])
                                              -> Result<GlyphMapping, Error> {
        self.cmap.glyph_mapping_for_codepoint_ranges(codepoint_ranges)
    }

    /// Calls the given callback for each point in the supplied glyph's contour.
    ///
    /// This function is the primary method for accessing a glyph's outline.
    #[inline]
    pub fn for_each_point<F>(&self, glyph_id: u16, callback: F) -> Result<(), Error>
                             where F: FnMut(&Point) {
        match (self.glyf, self.cff) {
            (Some(glyf), None) => {
                let loca = match self.loca {
                    Some(ref loca) => loca,
                    None => return Err(Error::RequiredTableMissing),
                };

                glyf.for_each_point(&self.head, loca, glyph_id, callback)
            }
            (None, Some(cff)) => cff.for_each_point(glyph_id, callback),
            (Some(_), Some(_)) => Err(Error::Failed),
            (None, None) => Ok(()),
        }
    }

    /// Returns the boundaries of the given glyph in font units.
    #[inline]
    pub fn glyph_bounds(&self, glyph_id: u16) -> Result<GlyphBounds, Error> {
        match (self.glyf, self.cff) {
            (Some(glyf), None) => {
                let loca = match self.loca {
                    Some(ref loca) => loca,
                    None => return Err(Error::RequiredTableMissing),
                };

                glyf.glyph_bounds(&self.head, loca, glyph_id)
            }
            (None, Some(cff)) => cff.glyph_bounds(glyph_id),
            (Some(_), Some(_)) => Err(Error::Failed),
            (None, None) => Err(Error::RequiredTableMissing),
        }
    }

    /// Returns the minimum shelf height that an atlas containing glyphs from this font will need.
    #[inline]
    pub fn shelf_height(&self, point_size: f32) -> u32 {
        // Add 2 to account for the border.
        self.head
            .max_glyph_bounds
            .subpixel_bounds(self.head.units_per_em, point_size)
            .round_out()
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

    /// Returns the kerning between the given two glyph IDs in font units.
    ///
    /// Positive values move glyphs farther apart; negative values move glyphs closer together.
    ///
    /// Zero is returned if no kerning is available in the font.
    #[inline]
    pub fn kerning_for_glyph_pair(&self, left_glyph_id: u16, right_glyph_id: u16) -> i16 {
        match self.kern {
            None => 0,
            Some(kern) => kern.kerning_for_glyph_pair(left_glyph_id, right_glyph_id).unwrap_or(0),
        }
    }

    /// Returns the distance from the baseline to the top of the text box in font units.
    ///
    /// The following expression computes the baseline-to-baseline height:
    /// `font.ascender() - font.descender() + font.line_gap()`.
    #[inline]
    pub fn ascender(&self) -> i16 {
        self.os_2.typo_ascender
    }

    /// Returns the distance from the baseline to the bottom of the text box in font units.
    ///
    /// The following expression computes the baseline-to-baseline height:
    /// `font.ascender() - font.descender() + font.line_gap()`.
    #[inline]
    pub fn descender(&self) -> i16 {
        self.os_2.typo_descender
    }

    /// Returns the recommended extra gap between lines in font units.
    ///
    /// The following expression computes the baseline-to-baseline height:
    /// `font.ascender() - font.descender() + font.line_gap()`.
    #[inline]
    pub fn line_gap(&self) -> i16 {
        self.os_2.typo_line_gap
    }
}

/// Errors that can occur when parsing OpenType fonts.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Error {
    /// A miscellaneous error occurred.
    Failed,
    /// The file ended unexpectedly.
    UnexpectedEof,
    /// There is no font with this index in this font collection.
    FontIndexOutOfBounds,
    /// The file declared that it was in a version of the format we don't support.
    UnsupportedVersion,
    /// The file was of a format we don't support.
    UnknownFormat,
    /// The font had a glyph format we don't support.
    UnsupportedGlyphFormat,
    /// We don't support the declared version of the font's CFF outlines.
    UnsupportedCffVersion,
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
    /// We don't support the declared version of the font's OS/2 and Windows table.
    UnsupportedOs2Version,
    /// A required table is missing.
    RequiredTableMissing,
    /// An integer in a CFF DICT was not found.
    CffIntegerNotFound,
    /// The CFF Top DICT was not found.
    CffTopDictNotFound,
    /// A CFF `Offset` value was formatted incorrectly.
    CffBadOffset,
    /// The CFF evaluation stack overflowed.
    CffStackOverflow,
    /// An unimplemented CFF CharString operator was encountered.
    CffUnimplementedOperator,
}

impl Error {
    #[doc(hidden)]
    #[inline]
    pub fn eof<T>(_: T) -> Error {
        Error::UnexpectedEof
    }
}

