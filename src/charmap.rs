// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A font's mapping from Unicode codepoints (characters) to glyphs.
//!
//! Consulting this table is typically the first step when rendering some text.

/// A consecutive series of Unicode codepoints.
#[derive(Clone, Copy, Debug)]
pub struct CodepointRange {
    /// The starting code point, inclusive.
    pub start: u32,
    /// The ending code point, *inclusive*.
    pub end: u32,
}

/// A collection of Unicode codepoints, organized into consecutive series.
#[derive(Clone, Debug)]
pub struct CodepointRanges {
    /// Consecutive series of codepoints.
    pub ranges: Vec<CodepointRange>,
}

impl CodepointRange {
    /// Creates a new codepoint range from the given start and end codepoints, *inclusive*.
    #[inline]
    pub fn new(start: u32, end: u32) -> CodepointRange {
        CodepointRange {
            start: start,
            end: end,
        }
    }

    /// Returns an iterator that iterates over all codepoints in this range.
    #[inline]
    pub fn iter(&self) -> CodepointRangeIter {
        CodepointRangeIter {
            start: self.start,
            end: self.end,
        }
    }
}

impl CodepointRanges {
    /// Creates codepoint ranges from a sorted array of characters, collapsing duplicates.
    ///
    /// This is useful when creating an atlas from a string. The array can be readily produced with
    /// an expression like `"Hello world".chars().collect()`.
    pub fn from_sorted_chars(chars: &[char]) -> CodepointRanges {
        let mut ranges: Vec<CodepointRange> = vec![];
        for &ch in chars {
            match ranges.last_mut() {
                Some(ref mut range) if range.end == ch as u32 => continue,
                Some(ref mut range) if range.end == ch as u32 + 1 => {
                    range.end += 1;
                    continue
                }
                _ => {}
            }
            ranges.push(CodepointRange::new(ch as u32, ch as u32))
        }

        CodepointRanges {
            ranges: ranges,
        }
    }
}

/// An iterator over all codepoints in a range.
pub struct CodepointRangeIter {
    start: u32,
    end: u32,
}

impl Iterator for CodepointRangeIter {
    type Item = u32;

    #[inline]
    fn next(&mut self) -> Option<u32> {
        if self.start > self.end {
            None
        } else {
            let item = self.start;
            self.start += 1;
            Some(item)
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct GlyphRange {
    /// The starting glyph ID in the range, inclusive.
    pub start: u16,
    /// The ending glyph ID in the range, *inclusive*.
    pub end: u16,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct MappedGlyphRange {
    pub codepoint_start: u32,
    pub glyphs: GlyphRange,
}

/// A map from Unicode codepoints to glyph IDs.
#[derive(Clone, Debug)]
pub struct GlyphMapping {
    ranges: Vec<MappedGlyphRange>,
}

impl GlyphRange {
    /// Returns an iterator over every glyph in this range.
    #[inline]
    pub fn iter(&self) -> GlyphRangeIter {
        GlyphRangeIter {
            start: self.start,
            end: self.end,
        }
    }
}

impl GlyphMapping {
    #[doc(hidden)]
    #[inline]
    pub fn new() -> GlyphMapping {
        GlyphMapping {
            ranges: vec![],
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn push(&mut self, range: MappedGlyphRange) {
        self.ranges.push(range)
    }


    #[inline]
    pub fn iter(&self) -> GlyphMappingIter {
        if self.ranges.is_empty() {
            return GlyphMappingIter {
                start: GlyphRangesIndex {
                    range_index: 0,
                    glyph_index: 0,
                },
                end: GlyphRangesIndex {
                    range_index: 0,
                    glyph_index: 0,
                },
                codepoint: 0,
                ranges: &self.ranges,
            }
        }

        GlyphMappingIter {
            start: GlyphRangesIndex {
                range_index: 0,
                glyph_index: self.ranges[0].glyphs.start,
            },
            end: GlyphRangesIndex {
                range_index: (self.ranges.len() - 1) as u16,
                glyph_index: self.ranges.last().unwrap().glyphs.end,
            },
            codepoint: self.ranges[0].codepoint_start,
            ranges: &self.ranges,
        }
    }

    pub fn glyph_for(&self, codepoint: u32) -> Option<u16> {
        let (mut lo, mut hi) = (0, self.ranges.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            if codepoint < self.ranges[mid].codepoint_start {
                hi = mid
            } else if codepoint > self.ranges[mid].codepoint_end() {
                lo = mid + 1
            } else {
                return Some((codepoint - self.ranges[mid].codepoint_start) as u16 +
                            self.ranges[mid].glyphs.start)
            }
        }
        None
    }
}

#[derive(Clone)]
pub struct GlyphRangeIter {
    start: u16,
    end: u16,
}

impl Iterator for GlyphRangeIter {
    type Item = u16;

    #[inline]
    fn next(&mut self) -> Option<u16> {
        if self.start > self.end {
            None
        } else {
            let item = self.start;
            self.start += 1;
            Some(item)
        }
    }
}

/// An iterator over the codepoint-to-glyph mapping.
///
/// Every call to `next()` returns a tuple consisting of the codepoint and glyph ID, in that order.
#[derive(Clone)]
pub struct GlyphMappingIter<'a> {
    start: GlyphRangesIndex,
    end: GlyphRangesIndex,
    codepoint: u32,
    ranges: &'a [MappedGlyphRange],
}

impl<'a> Iterator for GlyphMappingIter<'a> {
    type Item = (u32, u16);

    #[inline]
    fn next(&mut self) -> Option<(u32, u16)> {
        if self.start.range_index > self.end.range_index {
            return None
        }

        let item = (self.codepoint, self.start.glyph_index);

        self.codepoint += 1;
        self.start.glyph_index += 1;

        while self.start.glyph_index > self.ranges[self.start.range_index as usize].glyphs.end {
            self.start.range_index += 1;
            if self.start.range_index > self.end.range_index {
                break
            }

            self.start.glyph_index = self.ranges[self.start.range_index as usize].glyphs.start;
            self.codepoint = self.ranges[self.start.range_index as usize].codepoint_start;
        }

        Some(item)
    }
}

#[derive(Clone, Copy, Debug)]
struct GlyphRangesIndex {
    range_index: u16,
    glyph_index: u16,
}

impl MappedGlyphRange {
    /// Inclusive.
    #[inline]
    pub fn codepoint_end(&self) -> u32 {
        self.codepoint_start + self.glyphs.end as u32 - self.glyphs.start as u32
    }
}
