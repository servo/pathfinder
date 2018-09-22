// pathfinder/font-renderer/src/lib.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Reads outlines from OpenType fonts into Pathfinder path formats.
//! 
//! Use this crate in conjunction with `pathfinder_partitioner` in order to create meshes for
//! rendering.
//! 
//! To reduce dependencies and to match the system as closely as possible, this crate uses the
//! native OS font rendering infrastructure as much as it can. Backends are available for FreeType,
//! Core Graphics/Quartz on macOS, and DirectWrite on Windows.

extern crate app_units;
extern crate euclid;
extern crate libc;
extern crate lyon_path;
extern crate serde;

#[allow(unused_imports)]
#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;

#[cfg(test)]
extern crate env_logger;

#[cfg(any(target_os = "macos", target_os = "ios"))]
extern crate core_graphics as core_graphics_sys;

#[cfg(any(target_os = "macos", target_os = "ios"))]
extern crate core_text;

#[cfg(any(target_os = "linux", feature = "freetype-backend"))]
extern crate freetype as freetype_sys;

#[cfg(target_os = "windows")]
#[macro_use(DEFINE_GUID)]
extern crate winapi;

#[cfg(target_os = "windows")]
pub use self::directwrite::PathfinderComPtr;
#[cfg(target_os = "windows")]
pub use winapi::um::dwrite::IDWriteFontFace;

use app_units::Au;
use euclid::{Point2D, Size2D};

#[cfg(test)]
mod tests;

#[cfg(all(any(target_os = "macos", target_os = "ios"), not(feature = "freetype-backend")))]
pub use core_graphics::{FontContext, GlyphOutline};
#[cfg(all(target_os = "windows", not(feature = "freetype-backend")))]
pub use directwrite::FontContext;
#[cfg(any(target_os = "linux", feature = "freetype-backend"))]
pub use freetype::FontContext;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod core_graphics;
#[cfg(all(target_os = "windows", not(feature = "freetype-backend")))]
mod directwrite;
#[cfg(any(target_os = "linux", feature = "freetype-backend"))]
pub mod freetype;

/// The number of subpixels that each pixel is divided into for the purposes of subpixel glyph
/// positioning.
/// 
/// Right now, each glyph is snapped to the nearest quarter-pixel.
pub const SUBPIXEL_GRANULARITY: u8 = 4;

/// A font at one specific size.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
pub struct FontInstance<FK> where FK: Clone {
    /// The opaque font key that this font instance represents.
    pub font_key: FK,

    /// The size of the font.
    /// 
    /// This is in app units (1/60 pixels) to eliminate floating point error.
    pub size: Au,
}

impl<FK> FontInstance<FK> where FK: Clone {
    /// Creates a new instance of a font at the given size.
    #[inline]
    pub fn new(font_key: &FK, size: Au) -> FontInstance<FK> {
        FontInstance {
            font_key: (*font_key).clone(),
            size: size,
        }
    }
}

/// A subpixel offset, from 0 to `SUBPIXEL_GRANULARITY`.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct SubpixelOffset(pub u8);

impl Into<f32> for SubpixelOffset {
    #[inline]
    fn into(self) -> f32 {
        self.0 as f32 / SUBPIXEL_GRANULARITY as f32
    }
}

impl Into<f64> for SubpixelOffset {
    #[inline]
    fn into(self) -> f64 {
        self.0 as f64 / SUBPIXEL_GRANULARITY as f64
    }
}

/// A handle to the resolution-independent image of a single glyph in a single font.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GlyphKey {
    /// The OpenType glyph index.
    pub glyph_index: u32,
    /// The subpixel offset, from 0 to `SUBPIXEL_GRANULARITY`.
    pub subpixel_offset: SubpixelOffset,
}

impl GlyphKey {
    /// Creates a new glyph key from the given index and subpixel offset.
    #[inline]
    pub fn new(glyph_index: u32, subpixel_offset: SubpixelOffset) -> GlyphKey {
        GlyphKey {
            glyph_index: glyph_index,
            subpixel_offset: subpixel_offset,
        }
    }
}

/// The resolution-independent dimensions of a glyph, in font units.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlyphDimensions {
    /// The origin of the glyph.
    pub origin: Point2D<i32>,
    /// The total size of the glyph.
    pub size: Size2D<u32>,
    /// The advance of the glyph: that is, the distance from this glyph to the next one.
    pub advance: f32,
}

/// A bitmap image of a glyph.
pub struct GlyphImage {
    /// The dimensions of this image.
    pub dimensions: GlyphDimensions,
    /// The actual pixels.
    /// 
    /// This is 8 bits per pixel grayscale when grayscale antialiasing is in use and 24 bits per
    /// pixel RGB when subpixel antialiasing is in use.
    pub pixels: Vec<u8>,
}
