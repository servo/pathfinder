// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// An inclusive codepoint range.
#[derive(Clone, Copy, Debug)]
pub struct CodepointRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug)]
pub struct CodepointRanges {
    pub ranges: Vec<CodepointRange>,
}

impl CodepointRange {
    #[inline]
    pub fn new(start: u32, end: u32) -> CodepointRange {
        CodepointRange {
            start: start,
            end: end,
        }
    }

    #[inline]
    pub fn iter(&self) -> CodepointRangeIter {
        CodepointRangeIter {
            start: self.start,
            end: self.end,
        }
    }
}

impl CodepointRanges {
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

