// pathfinder/font-renderer/src/core_graphics.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core_graphics_sys::data_provider::CGDataProvider;
use core_graphics_sys::font::{CGFont, CGGlyph};
use core_graphics_sys::geometry::{CG_AFFINE_TRANSFORM_IDENTITY, CGPoint, CGRect};
use core_graphics_sys::geometry::{CGSize, CG_ZERO_POINT};
use core_graphics_sys::path::{CGPath, CGPathElementType};
use core_text::font::CTFont;
use core_text;
use euclid::{Point2D, Size2D};
use pathfinder_path_utils::cubic::{CubicPathCommand, CubicPathCommandApproxStream};
use pathfinder_path_utils::PathCommand;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::sync::Arc;
use {FontInstanceKey, FontKey, GlyphDimensions, GlyphKey};

const CURVE_APPROX_ERROR_BOUND: f32 = 0.1;

pub type GlyphOutline = Vec<PathCommand>;

pub struct FontContext {
    core_graphics_fonts: BTreeMap<FontKey, CGFont>,
    core_text_fonts: BTreeMap<FontInstanceKey, CTFont>,
}

impl FontContext {
    pub fn new() -> Result<FontContext, ()> {
        Ok(FontContext {
            core_graphics_fonts: BTreeMap::new(),
            core_text_fonts: BTreeMap::new(),
        })
    }

    pub fn add_font_from_memory(&mut self, font_key: &FontKey, bytes: Arc<Vec<u8>>, _: u32)
                                -> Result<(), ()> {
        match self.core_graphics_fonts.entry(*font_key) {
            Entry::Occupied(_) => Ok(()),
            Entry::Vacant(entry) => {
                let data_provider = CGDataProvider::from_buffer(&**bytes);
                let core_graphics_font = try!(CGFont::from_data_provider(data_provider));
                entry.insert(core_graphics_font);
                Ok(())
            }
        }
    }

    pub fn delete_font(&mut self, font_key: &FontKey) {
        self.core_graphics_fonts.remove(font_key);

        let core_text_font_keys: Vec<_> = self.core_text_fonts  
                                              .keys()
                                              .filter(|key| key.font_key == *font_key)
                                              .cloned()
                                              .collect();
        for core_text_font_key in &core_text_font_keys {
            self.core_text_fonts.remove(core_text_font_key);
        }
    }

    fn ensure_core_text_font(&mut self, font_instance_key: &FontInstanceKey)
                             -> Result<CTFont, ()> {
        match self.core_text_fonts.entry(*font_instance_key) {
            Entry::Occupied(entry) => Ok((*entry.get()).clone()),
            Entry::Vacant(entry) => {
                let core_graphics_font = match self.core_graphics_fonts
                                                   .get(&font_instance_key.font_key) {
                    None => return Err(()),
                    Some(core_graphics_font) => core_graphics_font,
                };

                let core_text_font = try!(font_instance_key.instantiate(&core_graphics_font));
                entry.insert(core_text_font.clone());
                Ok(core_text_font)
            }
        }
    }

    pub fn glyph_dimensions(&self, font_instance: &FontInstanceKey, glyph_key: &GlyphKey)
                            -> Option<GlyphDimensions> {
        let core_graphics_font = match self.core_graphics_fonts.get(&font_instance.font_key) {
            None => return None,
            Some(core_graphics_font) => core_graphics_font,
        };

        let glyph = glyph_key.glyph_index as CGGlyph;
        let mut bounding_boxes = [CGRect::new(&CG_ZERO_POINT, &CGSize::new(0.0, 0.0))];
        let mut advances = [0];
        if !core_graphics_font.get_glyph_b_boxes(&[glyph], &mut bounding_boxes) ||
                !core_graphics_font.get_glyph_advances(&[glyph], &mut advances) {
            return None
        }

        Some(GlyphDimensions {
            origin: Point2D::new(bounding_boxes[0].origin.x.round() as i32,
                                 bounding_boxes[0].origin.y.round() as i32),
            size: Size2D::new(bounding_boxes[0].size.width.round() as u32,
                              bounding_boxes[0].size.height.round() as u32),
            advance: advances[0] as f32,
        })
    }

    pub fn glyph_outline(&mut self, font_instance: &FontInstanceKey, glyph_key: &GlyphKey)
                         -> Result<GlyphOutline, ()> {
        let core_text_font = try!(self.ensure_core_text_font(font_instance));
        let path = try!(core_text_font.create_path_for_glyph(glyph_key.glyph_index as CGGlyph,
                                                             &CG_AFFINE_TRANSFORM_IDENTITY));

        let mut commands = vec![];
        path.apply(&|element| {
            let points = element.points();
            commands.push(match element.element_type {
                CGPathElementType::MoveToPoint => {
                    CubicPathCommand::MoveTo(convert_point(&points[0]))
                }
                CGPathElementType::AddLineToPoint => {
                    CubicPathCommand::LineTo(convert_point(&points[0]))
                }
                CGPathElementType::AddQuadCurveToPoint => {
                    CubicPathCommand::QuadCurveTo(convert_point(&points[0]),
                                                  convert_point(&points[1]))
                }
                CGPathElementType::AddCurveToPoint => {
                    CubicPathCommand::CubicCurveTo(convert_point(&points[0]),
                                                   convert_point(&points[1]),
                                                   convert_point(&points[2]))
                }
                CGPathElementType::CloseSubpath => CubicPathCommand::ClosePath,
            });
        });

        let approx_stream = CubicPathCommandApproxStream::new(commands.into_iter(),
                                                              CURVE_APPROX_ERROR_BOUND);

        let approx_commands: Vec<_> = approx_stream.collect();

        return Ok(approx_commands);

        fn convert_point(core_graphics_point: &CGPoint) -> Point2D<f32> {
            Point2D::new(core_graphics_point.x as f32, core_graphics_point.y as f32)
        }
    }
}

impl FontInstanceKey {
    fn instantiate(&self, core_graphics_font: &CGFont) -> Result<CTFont, ()> {
        Ok(core_text::font::new_from_CGFont(core_graphics_font, self.size.to_f64_px()))
    }
}
