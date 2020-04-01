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

use crate::render_target::RenderTargetId;
use crate::util;
use pathfinder_color::{self as color, ColorU};
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::{Vector2I, vec2i};
use std::collections::hash_map::DefaultHasher;
use std::fmt::{self, Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[cfg(feature = "pf-image")]
use image::RgbaImage;

/// A raster image pattern.
#[derive(Clone, PartialEq, Debug)]
pub struct Pattern {
    pub source: PatternSource,
    pub transform: Transform2F,
    pub flags: PatternFlags,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum PatternSource {
    Image(Image),
    RenderTarget(RenderTargetId),
}

/// RGBA, non-premultiplied.
// FIXME(pcwalton): Hash the pixel contents so that we don't have to compare every pixel!
// TODO(pcwalton): Should the pixels be premultiplied?
#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    size: Vector2I,
    pixels: Arc<Vec<ColorU>>,
    pixels_hash: u64,
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
    pub fn new(source: PatternSource, transform: Transform2F, flags: PatternFlags) -> Pattern {
        Pattern { source, transform, flags }
    }
}

impl Image {
    #[inline]
    pub fn new(size: Vector2I, pixels: Arc<Vec<ColorU>>) -> Image {
        assert_eq!(size.x() as usize * size.y() as usize, pixels.len());
        let is_opaque = pixels.iter().all(|pixel| pixel.is_opaque());

        let mut pixels_hasher = DefaultHasher::new();
        pixels.hash(&mut pixels_hasher);
        let pixels_hash = pixels_hasher.finish();

        Image { size, pixels, pixels_hash, is_opaque }
    }

    #[cfg(feature = "pf-image")]
    pub fn from_image_buffer(image_buffer: RgbaImage) -> Image {
        let (width, height) = image_buffer.dimensions();
        let pixels = color::u8_vec_to_color_vec(image_buffer.into_raw());
        Image::new(vec2i(width as i32, height as i32), Arc::new(pixels))
    }

    #[inline]
    pub fn size(&self) -> Vector2I {
        self.size
    }

    #[inline]
    pub fn pixels(&self) -> &Arc<Vec<ColorU>> {
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

impl Hash for Image {
    fn hash<H>(&self, hasher: &mut H) where H: Hasher {
        self.size.hash(hasher);
        self.pixels_hash.hash(hasher);
        self.is_opaque.hash(hasher);
    }
}

impl Eq for Pattern {}

impl Hash for Pattern {
    fn hash<H>(&self, state: &mut H) where H: Hasher {
        self.source.hash(state);
        util::hash_transform2f(self.transform, state);
        self.flags.hash(state);
    }
}
