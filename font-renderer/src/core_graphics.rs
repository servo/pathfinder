// pathfinder/font-renderer/src/core_graphics.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Font loading using macOS Core Graphics/Quartz.

use core_graphics_sys::base::{kCGImageAlphaNoneSkipFirst, kCGBitmapByteOrder32Little};
use core_graphics_sys::color_space::CGColorSpace;
use core_graphics_sys::context::{CGContext, CGTextDrawingMode};
use core_graphics_sys::data_provider::CGDataProvider;
use core_graphics_sys::font::{CGFont, CGGlyph};
use core_graphics_sys::geometry::{CG_AFFINE_TRANSFORM_IDENTITY, CGPoint, CGRect};
use core_graphics_sys::geometry::{CGSize, CG_ZERO_POINT};
use core_graphics_sys::path::CGPathElementType;
use core_text::font::CTFont;
use core_text;
use euclid::{Point2D, Rect, Size2D, Vector2D};
use lyon_path::PathEvent;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::sync::Arc;
use std::vec::IntoIter;
use {FontInstance, FontKey, GlyphDimensions, GlyphImage, GlyphKey};

const CG_ZERO_RECT: CGRect = CGRect {
    origin: CG_ZERO_POINT,
    size: CGSize {
        width: 0.0,
        height: 0.0,
    },
};

// A conservative overestimate of the amount of font dilation that Core Graphics performs, as a
// fraction of ppem.
//
// The actual amount as of High Sierra is 0.0121 in the X direction and 0.015125 in the Y
// direction.
const FONT_DILATION_AMOUNT: f32 = 0.02;

/// A list of path commands.
pub type GlyphOutline = IntoIter<PathEvent>;

/// An object that loads and renders fonts using macOS Core Graphics/Quartz.
pub struct FontContext {
    core_graphics_fonts: BTreeMap<FontKey, CGFont>,
    core_text_fonts: BTreeMap<FontInstance, CTFont>,
}

impl FontContext {
    /// Creates a new font context instance.
    pub fn new() -> Result<FontContext, ()> {
        Ok(FontContext {
            core_graphics_fonts: BTreeMap::new(),
            core_text_fonts: BTreeMap::new(),
        })
    }

    /// Loads an OpenType font from memory.
    /// 
    /// `font_key` is a handle that is used to refer to the font later. If this context has already
    /// loaded a font with the same font key, nothing is done, and `Ok` is returned.
    /// 
    /// `bytes` is the raw OpenType data (i.e. the contents of the `.otf` or `.ttf` file on disk).
    /// 
    /// `font_index` is the index of the font within the collection, if `bytes` refers to a
    /// collection (`.ttc`).
    pub fn add_font_from_memory(&mut self, font_key: &FontKey, bytes: Arc<Vec<u8>>, _: u32)
                                -> Result<(), ()> {
        match self.core_graphics_fonts.entry(*font_key) {
            Entry::Occupied(_) => Ok(()),
            Entry::Vacant(entry) => {
                let data_provider = CGDataProvider::from_buffer(bytes);
                let core_graphics_font = try!(CGFont::from_data_provider(data_provider));
                entry.insert(core_graphics_font);
                Ok(())
            }
        }
    }

    /// Unloads the font with the given font key from memory.
    /// 
    /// If the font isn't loaded, does nothing.
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

    fn ensure_core_text_font(&mut self, font_instance: &FontInstance) -> Result<CTFont, ()> {
        match self.core_text_fonts.entry(*font_instance) {
            Entry::Occupied(entry) => Ok((*entry.get()).clone()),
            Entry::Vacant(entry) => {
                let core_graphics_font = match self.core_graphics_fonts
                                                   .get(&font_instance.font_key) {
                    None => return Err(()),
                    Some(core_graphics_font) => core_graphics_font,
                };

                let core_text_font = try!(font_instance.instantiate(&core_graphics_font));
                entry.insert(core_text_font.clone());
                Ok(core_text_font)
            }
        }
    }

