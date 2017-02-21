// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Simple text layout.
//!
//! Do not use this for international or high-quality text. This layout has all of the limitations
//! of the shaper; additionally, it only does left-to-right text with a uniform page width and no
//! control over line spacing. Use Cocoa's `NSLayoutManager`, Pango, etc. for real use.

use charmap::CodepointRanges;
use euclid::Point2D;
use otf::Font;
use shaper;

pub struct Typesetter {
    pub glyph_positions: Vec<GlyphPosition>,
    page_width: f32,
    cursor: Point2D<f32>,
}

impl Typesetter {
    pub fn new(page_width: f32, initial_font: &Font, initial_point_size: f32) -> Typesetter {
        let pixels_per_unit = initial_point_size / initial_font.units_per_em() as f32;
        let initial_position = initial_font.ascender() as f32 * pixels_per_unit;

        Typesetter {
            glyph_positions: vec![],
            page_width: page_width,
            cursor: Point2D::new(0.0, initial_position),
        }
    }

    pub fn add_text(&mut self, font: &Font, point_size: f32, string: &str) {
        // TODO(pcwalton): Cache this mapping.
        let mut chars: Vec<char> = string.chars().collect();
        chars.push(' ');
        chars.sort();
        let codepoint_ranges = CodepointRanges::from_sorted_chars(&chars);
        let glyph_mapping = font.glyph_mapping_for_codepoint_ranges(&codepoint_ranges.ranges)
                                .unwrap();

        // All of these values are in pixels.
        let pixels_per_unit = point_size / font.units_per_em() as f32;
        let space_advance = font.metrics_for_glyph(glyph_mapping.glyph_for(' ' as u32).unwrap())
                                .unwrap()
                                .advance_width as f32 * pixels_per_unit;
        let line_spacing = (font.ascender() as f32 - font.descender() as f32 +
                            font.line_gap() as f32) * pixels_per_unit;

        for word in string.split_whitespace() {
            let shaped_glyph_positions = shaper::shape_text(&font, &glyph_mapping, word);
            let total_advance = pixels_per_unit *
                shaped_glyph_positions.iter().map(|p| p.advance as f32).sum::<f32>();
            if self.cursor.x + total_advance > self.page_width {
                self.cursor.x = 0.0;
                self.cursor.y += line_spacing;
            }

            for glyph_position in &shaped_glyph_positions {
                self.glyph_positions.push(GlyphPosition {
                    x: self.cursor.x,
                    y: self.cursor.y,
                    glyph_id: glyph_position.glyph_id,
                });
                self.cursor.x += glyph_position.advance as f32 * pixels_per_unit;
            }

            self.cursor.x += space_advance
        }
    }

    pub fn glyph_positions(&self) -> &[GlyphPosition] {
        &self.glyph_positions
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GlyphPosition {
    pub x: f32,
    pub y: f32,
    pub glyph_id: u16,
}

