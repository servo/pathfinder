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

#[allow(unused_imports)]
#[macro_use]
extern crate log;

#[cfg(test)]
extern crate env_logger;

#[cfg(all(target_os = "macos", not(feature = "freetype")))]
extern crate core_graphics as core_graphics_sys;

#[cfg(all(target_os = "macos", not(feature = "freetype")))]
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

#[cfg(all(target_os = "macos", not(feature = "freetype")))]
pub use core_graphics::FontContext;
#[cfg(all(target_os = "windows", not(feature = "freetype")))]
pub use directwrite::FontContext;
#[cfg(any(target_os = "linux", feature = "freetype"))]
pub use freetype::FontContext;

#[cfg(all(target_os = "macos", not(feature = "freetype")))]
mod core_graphics;
#[cfg(all(target_os = "windows", not(feature = "freetype")))]
mod directwrite;
#[cfg(any(target_os = "linux", feature = "freetype"))]
mod freetype;

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

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord)]
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

// FIXME(pcwalton): Subpixel offsets?
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
