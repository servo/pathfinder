// pathfinder/content/src/pattern.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Raster image patterns.

use pathfinder_color::{self as color, ColorU};
use pathfinder_geometry::vector::Vector2I;
use std::fmt::{self, Debug, Formatter};

#[cfg(feature = "pf-image")]
use image::RgbaImage;

/// A raster image pattern.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Pattern {
    pub source: PatternSource,
    pub flags: PatternFlags,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum PatternSource {
    Image(Image),
    RenderTarget(RenderTargetId),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RenderTargetId(pub u32);

/// RGBA, non-premultiplied.
// FIXME(pcwalton): Hash the pixel contents so that we don't have to compare every pixel!
// TODO(pcwalton): Should the pixels be premultiplied?
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Image {
    size: Vector2I,
    pixels: Vec<ColorU>,
    is_opaque: bool,
}

bitflags! {
    pub struct PatternFlags: u8 {
        const REPEAT_X      = 0x01;
        const REPEAT_Y      = 0x02;
        const NO_SMOOTHING  = 0x04;
    }
}

impl Pattern {
    #[inline]
    pub fn new(source: PatternSource, flags: PatternFlags) -> Pattern {
        Pattern { source, flags }
    }
}

impl Image {
    #[inline]
    pub fn new(size: Vector2I, pixels: Vec<ColorU>) -> Image {
        assert_eq!(size.x() as usize * size.y() as usize, pixels.len());
        let is_opaque = pixels.iter().all(|pixel| pixel.is_opaque());
        Image { size, pixels, is_opaque }
    }

    #[cfg(feature = "pf-image")]
    pub fn from_image_buffer(image_buffer: RgbaImage) -> Image {
        let (width, height) = image_buffer.dimensions();
        let pixels = color::u8_vec_to_color_vec(image_buffer.into_raw());
        Image::new(Vector2I::new(width as i32, height as i32), pixels)
    }

    #[inline]
    pub fn size(&self) -> Vector2I {
        self.size
    }

    #[inline]
    pub fn pixels(&self) -> &[ColorU] {
        &self.pixels
    }

    #[inline]
    pub fn is_opaque(&self) -> bool {
        self.is_opaque
    }
}

impl PatternSource {
    #[inline]
    pub fn is_opaque(&self) -> bool {
        match *self {
            PatternSource::Image(ref image) => image.is_opaque(),
            PatternSource::RenderTarget(_) => {
                // TODO(pcwalton): Maybe do something smarter here?
                false
            }
        }
    }
}

impl Debug for Image {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "(image {}×{} px)", self.size.x(), self.size.y())
    }
}
