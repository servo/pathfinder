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

use crate::effects::PatternFilter;
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
    source: PatternSource,
    transform: Transform2F,
    filter: Option<PatternFilter>,
    flags: PatternFlags,
}

/// Where a raster image pattern comes from.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum PatternSource {
    /// A image whose pixels are stored in CPU memory.
    Image(Image),
    /// Previously-rendered vector content.
    ///
    /// This value allows you to render content and then later use that content as a pattern.
    RenderTarget {
        /// The ID of the render target, including the ID of the scene it came from.
        id: RenderTargetId,
        /// The device pixel size of the render target.
        size: Vector2I,
    }
}

/// A raster image, in 32-bit RGBA (8 bits per channel), non-premultiplied form.
// FIXME(pcwalton): Hash the pixel contents so that we don't have to compare every pixel!
// TODO(pcwalton): Should the pixels be premultiplied?
// TODO(pcwalton): Color spaces.
#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    size: Vector2I,
    pixels: Arc<Vec<ColorU>>,
    pixels_hash: u64,
    is_opaque: bool,
}

/// Unique identifier for an image.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ImageHash(pub u64);

bitflags! {
    /// Various flags that determine behavior of a pattern.
    pub struct PatternFlags: u8 {
        /// If set, the pattern repeats in the X direction. If unset, the base color is used.
        const REPEAT_X      = 0x01;
        /// If set, the pattern repeats in the Y direction. If unset, the base color is used.
        const REPEAT_Y      = 0x02;
        /// If set, nearest-neighbor interpolation is used when compositing this pattern (i.e. the
        /// image will be pixelated). If unset, bilinear interpolation is used when compositing
        /// this pattern (i.e. the image will be smooth).
        const NO_SMOOTHING  = 0x04;
    }
}

impl Pattern {
    #[inline]
    fn from_source(source: PatternSource) -> Pattern {
        Pattern {
            source,
            transform: Transform2F::default(),
            filter: None,
            flags: PatternFlags::empty(),
        }
    }

    /// Creates a new pattern from the given image.
    ///
    /// The transform is initialized to the identity transform. There is no filter.
    #[inline]
    pub fn from_image(image: Image) -> Pattern {
        Pattern::from_source(PatternSource::Image(image))
    }

    /// Creates a new pattern from the given render target with the given size.
    ///
    /// The transform is initialized to the identity transform. There is no filter.
    #[inline]
    pub fn from_render_target(id: RenderTargetId, size: Vector2I) -> Pattern {
        Pattern::from_source(PatternSource::RenderTarget { id, size })
    }

    /// Returns the affine transform applied to this pattern.
    #[inline]
    pub fn transform(&self) -> Transform2F {
        self.transform
    }

    /// Applies the given transform to this pattern.
    ///
    /// The transform is applied after any existing transform.
    #[inline]
    pub fn apply_transform(&mut self, transform: Transform2F) {
        self.transform = transform * self.transform;
    }

    /// Returns the underlying pixel size of this pattern, not taking transforms into account.
    #[inline]
    pub fn size(&self) -> Vector2I {
        match self.source {
            PatternSource::Image(ref image) => image.size(),
            PatternSource::RenderTarget { size, .. } => size,
        }
    }

    /// Returns the filter attached to this pattern, if any.
    #[inline]
    pub fn filter(&self) -> Option<PatternFilter> {
        self.filter
    }

    /// Applies a filter to this pattern, replacing any previous filter if any.
    #[inline]
    pub fn set_filter(&mut self, filter: Option<PatternFilter>) {
        self.filter = filter;
    }

    /// Returns true if this pattern repeats in the X direction or false if the base color will be
    /// used when sampling beyond the coordinates of the image.
    #[inline]
    pub fn repeat_x(&self) -> bool {
        self.flags.contains(PatternFlags::REPEAT_X)
    }

    /// Set to true if the pattern should repeat in the X direction or false if the base color
    /// should be used when sampling beyond the coordinates of the image.
    #[inline]
    pub fn set_repeat_x(&mut self, repeat_x: bool) {
        self.flags.set(PatternFlags::REPEAT_X, repeat_x);
    }

