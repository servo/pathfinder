// pathfinder/font-renderer/src/freetype/mod.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Font loading using FreeType.

use euclid::{Point2D, Size2D, Vector2D};
use freetype_sys::{FT_Error};
use freetype_sys::freetype::{FT_BBox, FT_Bitmap, FT_Done_Face, FT_F26Dot6, FT_Face};
use freetype_sys::freetype::{FT_GlyphSlot, FT_Init_FreeType, FT_Int32, FT_Glyph_Format_};
use freetype_sys::freetype::{FT_LOAD_NO_HINTING, FT_Library, FT_Library_SetLcdFilter};
use freetype_sys::freetype::{FT_Load_Glyph, FT_Long, FT_New_Memory_Face, FT_Outline_Get_CBox};
use freetype_sys::freetype::{FT_Outline_Translate, FT_Render_Mode_, FT_LcdFilter_, FT_Render_Glyph};
use freetype_sys::freetype::{FT_Set_Char_Size, FT_UInt};
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::hash::Hash;
use std::mem;
use std::ptr;
use std::slice;
use std::sync::Arc;

use self::fixed::{FromFtF26Dot6, ToFtF26Dot6};
use self::outline::Outline;
use {FontInstance, GlyphDimensions, GlyphImage, GlyphKey};

mod fixed;
mod outline;

pub type GlyphOutline<'a> = Outline<'a>;

// Default to no hinting.
//
// TODO(pcwalton): Make this configurable.
const GLYPH_LOAD_FLAGS: FT_Int32 = FT_LOAD_NO_HINTING as i32;

const DPI: u32 = 72;

/// An object that loads and renders fonts using the FreeType library.
pub struct FontContext<FK> where FK: Clone + Hash + Eq + Ord {
    library: FT_Library,
    faces: BTreeMap<FK, Face>,
}

