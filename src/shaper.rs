// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A very basic text shaper for simple needs.
//!
//! Do not use this for international or high-quality text. This shaper does not do kerning,
//! ligation, or advanced typography features (`GSUB`, `GPOS`, text morphing). Consider HarfBuzz or
//! the system shaper instead.

use charmap::GlyphMapping;
use otf::Font;
use std::cmp;

/// Shapes the given Unicode text in the given font, returning the proper position for each glyph.
///
/// See the description of this module for caveats.
///
/// For proper operation, the given `glyph_mapping` must include all the glyphs necessary to render
/// the string.
pub fn shape_text(font: &Font, glyph_mapping: &GlyphMapping, string: &str) -> Vec<GlyphPos> {
    string.chars().map(|ch| {
        let glyph_id = glyph_mapping.glyph_for(ch as u32).unwrap_or(0);
        let metrics = font.metrics_for_glyph(glyph_id);

        let advance = match metrics {
            Err(_) => 0,
            Ok(metrics) => metrics.advance_width,
        };

        GlyphPos {
            glyph_id: glyph_id,
            advance: advance,
        }
    }).collect()
}

/// The position of a glyph after shaping.
#[derive(Clone, Copy, Debug)]
pub struct GlyphPos {
    /// The glyph ID to emit.
    pub glyph_id: u16,
    /// The amount to move the cursor forward *after* emitting this glyph.
    pub advance: u16,
}