    /// Returns the dimensions of the given glyph in the given font.
    /// 
    /// If `exact` is true, then the raw outline extents as specified by the font designer are
    /// returned. These may differ from the extents when rendered on screen, because some font
    /// libraries (including Pathfinder) apply modifications to the outlines: for example, to
    /// dilate them for easier reading. To retrieve extents that account for these modifications,
    /// set `exact` to false.
    pub fn glyph_dimensions(&self, font_instance: &FontInstance, glyph_key: &GlyphKey, exact: bool)
                            -> Result<GlyphDimensions, ()> {
        let core_graphics_font = match self.core_graphics_fonts.get(&font_instance.font_key) {
            None => return Err(()),
            Some(core_graphics_font) => core_graphics_font,
        };

        let glyph = glyph_key.glyph_index as CGGlyph;
        let mut bounding_boxes = [CG_ZERO_RECT];
        let mut advances = [0];
        if !core_graphics_font.get_glyph_b_boxes(&[glyph], &mut bounding_boxes) ||
                !core_graphics_font.get_glyph_advances(&[glyph], &mut advances) {
            return Err(())
        }

        // FIXME(pcwalton): Vertical subpixel offsets.
        let subpixel_offset = Point2D::new(glyph_key.subpixel_offset.into(), 0.0);

        // Round out to pixel boundaries.
        let units_per_em = core_graphics_font.get_units_per_em();
        let scale = font_instance.size.to_f64_px() / (units_per_em as f64);
        let bounding_box =
            Rect::new(Point2D::new(bounding_boxes[0].origin.x,
                                   bounding_boxes[0].origin.y),
                      Size2D::new(bounding_boxes[0].size.width,
                                  bounding_boxes[0].size.height)).scale(scale, scale);
        let mut lower_left = Point2D::new(bounding_box.origin.x.floor() as i32,
                                          bounding_box.origin.y.floor() as i32);
        let mut upper_right = Point2D::new((bounding_box.origin.x + bounding_box.size.width +
                                            subpixel_offset.x).ceil() as i32,
                                           (bounding_box.origin.y + bounding_box.size.height +
                                            subpixel_offset.y).ceil() as i32);

        // Core Graphics performs font dilation to expand the outlines a bit. As of High Sierra,
        // the values seem to be 1.21% in the X direction and 1.5125% in the Y direction. Make sure
        // that there's enough room to account for this. We round the values up to 2% to account
        // for the possibility that Apple might tweak this later.
        if !exact {
            let font_dilation_radius = (font_instance.size.to_f32_px() *
                                        FONT_DILATION_AMOUNT * 0.5).ceil() as i32;
            lower_left += Vector2D::new(-font_dilation_radius, -font_dilation_radius);
            upper_right += Vector2D::new(font_dilation_radius, font_dilation_radius);
        }

        Ok(GlyphDimensions {
            origin: lower_left,
            size: Size2D::new((upper_right.x - lower_left.x) as u32,
                              (upper_right.y - lower_left.y) as u32),
            advance: advances[0] as f32,
        })
    }

    /// Returns a list of path commands that represent the given glyph in the given font.
    pub fn glyph_outline(&mut self, font_instance: &FontInstance, glyph_key: &GlyphKey)
                         -> Result<GlyphOutline, ()> {
        let core_text_font = try!(self.ensure_core_text_font(font_instance));
        let path = try!(core_text_font.create_path_for_glyph(glyph_key.glyph_index as CGGlyph,
                                                             &CG_AFFINE_TRANSFORM_IDENTITY));

        let mut builder = vec![];
        path.apply(&|element| {
            let points = element.points();
            match element.element_type {
                CGPathElementType::MoveToPoint => {
                    builder.push(PathEvent::MoveTo(convert_point(&points[0])))
                }
                CGPathElementType::AddLineToPoint => {
                    builder.push(PathEvent::LineTo(convert_point(&points[0])))
                }
                CGPathElementType::AddQuadCurveToPoint => {
                    builder.push(PathEvent::QuadraticTo(convert_point(&points[0]),
                                                        convert_point(&points[1])))
                }
                CGPathElementType::AddCurveToPoint => {
                    builder.push(PathEvent::CubicTo(convert_point(&points[0]),
                                                    convert_point(&points[1]),
                                                    convert_point(&points[2])))
                }
                CGPathElementType::CloseSubpath => builder.push(PathEvent::Close),
            }
        });

        return Ok(builder.into_iter());

        fn convert_point(core_graphics_point: &CGPoint) -> Point2D<f32> {
            Point2D::new(core_graphics_point.x as f32, core_graphics_point.y as f32)
        }
    }