    /// Returns true if this pattern repeats in the Y direction or false if the base color will be
    /// used when sampling beyond the coordinates of the image.
    #[inline]
    pub fn repeat_y(&self) -> bool {
        self.flags.contains(PatternFlags::REPEAT_Y)
    }

    /// Set to true if the pattern should repeat in the Y direction or false if the base color
    /// should be used when sampling beyond the coordinates of the image.
    #[inline]
    pub fn set_repeat_y(&mut self, repeat_y: bool) {
        self.flags.set(PatternFlags::REPEAT_Y, repeat_y);
    }

    /// Returns true if this pattern should use bilinear interpolation (i.e. the image will be
    /// smooth) when scaled or false if this pattern should use nearest-neighbor interpolation
    /// (i.e. the image will be pixelated).
    #[inline]
    pub fn smoothing_enabled(&self) -> bool {
        !self.flags.contains(PatternFlags::NO_SMOOTHING)
    }

    /// Set to true if the pattern should use bilinear interpolation (i.e. should be smooth) when
    /// scaled or false if this pattern should use nearest-neighbor interpolation (i.e. should be
    /// pixelated).
    #[inline]
    pub fn set_smoothing_enabled(&mut self, enable: bool) {
        self.flags.set(PatternFlags::NO_SMOOTHING, !enable);
    }

    /// Returns true if this pattern is obviously fully opaque.
    ///
    /// This is a best-effort quick check, so it might return false even if the image is actually
    /// opaque.
    #[inline]
    pub fn is_opaque(&self) -> bool {
        self.source.is_opaque()
    }

    /// Returns the underlying source of the pattern.
    #[inline]
    pub fn source(&self) -> &PatternSource {
        &self.source
    }
}

impl Image {
    /// Creates a new image with the given device pixel size and pixel store, as 32-bit RGBA (8
    /// bits per channel), RGBA, linear color space, nonpremultiplied.
    #[inline]
    pub fn new(size: Vector2I, pixels: Arc<Vec<ColorU>>) -> Image {
        assert_eq!(size.x() as usize * size.y() as usize, pixels.len());
        let is_opaque = pixels.iter().all(|pixel| pixel.is_opaque());

        let mut pixels_hasher = DefaultHasher::new();
        pixels.hash(&mut pixels_hasher);
        let pixels_hash = pixels_hasher.finish();

        Image { size, pixels, pixels_hash, is_opaque }
    }

    /// A convenience function to create a new image with the given image from the `image` crate.
    #[cfg(feature = "pf-image")]
    pub fn from_image_buffer(image_buffer: RgbaImage) -> Image {
        let (width, height) = image_buffer.dimensions();
        let pixels = color::u8_vec_to_color_vec(image_buffer.into_raw());
        Image::new(vec2i(width as i32, height as i32), Arc::new(pixels))
    }

    /// Returns the device pixel size of the image.
    #[inline]
    pub fn size(&self) -> Vector2I {
        self.size
    }

    /// Returns the pixel buffer of this image as 32-bit RGBA (8 bits per channel), RGBA, linear
    /// color space, nonpremultiplied.
    #[inline]
    pub fn pixels(&self) -> &Arc<Vec<ColorU>> {
        &self.pixels
    }

    /// Returns true if this image is obviously opaque.
    ///
    /// This is a best-guess quick check, and as such it might return false even if the image is
    /// fully opaque.
    #[inline]
    pub fn is_opaque(&self) -> bool {
        self.is_opaque
    }

    /// Returns a non-cryptographic hash of the image, which should be globally unique.
    #[inline]
    pub fn get_hash(&self) -> ImageHash {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        ImageHash(hasher.finish())
    }
}

impl PatternSource {
    /// Returns true if this pattern is obviously opaque.
    ///
    /// This is a best-guess quick check, and as such it might return false even if the pattern is
    /// fully opaque.
    #[inline]
    pub fn is_opaque(&self) -> bool {
        match *self {
            PatternSource::Image(ref image) => image.is_opaque(),
            PatternSource::RenderTarget { .. } => {
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
