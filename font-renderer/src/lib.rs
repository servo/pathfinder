// pathfinder/font-renderer/src/lib.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate app_units;
extern crate euclid;
extern crate libc;
extern crate pathfinder_path_utils;
extern crate serde;

#[allow(unused_imports)]
#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;

#[cfg(test)]
extern crate env_logger;

#[cfg(target_os = "macos")]
extern crate core_graphics as core_graphics_sys;

#[cfg(target_os = "macos")]
extern crate core_text;

#[cfg(any(target_os = "linux", feature = "freetype"))]
extern crate freetype_sys;

#[cfg(target_os = "windows")]
extern crate dwrite;
#[cfg(target_os = "windows")]
extern crate kernel32;
#[cfg(target_os = "windows")]
extern crate uuid;
#[cfg(target_os = "windows")]
#[macro_use(DEFINE_GUID)]
extern crate winapi;

use app_units::Au;
use euclid::{Point2D, Size2D};
use std::sync::atomic::{ATOMIC_USIZE_INIT, AtomicUsize, Ordering};

#[cfg(test)]
mod tests;

#[cfg(all(target_os = "macos", not(feature = "freetype")))]
pub use core_graphics::FontContext;
#[cfg(all(target_os = "windows", not(feature = "freetype")))]
pub use directwrite::FontContext;
#[cfg(any(target_os = "linux", feature = "freetype"))]
pub use freetype::FontContext;

#[cfg(target_os = "macos")]
pub mod core_graphics;
#[cfg(all(target_os = "windows", not(feature = "freetype")))]
mod directwrite;
#[cfg(any(target_os = "linux", feature = "freetype"))]
mod freetype;

const SUBPIXEL_GRANULARITY: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
pub struct FontInstanceKey {
    id: usize,
}

impl FontInstanceKey {
    #[inline]
    pub fn new() -> FontInstanceKey {
        static NEXT_FONT_INSTANCE_KEY_ID: AtomicUsize = ATOMIC_USIZE_INIT;
        FontInstanceKey {
            id: NEXT_FONT_INSTANCE_KEY_ID.fetch_add(1, Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
pub struct FontInstance {
    pub font_key: FontKey,

    // The font size is in *device* pixels, not logical pixels.
    // It is stored as an Au since we need sub-pixel sizes, but
    // we can't store an f32 due to use of this type as a hash key.
    // TODO(gw): Perhaps consider having LogicalAu and DeviceAu
    //           or something similar to that.
    pub size: Au,
}

impl FontInstance {
    #[inline]
    pub fn new(font_key: &FontKey, size: Au) -> FontInstance {
        FontInstance {
            font_key: *font_key,
            size: size,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GlyphKey {
    pub glyph_index: u32,
    pub subpixel_offset: SubpixelOffset,
}

impl GlyphKey {
    #[inline]
    pub fn new(glyph_index: u32, subpixel_offset: SubpixelOffset) -> GlyphKey {
        GlyphKey {
            glyph_index: glyph_index,
            subpixel_offset: subpixel_offset,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlyphDimensions {
    pub origin: Point2D<i32>,
    pub size: Size2D<u32>,
    pub advance: f32,
}

pub struct GlyphImage {
    pub dimensions: GlyphDimensions,
    pub pixels: Vec<u8>,
}
