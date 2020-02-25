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
    pub repeat: Repeat,
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
    pub struct Repeat: u8 {
        const X = 0x01;
        const Y = 0x02;
    }
}

impl Pattern {
    #[inline]
    pub fn new(source: PatternSource, repeat: Repeat) -> Pattern {
        Pattern { source, repeat }
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

    pub fn set_opacity(&mut self, alpha: f32) {
        debug_assert!(alpha >= 0.0 && alpha <= 1.0);
        if alpha == 1.0 {
            return;
        }

        // TODO(pcwalton): Go four pixels at a time with SIMD.
        self.pixels.iter_mut().for_each(|pixel| pixel.a = (pixel.a as f32 * alpha).round() as u8);
        self.is_opaque = false;
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

    #[inline]
    pub fn set_opacity(&mut self, alpha: f32) {
        match *self {
            PatternSource::Image(ref mut image) => image.set_opacity(alpha),
            PatternSource::RenderTarget(_) => {
                // TODO(pcwalton): We'll probably have to introduce and use an Opacity filter for
                // this.
                unimplemented!()
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
