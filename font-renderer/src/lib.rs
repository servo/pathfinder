// pathfinder/font-renderer/lib.rs

extern crate app_units;
extern crate euclid;
extern crate freetype_sys;
extern crate pathfinder_path_utils;

#[allow(unused_imports)]
#[macro_use]
extern crate log;

#[cfg(test)]
extern crate env_logger;

use app_units::Au;
use euclid::{Point2D, Size2D};
use freetype_sys::{FT_BBox, FT_Done_Face, FT_F26Dot6, FT_Face, FT_GLYPH_FORMAT_OUTLINE};
use freetype_sys::{FT_GlyphSlot, FT_Init_FreeType, FT_Int32, FT_LOAD_NO_HINTING, FT_Library};
use freetype_sys::{FT_Load_Glyph, FT_Long, FT_New_Memory_Face, FT_Outline_Get_CBox};
use freetype_sys::{FT_Set_Char_Size, FT_UInt};
use pathfinder_path_utils::PathSegment;
use pathfinder_path_utils::freetype::OutlineStream;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::marker::PhantomData;
use std::mem;
use std::ptr;
use std::sync::atomic::{ATOMIC_USIZE_INIT, AtomicUsize, Ordering};

#[cfg(test)]
mod tests;

// Default to no hinting.
//
// TODO(pcwalton): Make this configurable.
const GLYPH_LOAD_FLAGS: FT_Int32 = FT_LOAD_NO_HINTING;

const DPI: u32 = 72;

pub struct FontContext {
    library: FT_Library,
    faces: BTreeMap<FontKey, Face>,
}

impl FontContext {
    pub fn new() -> FontContext {
        let mut library: FT_Library = ptr::null_mut();
        unsafe {
            let result = FT_Init_FreeType(&mut library);
            assert!(result == 0, "Unable to initialize FreeType");
        }
        FontContext {
            library: library,
            faces: BTreeMap::new(),
        }
    }

    pub fn add_font_from_memory(&mut self, font_key: &FontKey, bytes: Vec<u8>, font_index: u32)
                                -> Result<(), ()> {
        match self.faces.entry(*font_key) {
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
                    if result == 0 && !face.face.is_null() {
                        entry.insert(face);
                        Ok(())
                    } else {
                        Err(())
                    }
                }
            }
        }
    }

    pub fn delete_font(&mut self, font_key: &FontKey) {
        self.faces.remove(font_key);
    }

    pub fn glyph_dimensions(&self, font_instance: &FontInstanceKey, glyph_key: &GlyphKey)
                            -> Option<GlyphDimensions> {
        self.load_glyph(font_instance, glyph_key).and_then(|glyph_slot| {
            self.glyph_dimensions_from_slot(font_instance, glyph_key, glyph_slot)
        })
    }

    pub fn glyph_outline<'a>(&'a mut self, font_instance: &FontInstanceKey, glyph_key: &GlyphKey)
                             -> Result<GlyphOutline<'a>, ()> {
        self.load_glyph(font_instance, glyph_key).ok_or(()).map(|glyph_slot| {
            unsafe {
                GlyphOutline {
                    stream: OutlineStream::new(&(*glyph_slot).outline),
                    phantom: PhantomData,
                }
            }
        })
    }

