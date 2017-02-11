// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Atlases, which hold rendered glyphs on the GPU.

use error::GlError;
use euclid::{Point2D, Rect, Size2D};
use gl::types::{GLenum, GLsizei, GLsizeiptr, GLuint, GLvoid};
use gl;
use outline::Outlines;
use rect_packer::RectPacker;
use std::mem;
use std::os::raw::c_void;
use std::u16;

/// Places glyphs in an atlas.
///
/// Atlases are composed of vertically-stacked "shelves" of uniform height. No glyphs may cross
/// shelves. Therefore, the shelf height must be tall enough to encompass all of the glyphs you
/// wish to render into the atlas.
///
/// Typically, when using Pathfinder, you first create an atlas builder, place all the glyphs into
/// it, generate the atlas, and then pass that glyph to a rasterizer for rendering on the GPU.
/// Afterward, you can retrieve the positions of each glyph in the atlas for final composition to
/// the screen.
pub struct AtlasBuilder {
    rect_packer: RectPacker,
    batch_builders: Vec<BatchBuilder>,
}

impl AtlasBuilder {
    /// Constructs a new atlas builder with the given width in pixels and shelf height.
    ///
    /// The width can be any value at least as large as all glyphs in the font. It is recommended
    /// to keep it fairly large in order to make efficient use of the space: 1024 or 2048 is a good
    /// choice on modern GPUs.
    ///
    /// The shelf height should be the maximum of all minimum shelf heights for all fonts you wish
    /// to render into the atlas. You can retrive the minimum shelf height for a font with the
    /// `Font::shelf_height()` method.
    #[inline]
    pub fn new(available_width: u32, shelf_height: u32) -> AtlasBuilder {
        AtlasBuilder {
            rect_packer: RectPacker::new(available_width, shelf_height),
            batch_builders: vec![],
        }
    }

    /// Places a glyph into the atlas.
    ///
    /// The glyph is supplied as an *index* into the supplied outline buffer. Note that indices are
    /// separate from IDs; the indices are returned from each call to
    /// `OutlineBuilder::add_glyph()`.
    ///
    /// Returns the subpixel origin of the glyph in the atlas if successful or an error if there is
    /// no space left for the glyph.
    pub fn pack_glyph(&mut self, outlines: &Outlines, glyph_index: u16, point_size: f32)
                      -> Result<Point2D<f32>, ()> {
        let pixel_bounds = outlines.glyph_pixel_bounds(glyph_index, point_size);
        let atlas_origin = try!(self.rect_packer.pack(&pixel_bounds.size().cast().unwrap()));

        for batch_builder in &mut self.batch_builders {
            if let Ok(atlas_origin) = batch_builder.add_glyph(outlines,
                                                              &atlas_origin,
                                                              glyph_index,
                                                              point_size) {
                return Ok(atlas_origin)
            }
        }

        let mut batch_builder = BatchBuilder::new();
        let atlas_origin = try!(batch_builder.add_glyph(outlines,
                                                        &atlas_origin,
                                                        glyph_index,
                                                        point_size));
        self.batch_builders.push(batch_builder);
        Ok(atlas_origin)
    }

    /// Creates an atlas by uploading the atlas info to the GPU.
    pub fn create_atlas(mut self) -> Result<Atlas, GlError> {
        let mut batches = vec![];
        for batch_builder in self.batch_builders.into_iter() {
            batches.push(try!(batch_builder.create_batch()))
        }

        Ok(Atlas {
            batches: batches,
            shelf_height: self.rect_packer.shelf_height(),
            shelf_columns: self.rect_packer.shelf_columns(),
        })
    }
}

struct BatchBuilder {
    image_descriptors: Vec<ImageDescriptor>,
    image_metadata: Vec<ImageMetadata>,
}

impl BatchBuilder {
    fn new() -> BatchBuilder {
        BatchBuilder {
            image_descriptors: vec![],
            image_metadata: vec![],
        }
    }

