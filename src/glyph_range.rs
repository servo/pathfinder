// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[derive(Clone, Copy, Debug)]
pub struct GlyphRange {
    pub start: u16,
    pub end: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct MappedGlyphRange {
    pub codepoint_start: u32,
    pub glyphs: GlyphRange,
}

#[derive(Clone, Debug)]
pub struct GlyphRanges {
    pub ranges: Vec<MappedGlyphRange>,
}

impl GlyphRange {
    #[inline]
    pub fn iter(&self) -> GlyphRangeIter {
        GlyphRangeIter {
            start: self.start,
            end: self.end,
        }
    }
}

impl GlyphRanges {
    #[inline]
    pub fn new() -> GlyphRanges {
        GlyphRanges {
            ranges: vec![],
        }
    }

    #[inline]
    pub fn iter(&self) -> GlyphRangesIter {
        if self.ranges.is_empty() {
            return GlyphRangesIter {
                start: GlyphRangesIndex {
                    range_index: 0,
                    glyph_index: 0,
                },
                end: GlyphRangesIndex {
                    range_index: 0,
                    glyph_index: 0,
                },
                ranges: &self.ranges,
            }
        }

        GlyphRangesIter {
            start: GlyphRangesIndex {
                range_index: 0,
                glyph_index: self.ranges[0].glyphs.start,
            },
            end: GlyphRangesIndex {
                range_index: (self.ranges.len() - 1) as u16,
                glyph_index: self.ranges.last().unwrap().glyphs.end,
            },
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

#[derive(Clone)]
pub struct GlyphRangesIter<'a> {
    start: GlyphRangesIndex,
    end: GlyphRangesIndex,
    ranges: &'a [MappedGlyphRange],
}

impl<'a> Iterator for GlyphRangesIter<'a> {
    type Item = u16;

    #[inline]
    fn next(&mut self) -> Option<u16> {
        if self.start.range_index > self.end.range_index {
            return None
        }

        let item = self.start.glyph_index;

        self.start.glyph_index += 1;
        while self.start.glyph_index > self.ranges[self.start.range_index as usize].glyphs.end {
            self.start.range_index += 1;
            if self.start.range_index > self.end.range_index {
                break
            }
            self.start.glyph_index = self.ranges[self.start.range_index as usize].glyphs.start
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