impl<FK> FontContext<FK> where FK: Clone + Hash + Eq + Ord {
    /// Creates a new font context instance.
    pub fn new() -> Result<FontContext<FK>, ()> {
        let mut library: FT_Library = ptr::null_mut();
        unsafe {
            let result = FT_Init_FreeType(&mut library);
            if result != FT_Error(0) {
                return Err(())
            }
        }
        Ok(FontContext {
            library: library,
            faces: BTreeMap::new(),
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
    pub fn add_font_from_memory(&mut self, font_key: &FK, bytes: Arc<Vec<u8>>, font_index: u32)
                                -> Result<(), ()> {
        match self.faces.entry((*font_key).clone()) {
            Entry::Occupied(_) => Ok(()),
            Entry::Vacant(entry) => {
                unsafe {
                    let mut face = Face {
                        face: ptr::null_mut(),
                        bytes: bytes,
                    };
                    let result = FT_New_Memory_Face(self.library,
                                                    face.bytes.as_ptr(),
                                                    face.bytes.len() as FT_Long,
                                                    font_index as FT_Long,
                                                    &mut face.face);
                    if result == FT_Error(0) && !face.face.is_null() {
                        entry.insert(face);
                        Ok(())
                    } else {
                        Err(())
                    }
                }
            }
        }
    }

    /// Unloads the font with the given font key from memory.
    ///
    /// If the font isn't loaded, does nothing.
    pub fn delete_font(&mut self, font_key: &FK) {
        self.faces.remove(font_key);
    }

    /// Returns the dimensions of the given glyph in the given font.
    ///
    /// If `exact` is true, then the raw outline extents as specified by the font designer are
    /// returned. These may differ from the extents when rendered on screen, because some font
    /// libraries (including Pathfinder) apply modifications to the outlines: for example, to
    /// dilate them for easier reading. To retrieve extents that account for these modifications,
    /// set `exact` to false.
    pub fn glyph_dimensions(&self, font_instance: &FontInstance<FK>, glyph_key: &GlyphKey)
                            -> Result<GlyphDimensions, ()> {
        self.load_glyph(font_instance, glyph_key).ok_or(()).and_then(|glyph_slot| {
            self.glyph_dimensions_from_slot(font_instance, glyph_key, glyph_slot)
        })
    }

    pub fn glyph_outline<'a>(&'a mut self, font_instance: &FontInstance<FK>, glyph_key: &GlyphKey)
                             -> Result<GlyphOutline<'a>, ()> {
        self.load_glyph(font_instance, glyph_key).ok_or(()).map(|glyph_slot| {
            unsafe {
                GlyphOutline::new(&(*glyph_slot).outline)
            }
        })
    }

    /// Uses the FreeType library to rasterize a glyph on CPU.
    ///
    /// Pathfinder uses this for reference testing.
    ///
    /// If `exact` is true, then the glyph image will have precisely the size specified by the font
    /// designer. Because some font libraries, such as Core Graphics, perform modifications to the
    /// glyph outlines, to ensure the entire outline fits it is best to pass false for `exact`.
    pub fn rasterize_glyph_with_native_rasterizer(&self,
                                                  font_instance: &FontInstance<FK>,
                                                  glyph_key: &GlyphKey,
                                                  _: bool)
                                                  -> Result<GlyphImage, ()> {
        // Load the glyph.
        let slot = match self.load_glyph(font_instance, glyph_key) {
            None => return Err(()),
            Some(slot) => slot,
        };

        // Get the subpixel offset.
        let subpixel_offset: Vector2D<FT_F26Dot6> =
            Vector2D::new(f32::to_ft_f26dot6(glyph_key.subpixel_offset.into()), 0);

        // Move the outline curves to be at the origin, taking the subpixel positioning into
        // account.
        unsafe {
            let outline = &(*slot).outline;
            let mut control_box: FT_BBox = mem::uninitialized();
            FT_Outline_Get_CBox(outline, &mut control_box);
            FT_Outline_Translate(
                outline,
                subpixel_offset.x - fixed::floor(control_box.xMin + subpixel_offset.x),
                subpixel_offset.y - fixed::floor(control_box.yMin + subpixel_offset.y));
        }

        // Set the LCD filter.
        //
        // TODO(pcwalton): Non-subpixel AA.
        unsafe {
            FT_Library_SetLcdFilter(self.library, FT_LcdFilter_::FT_LCD_FILTER_DEFAULT);
        }

        // Render the glyph.
        //
        // TODO(pcwalton): Non-subpixel AA.
        unsafe {
            FT_Render_Glyph(slot, FT_Render_Mode_::FT_RENDER_MODE_LCD);
        }

        unsafe {
            // Make sure that the pixel mode is LCD.
            //
            // TODO(pcwalton): Non-subpixel AA.
            let bitmap: *const FT_Bitmap = &(*slot).bitmap;
            if (*bitmap).pixel_mode != 5 {
                return Err(())
            }

            debug_assert_eq!((*bitmap).width % 3, 0);
            let pixel_size = Size2D::new((*bitmap).width as u32 / 3, (*bitmap).rows as u32);
            let pixel_origin = Point2D::new((*slot).bitmap_left, (*slot).bitmap_top);

            // Allocate the RGBA8 buffer.
            let src_stride = (*bitmap).pitch as usize;
            let dest_stride = pixel_size.width as usize;
            let src_area = src_stride * ((*bitmap).rows as usize);
            let dest_area = pixel_size.area() as usize;
            let mut dest_pixels: Vec<u32> = vec![0; dest_area];
            let src_pixels = slice::from_raw_parts((*bitmap).buffer, src_area);

            // Convert to RGBA8.
            for y in 0..(pixel_size.height as usize) {
                let dest_row = &mut dest_pixels[(y * dest_stride)..((y + 1) * dest_stride)];
                let src_row = &src_pixels[(y * src_stride)..((y + 1) * src_stride)];
                for (x, dest) in dest_row.iter_mut().enumerate() {
                    *dest = ((255 - src_row[x * 3 + 2]) as u32) |
                        (((255 - src_row[x * 3 + 1]) as u32) << 8) |
                        (((255 - src_row[x * 3 + 0]) as u32) << 16) |
                        (0xff << 24)
                }
            }

            // Return the result.
            Ok(GlyphImage {
                dimensions: GlyphDimensions {
                    origin: pixel_origin,
                    size: pixel_size,
                    advance: f32::from_ft_f26dot6((*slot).metrics.horiAdvance),
                },
                pixels: convert_vec_u32_to_vec_u8(dest_pixels),
            })
        }
    }

    fn load_glyph(&self, font_instance: &FontInstance<FK>, glyph_key: &GlyphKey)
                  -> Option<FT_GlyphSlot> {
        let face = match self.faces.get(&font_instance.font_key) {
            None => return None,
            Some(face) => face,
        };

        unsafe {
            let point_size = font_instance.size.to_ft_f26dot6();
            FT_Set_Char_Size(face.face, point_size, 0, DPI, 0);

            if FT_Load_Glyph(face.face, glyph_key.glyph_index as FT_UInt, GLYPH_LOAD_FLAGS) != FT_Error(0) {
                return None
            }

            let slot = (*face.face).glyph;
            if (*slot).format != FT_Glyph_Format_::FT_GLYPH_FORMAT_OUTLINE {
                return None
            }

            Some(slot)
        }
    }

    fn glyph_dimensions_from_slot(&self,
                                  font_instance: &FontInstance<FK>,
                                  glyph_key: &GlyphKey,
                                  glyph_slot: FT_GlyphSlot)
                                  -> Result<GlyphDimensions, ()> {
        unsafe {
            let metrics = &(*glyph_slot).metrics;

            // This matches what WebRender does.
            if metrics.horiAdvance == 0 {
                return Err(())
            }

            let bounding_box = self.bounding_box_from_slot(font_instance, glyph_key, glyph_slot);
            Ok(GlyphDimensions {
                origin: Point2D::new((bounding_box.xMin >> 6) as i32,
                                     (bounding_box.yMax >> 6) as i32),
                size: Size2D::new(((bounding_box.xMax - bounding_box.xMin) >> 6) as u32,
                                  ((bounding_box.yMax - bounding_box.yMin) >> 6) as u32),
                advance: f32::from_ft_f26dot6(metrics.horiAdvance),
            })
        }
    }

    // Returns the bounding box for a glyph, accounting for subpixel positioning as appropriate.
    //
    // TODO(pcwalton): Subpixel positioning.
    fn bounding_box_from_slot(&self, _: &FontInstance<FK>, _: &GlyphKey, glyph_slot: FT_GlyphSlot)
                              -> FT_BBox {
        let mut bounding_box: FT_BBox;
        unsafe {
            bounding_box = mem::zeroed();
            FT_Outline_Get_CBox(&(*glyph_slot).outline, &mut bounding_box);
        };

        // Outset the box to device pixel boundaries. This matches what WebRender does.
        bounding_box.xMin = fixed::floor(bounding_box.xMin);
        bounding_box.yMin = fixed::floor(bounding_box.yMin);
        bounding_box.xMax = fixed::floor(bounding_box.xMax + 0x3f);
        bounding_box.yMax = fixed::floor(bounding_box.yMax + 0x3f);

        bounding_box
    }
}

struct Face {
    face: FT_Face,
    bytes: Arc<Vec<u8>>,
}

impl Drop for Face {
    fn drop(&mut self) {
        unsafe {
            FT_Done_Face(self.face);
        }
    }
}

unsafe fn convert_vec_u32_to_vec_u8(mut input: Vec<u32>) -> Vec<u8> {
    let (ptr, len, cap) = (input.as_mut_ptr(), input.len(), input.capacity());
    mem::forget(input);
    Vec::from_raw_parts(ptr as *mut u8, len * 4, cap * 4)
}