    /// Uses the native Core Graphics library to rasterize a glyph on CPU.
    /// 
    /// Pathfinder uses this for reference testing.
    /// 
    /// If `exact` is true, then the glyph image will have precisely the size specified by the font
    /// designer. Because some font libraries, such as Core Graphics, perform modifications to the
    /// glyph outlines, to ensure the entire outline fits it is best to pass false for `exact`.
    pub fn rasterize_glyph_with_native_rasterizer(&self,
                                                  font_instance: &FontInstance,
                                                  glyph_key: &GlyphKey,
                                                  exact: bool)
                                                  -> Result<GlyphImage, ()> {
        let core_graphics_font = match self.core_graphics_fonts.get(&font_instance.font_key) {
            None => return Err(()),
            Some(core_graphics_font) => core_graphics_font,
        };

        let dimensions = try!(self.glyph_dimensions(font_instance, glyph_key, exact));

        // TODO(pcwalton): Add support for non-subpixel render modes.
        let bitmap_context_flags = kCGBitmapByteOrder32Little | kCGImageAlphaNoneSkipFirst;

        let mut core_graphics_context =
            CGContext::create_bitmap_context(None,
                                             dimensions.size.width as usize,
                                             dimensions.size.height as usize,
                                             8,
                                             dimensions.size.width as usize * 4,
                                             &CGColorSpace::create_device_rgb(),
                                             bitmap_context_flags);

        // TODO(pcwalton): Add support for non-subpixel render modes.
        let (antialias, smooth, bg_color) = (true, true, 1.0);

        // Use subpixel positioning. But don't let Core Graphics quantize, because we do that
        // ourselves.
        core_graphics_context.set_allows_font_subpixel_positioning(true);
        core_graphics_context.set_should_subpixel_position_fonts(true);
        core_graphics_context.set_allows_font_subpixel_quantization(false);
        core_graphics_context.set_should_subpixel_quantize_fonts(false);

        // Set up antialiasing flags.
        core_graphics_context.set_allows_font_smoothing(smooth);
        core_graphics_context.set_should_smooth_fonts(smooth);
        core_graphics_context.set_allows_antialiasing(antialias);
        core_graphics_context.set_should_antialias(antialias);

        // Set up the background.
        core_graphics_context.set_rgb_fill_color(bg_color, bg_color, bg_color, bg_color);
        core_graphics_context.fill_rect(CGRect {
            origin: CG_ZERO_POINT,
            size: CGSize {
                width: dimensions.size.width as f64,
                height: dimensions.size.height as f64,
            },
        });

        // Set up the text color.
        core_graphics_context.set_rgb_fill_color(0.0, 0.0, 0.0, 1.0);
        core_graphics_context.set_text_drawing_mode(CGTextDrawingMode::CGTextFill);

        // Set up the font.
        core_graphics_context.set_font(core_graphics_font);
        core_graphics_context.set_font_size(font_instance.size.to_f64_px());

        // Compute the rasterization origin.
        // TODO(pcwalton): Vertical subpixel positioning.
        let subpixel_offset = Point2D::new(glyph_key.subpixel_offset.into(), 0.0);
        let origin = CGPoint {
            x: -dimensions.origin.x as f64 + subpixel_offset.x,
            y: -dimensions.origin.y as f64,
        };

        // Draw the glyph, and extract the pixels.
        core_graphics_context.show_glyphs_at_positions(&[glyph_key.glyph_index as CGGlyph],
                                                       &[origin]);
        let mut pixels = core_graphics_context.data().to_vec();

        // Swap BGRA to RGBA.
        for pixel in pixels.chunks_mut(4) {
            let (b, r) = (pixel[0], pixel[2]);
            pixel[0] = r;
            pixel[2] = b;
        }

        // Return the image.
        Ok(GlyphImage {
            dimensions: dimensions,
            pixels: pixels,
        })
    }
}

impl FontInstance {
    fn instantiate(&self, core_graphics_font: &CGFont) -> Result<CTFont, ()> {
        Ok(core_text::font::new_from_CGFont(core_graphics_font, self.size.to_f64_px()))
    }
}
