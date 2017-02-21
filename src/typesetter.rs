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
use error::GlyphStoreCreationError;
use euclid::Point2D;
use otf::Font;
use outline::{OutlineBuilder, Outlines};
use shaper;
use std::u16;

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

    pub fn create_glyph_store(&self, font: &Font) -> Result<GlyphStore, GlyphStoreCreationError> {
        let glyph_ids = self.glyph_positions
                            .iter()
                            .map(|glyph_position| glyph_position.glyph_id)
                            .collect();
        GlyphStore::from_glyph_ids(glyph_ids, font)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GlyphPosition {
    pub x: f32,
    pub y: f32,
    pub glyph_id: u16,
}

pub struct GlyphStore {
    pub outlines: Outlines,
    pub glyph_id_to_glyph_index: Vec<u16>,
    pub all_glyph_indices: Vec<u16>,
}

impl GlyphStore {
    fn from_glyph_ids(mut glyph_ids: Vec<u16>, font: &Font)
                      -> Result<GlyphStore, GlyphStoreCreationError> {
        glyph_ids.sort();
        glyph_ids.dedup();

        let last_glyph_id = match glyph_ids.last() {
            Some(&id) => id + 1,
            None => 0,
        };

        let mut outline_builder = OutlineBuilder::new();
        let mut glyph_id_to_glyph_index = vec![u16::MAX; last_glyph_id as usize];
        let mut all_glyph_indices = vec![];
        for glyph_id in glyph_ids {
            let glyph_index = try!(outline_builder.add_glyph(font, glyph_id)
                                                  .map_err(GlyphStoreCreationError::OtfError));
            glyph_id_to_glyph_index[glyph_id as usize] = glyph_index;
            all_glyph_indices.push(glyph_index);
        }

        let outlines = try!(outline_builder.create_buffers()
                                           .map_err(GlyphStoreCreationError::GlError));

        all_glyph_indices.sort();
        all_glyph_indices.dedup();

        Ok(GlyphStore {
            outlines: outlines,
            glyph_id_to_glyph_index: glyph_id_to_glyph_index,
            all_glyph_indices: all_glyph_indices,
        })
    }

    pub fn from_codepoints(codepoints: &CodepointRanges, font: &Font)
                           -> Result<GlyphStore, GlyphStoreCreationError> {
        let mapping = try!(font.glyph_mapping_for_codepoint_ranges(&codepoints.ranges)
                               .map_err(GlyphStoreCreationError::OtfError));
        let glyph_ids = mapping.iter().map(|(_, glyph_id)| glyph_id).collect();
        GlyphStore::from_glyph_ids(glyph_ids, font)
    }

    #[inline]
    pub fn glyph_index(&self, glyph_id: u16) -> Option<u16> {
        match self.glyph_id_to_glyph_index.get(glyph_id as usize) {
            None | Some(&u16::MAX) => None,
            Some(&index) => Some(index),
        }
    }
}