/*
    pub fn push_glyph_outline(&self,
                              font_instance: &FontInstanceKey,
                              glyph_key: &GlyphKey,
                              path_buffer: &mut PathBuffer)
                              -> Result<(), ()> {
        self.load_glyph(font_instance, glyph_key).ok_or(()).map(|glyph_slot| {
            unsafe {
                let outline = &(*glyph_slot).outline;
                for contour_index in 0..outline.n_contours as u16 {
                    path_buffer.add_subpath_from_stream(OutlineIter::new(outline, contour_index))
                }
            }
        })
    }*/

    fn load_glyph(&self, font_instance: &FontInstanceKey, glyph_key: &GlyphKey)
                  -> Option<FT_GlyphSlot> {
        let face = match self.faces.get(&font_instance.font_key) {
            None => return None,
            Some(face) => face,
        };

        unsafe {
            let point_size = (font_instance.size.to_f64_px() / (DPI as f64)).to_ft_f26dot6();
            FT_Set_Char_Size(face.face, point_size, 0, DPI, 0);

            if FT_Load_Glyph(face.face, glyph_key.glyph_index as FT_UInt, GLYPH_LOAD_FLAGS) != 0 {
                return None
            }

            let slot = (*face.face).glyph;
            if (*slot).format != FT_GLYPH_FORMAT_OUTLINE {
                return None
            }

            Some(slot)
        }
    }

    fn glyph_dimensions_from_slot(&self,
                                  font_instance: &FontInstanceKey,
                                  glyph_key: &GlyphKey,
                                  glyph_slot: FT_GlyphSlot)
                                  -> Option<GlyphDimensions> {
        unsafe {
            let metrics = &(*glyph_slot).metrics;

            // This matches what WebRender does.
            if metrics.horiAdvance == 0 {
                return None
            }

            let bounding_box = self.bounding_box_from_slot(font_instance, glyph_key, glyph_slot);
            Some(GlyphDimensions {
                origin: Point2D::new((bounding_box.xMin >> 6) as i32,
                                     (bounding_box.yMax >> 6) as i32),
                size: Size2D::new(((bounding_box.xMax - bounding_box.xMin) >> 6) as u32,
                                  ((bounding_box.yMax - bounding_box.yMin) >> 6) as u32),
                advance: metrics.horiAdvance as f32 / 64.0,
            })
        }
    }

    // Returns the bounding box for a glyph, accounting for subpixel positioning as appropriate.
    //
    // TODO(pcwalton): Subpixel positioning.
    fn bounding_box_from_slot(&self, _: &FontInstanceKey, _: &GlyphKey, glyph_slot: FT_GlyphSlot)
                              -> FT_BBox {
        let mut bounding_box: FT_BBox;
        unsafe {
            bounding_box = mem::zeroed();
            FT_Outline_Get_CBox(&(*glyph_slot).outline, &mut bounding_box);
        };

        // Outset the box to device pixel boundaries. This matches what WebRender does.
        bounding_box.xMin &= !0x3f;
        bounding_box.yMin &= !0x3f;
        bounding_box.xMax = (bounding_box.xMax + 0x3f) & !0x3f;
        bounding_box.yMax = (bounding_box.yMax + 0x3f) & !0x3f;

        bounding_box
    }
}

pub struct GlyphOutline<'a> {
    stream: OutlineStream<'static>,
    phantom: PhantomData<&'a ()>,
}

impl<'a> Iterator for GlyphOutline<'a> {
    type Item = PathSegment;
    fn next(&mut self) -> Option<PathSegment> {
        self.stream.next()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct FontKey {
    id: usize,
}

impl FontKey {
    pub fn new() -> FontKey {
        static NEXT_FONT_KEY_ID: AtomicUsize = ATOMIC_USIZE_INIT;
        FontKey {
            id: NEXT_FONT_KEY_ID.fetch_add(1, Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontInstanceKey {
    pub font_key: FontKey,
    pub size: Au,
}

impl FontInstanceKey {
    #[inline]
    pub fn new(font_key: &FontKey, size: Au) -> FontInstanceKey {
        FontInstanceKey {
            font_key: *font_key,
            size: size,
        }
    }
}

// TODO(pcwalton): Subpixel offsets.
#[derive(Clone, Copy, PartialEq)]
pub struct GlyphKey {
    pub glyph_index: u32,
}

impl GlyphKey {
    #[inline]
    pub fn new(glyph_index: u32) -> GlyphKey {
        GlyphKey {
            glyph_index: glyph_index,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphDimensions {
    pub origin: Point2D<i32>,
    pub size: Size2D<u32>,
    pub advance: f32,
}

struct Face {
    face: FT_Face,
    bytes: Vec<u8>,
}

impl Drop for Face {
    fn drop(&mut self) {
        unsafe {
            FT_Done_Face(self.face);
        }
    }
}

trait FromFtF26Dot6 {
    fn from_ft_f26dot6(value: FT_F26Dot6) -> Self;
}

impl FromFtF26Dot6 for f32 {
    fn from_ft_f26dot6(value: FT_F26Dot6) -> f32 {
        (value as f32) / 64.0
    }
}

trait ToFtF26Dot6 {
    fn to_ft_f26dot6(&self) -> FT_F26Dot6;
}

impl ToFtF26Dot6 for f64 {
    fn to_ft_f26dot6(&self) -> FT_F26Dot6 {
        (*self * 64.0 + 0.5) as FT_F26Dot6
    }
}

impl ToFtF26Dot6 for Au {
    fn to_ft_f26dot6(&self) -> FT_F26Dot6 {
        self.to_f64_px().to_ft_f26dot6()
    }
}
