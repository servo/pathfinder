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
use charmap::CodepointRange;
use glyph_range::{GlyphRange, GlyphRanges, MappedGlyphRange};
use otf::{Error, FontTable};
use std::char;
use std::cmp;
use std::mem;
use std::u16;
use util::Jump;

const PLATFORM_ID_UNICODE: u16 = 0;
const PLATFORM_ID_MICROSOFT: u16 = 3;

const MICROSOFT_ENCODING_ID_UNICODE_BMP: u16 = 1;
const MICROSOFT_ENCODING_ID_UNICODE_UCS4: u16 = 10;

const FORMAT_SEGMENT_MAPPING_TO_DELTA_VALUES: u16 = 4;

const MISSING_GLYPH: u16 = 0;

#[derive(Clone, Copy)]
pub struct CmapTable<'a> {
    table: FontTable<'a>,
}

impl<'a> CmapTable<'a> {
    pub fn new(table: FontTable) -> CmapTable {
        CmapTable {
            table: table,
        }
    }

    pub fn glyph_ranges_for_codepoint_ranges(&self, codepoint_ranges: &[CodepointRange])
                                             -> Result<GlyphRanges, Error> {
        let mut cmap_reader = self.table.bytes;

        // Check version.
        if try!(cmap_reader.read_u16::<BigEndian>().map_err(Error::eof)) != 0 {
            return Err(Error::UnsupportedCmapVersion)
        }

        let num_tables = try!(cmap_reader.read_u16::<BigEndian>().map_err(Error::eof));

        // Check platform ID and encoding.
        // TODO(pcwalton): Handle more.
        let mut table_found = false;
        for _ in 0..num_tables {
            let platform_id = try!(cmap_reader.read_u16::<BigEndian>().map_err(Error::eof));
            let encoding_id = try!(cmap_reader.read_u16::<BigEndian>().map_err(Error::eof));
            let offset = try!(cmap_reader.read_u32::<BigEndian>().map_err(Error::eof));
            match (platform_id, encoding_id) {
                (PLATFORM_ID_UNICODE, _) |
                (PLATFORM_ID_MICROSOFT, MICROSOFT_ENCODING_ID_UNICODE_BMP) |
                (PLATFORM_ID_MICROSOFT, MICROSOFT_ENCODING_ID_UNICODE_UCS4) => {
                    // Move to the mapping table.
                    cmap_reader = self.table.bytes;
                    try!(cmap_reader.jump(offset as usize).map_err(Error::eof));
                    table_found = true;
                    break
                }
                _ => {}
            }
        }

        if !table_found {
            return Err(Error::UnsupportedCmapEncoding)
        }

        // Check the mapping table format.
        let format = try!(cmap_reader.read_u16::<BigEndian>().map_err(Error::eof));
        if format != FORMAT_SEGMENT_MAPPING_TO_DELTA_VALUES {
            return Err(Error::UnsupportedCmapFormat)
        }

        // Read the mapping table header.
        let _length = try!(cmap_reader.read_u16::<BigEndian>().map_err(Error::eof));
        let _language = try!(cmap_reader.read_u16::<BigEndian>().map_err(Error::eof));
        let seg_count = try!(cmap_reader.read_u16::<BigEndian>().map_err(Error::eof)) / 2;
        let _search_range = try!(cmap_reader.read_u16::<BigEndian>().map_err(Error::eof));
        let _entry_selector = try!(cmap_reader.read_u16::<BigEndian>().map_err(Error::eof));
        let _range_shift = try!(cmap_reader.read_u16::<BigEndian>().map_err(Error::eof));

        // Set up parallel array pointers.
        //
        // NB: Microsoft's spec refers to `startCode` and `endCode` as `startCount` and `endCount`
        // respectively in a few places. I believe this is a mistake, and `startCode` and `endCode`
        // are the correct names.
        let (end_codes, mut start_codes) = (cmap_reader, cmap_reader);
        try!(start_codes.jump((seg_count as usize + 1) * mem::size_of::<u16>())
                        .map_err(Error::eof));
        let mut id_deltas = start_codes;
        try!(id_deltas.jump(seg_count as usize * mem::size_of::<u16>()).map_err(Error::eof));
        let mut id_range_offsets = id_deltas;
        try!(id_range_offsets.jump(seg_count as usize * mem::size_of::<u16>())
                             .map_err(Error::eof));
        let mut glyph_ids = id_range_offsets;
        try!(glyph_ids.jump(seg_count as usize * mem::size_of::<u16>()).map_err(Error::eof));

        // Now perform the lookups.
        let mut glyph_ranges = GlyphRanges::new();
        for codepoint_range in codepoint_ranges {
            let mut codepoint_range = *codepoint_range;
            while codepoint_range.end >= codepoint_range.start {
                if codepoint_range.start > u16::MAX as u32 {
                    glyph_ranges.ranges.push(MappedGlyphRange {
                        codepoint_start: codepoint_range.start,
                        glyphs: GlyphRange {
                            start: MISSING_GLYPH,
                            end: MISSING_GLYPH,
                        },
                    });
                    codepoint_range.start += 1;
                    continue
                }

                let start_codepoint_range = codepoint_range.start as u16;
                let mut end_codepoint_range = codepoint_range.end as u16;

                // Binary search to find the segment.
                let (mut low, mut high) = (0, seg_count);
                let mut segment_index = None;
                while low < high {
                    let mid = (low + high) / 2;

                    let mut end_code = end_codes;
                    try!(end_code.jump(mid as usize * 2).map_err(Error::eof));
                    let end_code = try!(end_code.read_u16::<BigEndian>().map_err(Error::eof));
                    if start_codepoint_range > end_code {
                        low = mid + 1;
                        continue
                    }

                    let mut start_code = start_codes;
                    try!(start_code.jump(mid as usize * 2).map_err(Error::eof));
                    let start_code = try!(start_code.read_u16::<BigEndian>().map_err(Error::eof));
                    if start_codepoint_range < start_code {
                        high = mid;
                        continue
                    }

                    segment_index = Some(mid);
                    break
                }

                let segment_index = match segment_index {
                    Some(segment_index) => segment_index,
                    None => {
                        glyph_ranges.ranges.push(MappedGlyphRange {
                            codepoint_start: codepoint_range.start,
                            glyphs: GlyphRange {
                                start: MISSING_GLYPH,
                                end: MISSING_GLYPH,
                            },
                        });
                        codepoint_range.start += 1;
                        continue
                    }
                };

                // Read out the segment info.
                let mut start_code = start_codes;
                let mut end_code = end_codes;
                let mut id_range_offset = id_range_offsets;
                let mut id_delta = id_deltas;
                try!(start_code.jump(segment_index as usize * 2).map_err(Error::eof));
                try!(end_code.jump(segment_index as usize * 2).map_err(Error::eof));
                try!(id_range_offset.jump(segment_index as usize * 2).map_err(Error::eof));
                try!(id_delta.jump(segment_index as usize * 2).map_err(Error::eof));
                let start_code = try!(start_code.read_u16::<BigEndian>().map_err(Error::eof));
                let end_code = try!(end_code.read_u16::<BigEndian>().map_err(Error::eof));
                let id_range_offset = try!(id_range_offset.read_u16::<BigEndian>()
                                                          .map_err(Error::eof));
                let id_delta = try!(id_delta.read_i16::<BigEndian>().map_err(Error::eof));

                end_codepoint_range = cmp::min(end_codepoint_range, end_code);
                codepoint_range.start = (end_codepoint_range + 1) as u32;

                let start_code_offset = start_codepoint_range - start_code;
                let end_code_offset = end_codepoint_range - start_code;

                // If we're direct-mapped (`idRangeOffset` = 0), then try to convert as much of the
                // codepoint range as possible to a contiguous glyph range.
                if id_range_offset == 0 {
                    // Microsoft's documentation is contradictory as to whether the code offset or
                    // the actual code is added to the ID delta here. In reality it seems to be the
                    // latter.
                    glyph_ranges.ranges.push(MappedGlyphRange {
                        codepoint_start: start_codepoint_range as u32,
                        glyphs: GlyphRange {
                            start: (start_codepoint_range as i16).wrapping_add(id_delta) as u16,
                            end: (end_codepoint_range as i16).wrapping_add(id_delta) as u16,
                        },
                    });
                    continue
                }

                // Otherwise, look up the glyphs individually.
                for code_offset in start_code_offset..(end_code_offset + 1) {
                    let mut reader = id_range_offsets;
                    try!(reader.jump(segment_index as usize * 2 + code_offset as usize * 2 +
                                     id_range_offset as usize).map_err(Error::eof));
                    let mut glyph_id = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
                    if glyph_id == 0 {
                        glyph_ranges.ranges.push(MappedGlyphRange {
                            codepoint_start: start_code as u32 + code_offset as u32,
                            glyphs: GlyphRange {
                                start: MISSING_GLYPH,
                                end: MISSING_GLYPH,
                            },
                        })
                    } else {
                        glyph_id = (glyph_id as i16).wrapping_add(id_delta) as u16;
                        glyph_ranges.ranges.push(MappedGlyphRange {
                            codepoint_start: start_code as u32 + code_offset as u32,
                            glyphs: GlyphRange {
                                start: glyph_id,
                                end: glyph_id,
                            },
                        })
                    }
                }
            }
        }

        Ok(glyph_ranges)
    }
}

