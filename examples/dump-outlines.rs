/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

extern crate memmap;
extern crate pathfinder;

use memmap::{Mmap, Protection};
use pathfinder::batch::{CodepointBatch, CodepointRange, GlyphBatch};
use pathfinder::otf::FontData;
use std::env;

fn main() {
    let file = Mmap::open_path(env::args().nth(1).unwrap(), Protection::Read).unwrap();
    unsafe {
        let font = FontData::new(file.as_slice());
        let mut glyph_batch = GlyphBatch::new();
        glyph_batch.find_glyph_ranges_for_codepoint_ranges(&CodepointBatch {
            ranges: vec![CodepointRange::new('A' as u32, 'Z' as u32, 0)],
            fonts: vec![font],
        }).unwrap();
    }
}

