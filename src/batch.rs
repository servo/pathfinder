// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use otf::FontData;

#[derive(Clone, Copy, Debug)]
pub struct CodepointRange {
    pub start: u32,
    pub end: u32,
    pub font_index: u32,
}

impl CodepointRange {
    #[inline]
    pub fn new(start: u32, end: u32, font_index: u32) -> CodepointRange {
        CodepointRange {
            start: start,
            end: end,
            font_index: font_index,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GlyphRange {
    pub start: u32,
    pub end: u32,
    pub font_index: u32,
}

#[derive(Clone)]
pub struct CodepointBatch<'a> {
    pub ranges: Vec<CodepointRange>,
    pub fonts: Vec<FontData<'a>>,
}

#[derive(Clone)]
pub struct GlyphBatch<'a> {
    pub ranges: Vec<GlyphRange>,
    pub fonts: Vec<FontData<'a>>,
}

impl<'a> GlyphBatch<'a> {
    pub fn new<'b>() -> GlyphBatch<'b> {
        GlyphBatch {
            ranges: vec![],
            fonts: vec![],
        }
    }
}

