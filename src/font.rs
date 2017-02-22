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
use charmap::{CodepointRange, GlyphMapping};
use containers::dfont;
use containers::otf::{FontTables, SFNT_VERSIONS};
use containers::ttc;
use containers::woff;
use error::FontError;
use euclid::Point2D;
use outline::GlyphBounds;
use tables::hmtx::HorizontalMetrics;

/// A handle to a font backed by a byte buffer containing the contents of the file (`.ttf`,
/// `.otf`), etc.
///
/// For optimum performance, consider using the `memmap` crate to provide the byte buffer.
pub struct Font<'a> {
    pub bytes: &'a [u8],
    tables: FontTables<'a>,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct FontTable<'a> {
    pub bytes: &'a [u8],
}

impl<'a> Font<'a> {
    #[doc(hidden)]
    pub fn from_tables<'b>(bytes: &'b [u8], tables: FontTables<'b>) -> Font<'b> {
        Font {
            bytes: bytes,
            tables: tables,
        }
    }

    /// Creates a new font from a byte buffer containing the contents of a file or font collection
    /// (`.ttf`, `.ttc`, `.otf`, etc.)
    ///
    /// If this is a `.ttc` or `.dfont` collection, this returns the first font within it. If you
    /// want to read another one, use the `Font::from_collection_index` API.
    ///
    /// The supplied `buffer` is an arbitrary vector that may or may not be used as a temporary
    /// storage space. Typically you will want to just pass an empty vector here.
    ///
    /// Returns the font on success or an error on failure.
    pub fn new<'b>(bytes: &'b [u8], buffer: &'b mut Vec<u8>) -> Result<Font<'b>, FontError> {
        Font::from_collection_index(bytes, 0, buffer)
    }

    /// Creates a new font from a single font within a byte buffer containing the contents of a
    /// file or a font collection (`.ttf`, `.ttc`, `.otf`, etc.)
    ///
    /// If this is a `.ttc` or `.dfont` collection, this returns the appropriate font within it.
    ///
    /// The supplied `buffer` is an arbitrary vector that may or may not be used as a temporary
    /// storage space. Typically you will want to just pass an empty vector here.
    ///
    /// Returns the font on success or an error on failure.
    pub fn from_collection_index<'b>(bytes: &'b [u8], index: u32, buffer: &'b mut Vec<u8>)
                                     -> Result<Font<'b>, FontError> {
        // Check the magic number.
        let mut reader = bytes;
        let magic_number = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
        match magic_number {
            ttc::MAGIC_NUMBER => Font::from_ttc_index(bytes, index),
            woff::MAGIC_NUMBER => Font::from_woff(bytes, buffer),
            dfont::MAGIC_NUMBER => Font::from_dfont_index(bytes, index),
            magic_number if SFNT_VERSIONS.contains(&magic_number) => Font::from_otf(bytes, 0),
            _ => Err(FontError::UnknownFormat),
        }
    }

    /// Returns the glyph IDs that map to the given ranges of Unicode codepoints.
    ///
    /// The returned glyph ranges are in the same order as the codepoints.
    #[inline]
    pub fn glyph_mapping_for_codepoint_ranges(&self, codepoint_ranges: &[CodepointRange])
                                              -> Result<GlyphMapping, FontError> {
        self.tables.cmap.glyph_mapping_for_codepoint_ranges(codepoint_ranges)
    }

    /// Calls the given callback for each point in the supplied glyph's contour.
    ///
    /// This function is the primary method for accessing a glyph's outline.
    #[inline]
    pub fn for_each_point<F>(&self, glyph_id: u16, callback: F) -> Result<(), FontError>
                             where F: FnMut(&Point) {
        match (self.tables.glyf, self.tables.cff) {
            (Some(glyf), None) => {
                let loca = match self.tables.loca {
                    Some(ref loca) => loca,
                    None => return Err(FontError::RequiredTableMissing),
                };

                glyf.for_each_point(&self.tables.head, loca, glyph_id, callback)
            }
            (None, Some(cff)) => cff.for_each_point(glyph_id, callback),
            (Some(_), Some(_)) => Err(FontError::Failed),
            (None, None) => Ok(()),
        }
    }

    /// Returns the boundaries of the given glyph in font units.
    #[inline]
    pub fn glyph_bounds(&self, glyph_id: u16) -> Result<GlyphBounds, FontError> {
        match (self.tables.glyf, self.tables.cff) {
            (Some(glyf), None) => {
                let loca = match self.tables.loca {
                    Some(ref loca) => loca,
                    None => return Err(FontError::RequiredTableMissing),
                };

                glyf.glyph_bounds(&self.tables.head, loca, glyph_id)
            }
            (None, Some(cff)) => cff.glyph_bounds(glyph_id),
            (Some(_), Some(_)) => Err(FontError::Failed),
            (None, None) => Err(FontError::RequiredTableMissing),
        }
    }

    /// Returns the minimum shelf height that an atlas containing glyphs from this font will need.
    #[inline]
    pub fn shelf_height(&self, point_size: f32) -> u32 {
        // Add 2 to account for the border.
        self.tables.head
            .max_glyph_bounds
            .subpixel_bounds(self.tables.head.units_per_em, point_size)
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
        self.tables.head.units_per_em
    }

    /// Returns the horizontal metrics for the glyph with the given ID.
    ///
    /// Horizontal metrics are important for text shaping, as they specify the number of units to
    /// advance the pen after typesetting a glyph.
    #[inline]
    pub fn metrics_for_glyph(&self, glyph_id: u16) -> Result<HorizontalMetrics, FontError> {
        self.tables.hmtx.metrics_for_glyph(&self.tables.hhea, glyph_id)
    }

    /// Returns the kerning between the given two glyph IDs in font units.
    ///
    /// Positive values move glyphs farther apart; negative values move glyphs closer together.
    ///
    /// Zero is returned if no kerning is available in the font.
    #[inline]
    pub fn kerning_for_glyph_pair(&self, left_glyph_id: u16, right_glyph_id: u16) -> i16 {
        match self.tables.kern {
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
        self.tables.os_2.typo_ascender
    }

    /// Returns the distance from the baseline to the bottom of the text box in font units.
    ///
    /// The following expression computes the baseline-to-baseline height:
    /// `font.ascender() - font.descender() + font.line_gap()`.
    #[inline]
    pub fn descender(&self) -> i16 {
        self.tables.os_2.typo_descender
    }

    /// Returns the recommended extra gap between lines in font units.
    ///
    /// The following expression computes the baseline-to-baseline height:
    /// `font.ascender() - font.descender() + font.line_gap()`.
    #[inline]
    pub fn line_gap(&self) -> i16 {
        self.tables.os_2.typo_line_gap
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Point {
    /// Where the point is located in glyph space.
    pub position: Point2D<i16>,

    /// The index of the point in this contour.
    ///
    /// When iterating over points via `for_each_point`, a value of 0 here indicates that a new
    /// contour begins.
    pub index_in_contour: u16,

    /// The kind of point this is.
    pub kind: PointKind,
}

/// The type of point.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PointKind {
    /// The point is on the curve.
    OnCurve,
    /// The point is a quadratic control point.
    QuadControl,
    /// The point is the first cubic control point.
    FirstCubicControl,
    /// The point is the second cubic control point.
    SecondCubicControl,
}