    fn add_glyph(&mut self,
                 outlines: &Outlines,
                 atlas_origin: &Point2D<u32>,
                 glyph_index: u16,
                 point_size: f32)
                 -> Result<Point2D<f32>, ()> {
        // Check to see if we're already rendering this glyph.
        if let Some(image_descriptor) = self.image_descriptors.get(glyph_index as usize) {
            if image_descriptor.point_size == point_size {
                // Glyph is already present.
                return Ok(Point2D::new(image_descriptor.atlas_x, image_descriptor.atlas_y))
            } else {
                // Glyph is present at a different font size. We need a new batch.
                return Err(())
            }
        }

        let subpixel_bounds = outlines.glyph_subpixel_bounds(glyph_index, point_size);
        let glyph_id = outlines.glyph_id(glyph_index);
        let glyph_index = self.image_descriptors.len() as u16;

        while self.image_descriptors.len() < glyph_index as usize + 1 {
            self.image_descriptors.push(ImageDescriptor::default())
        }

        while self.image_metadata.len() < glyph_index as usize + 1 {
            self.image_metadata.push(ImageMetadata::default())
        }

        let atlas_origin = Point2D::new(atlas_origin.x as f32 + subpixel_bounds.left.fract(),
                                        atlas_origin.y as f32 + 1.0 - subpixel_bounds.top.fract());
        self.image_descriptors[glyph_index as usize] = ImageDescriptor {
            atlas_x: atlas_origin.x,
            atlas_y: atlas_origin.y,
            point_size: point_size,
            glyph_index: glyph_index as f32,
        };

        self.image_metadata[glyph_index as usize] = ImageMetadata {
            glyph_index: glyph_index as u32,
            glyph_id: glyph_id,
            start_index: outlines.descriptor(glyph_index).unwrap().start_index(),
            end_index: match outlines.descriptor(glyph_index + 1) {
                Some(descriptor) => descriptor.start_index() as u32,
                None => outlines.indices_count() as u32,
            },
        };

        Ok(atlas_origin)
    }

    /// Uploads this batch data to the GPU.
    fn create_batch(mut self) -> Result<Batch, GlError> {
        self.image_metadata.sort_by(|a, b| a.glyph_index.cmp(&b.glyph_index));

        let (mut current_range, mut counts, mut start_indices) = (None, vec![], vec![]);
        for image_metadata in &self.image_metadata {
            let glyph_index = image_metadata.glyph_index;
            let start_index = image_metadata.start_index;
            let end_index = image_metadata.end_index;

            match current_range {
                Some((current_first, current_last)) if start_index == current_last => {
                    current_range = Some((current_first, end_index))
                }
                Some((current_first, current_last)) => {
                    counts.push((current_last - current_first) as GLsizei);
                    start_indices.push(current_first as usize);
                    current_range = Some((start_index, end_index))
                }
                None => current_range = Some((start_index, end_index)),
            }
        }
        if let Some((current_first, current_last)) = current_range {
            counts.push((current_last - current_first) as GLsizei);
            start_indices.push(current_first as usize);
        }

        // TODO(pcwalton): Try using `glMapBuffer` here.
        unsafe {
            let mut images = 0;
            gl::GenBuffers(1, &mut images);

            let length = self.image_descriptors.len() * mem::size_of::<ImageDescriptor>();
            let ptr = self.image_descriptors.as_ptr() as *const ImageDescriptor as *const c_void;
            gl::BindBuffer(gl::UNIFORM_BUFFER, images);
            gl::BufferData(gl::UNIFORM_BUFFER, length as GLsizeiptr, ptr, gl::DYNAMIC_DRAW);

            Ok(Batch {
                images_buffer: images,
                start_indices: start_indices,
                counts: counts,
            })
        }
    }
}

/// An atlas holding rendered glyphs on the GPU.
pub struct Atlas {
    batches: Vec<Batch>,
    shelf_height: u32,
    shelf_columns: u32,
}

impl Atlas {
    #[doc(hidden)]
    pub unsafe fn draw(&self, primitive: GLenum) {
        for batch in &self.batches {
            batch.draw(primitive)
        }
    }

    /// Returns the height of each shelf.
    #[inline]
    pub fn shelf_height(&self) -> u32 {
        self.shelf_height
    }

    #[doc(hidden)]
    #[inline]
    pub fn shelf_columns(&self) -> u32 {
        self.shelf_columns
    }
}

struct Batch {
    images_buffer: GLuint,
    start_indices: Vec<usize>,
    counts: Vec<GLsizei>,
}

impl Drop for Batch {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &mut self.images_buffer);
        }
    }
}

impl Batch {
    unsafe fn draw(&self, primitive: GLenum) {
        debug_assert!(self.counts.len() == self.start_indices.len());

        // The image descriptors are bound to binding point 2. See `draw.vs.glsl`.
        gl::BindBufferBase(gl::UNIFORM_BUFFER, 2, self.images_buffer);

        gl::MultiDrawElements(primitive,
                              self.counts.as_ptr(),
                              gl::UNSIGNED_INT,
                              self.start_indices.as_ptr() as *const *const GLvoid,
                              self.counts.len() as GLsizei);
    }
}

// Information about each image that we send to the GPU.
#[repr(C)]
#[doc(hidden)]
#[derive(Clone, Copy, Default, Debug)]
pub struct ImageDescriptor {
    atlas_x: f32,
    atlas_y: f32,
    point_size: f32,
    glyph_index: f32,
}

// Information about each image that we keep around ourselves.
#[doc(hidden)]
#[derive(Clone, Copy, Default, Debug)]
pub struct ImageMetadata {
    glyph_index: u32,
    glyph_id: u16,
    start_index: u32,
    end_index: u32,
}

