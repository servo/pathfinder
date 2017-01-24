// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use atlas::Atlas;
use gl::types::{GLsizei, GLsizeiptr, GLuint};
use gl;
use glyph_buffer::GlyphBufferBuilder;
use std::mem;
use std::os::raw::c_void;
use std::u16;

pub struct BatchBuilder {
    pub atlas: Atlas,
    pub images: Vec<ImageDescriptor>,
    pub glyph_indices: Vec<u32>,
}

impl BatchBuilder {
    /// FIXME(pcwalton): Including the shelf height here may be a bad API.
    #[inline]
    pub fn new(available_width: u32, shelf_height: u32) -> BatchBuilder {
        BatchBuilder {
            atlas: Atlas::new(available_width, shelf_height),
            images: vec![],
            glyph_indices: vec![],
        }
    }

    /// FIXME(pcwalton): Support the same glyph drawn at multiple point sizes.
    pub fn add_glyph(&mut self,
                     glyph_buffer_builder: &GlyphBufferBuilder,
                     glyph_index: u32,
                     point_size: f32)
                     -> Result<(), ()> {
        let descriptor = &glyph_buffer_builder.descriptors[glyph_index as usize];

        // FIXME(pcwalton): I think this will check for negative values and panic, which is
        // unnecessary.
        let pixel_size = descriptor.pixel_rect(point_size).size.ceil().cast().unwrap();
        let atlas_origin = try!(self.atlas.place(&pixel_size));

        while self.images.len() < glyph_index as usize + 1 {
            self.images.push(ImageDescriptor::default())
        }

        self.images[glyph_index as usize] = ImageDescriptor {
            atlas_x: atlas_origin.x,
            atlas_y: atlas_origin.y,
            point_size: point_size,
            glyph_index: glyph_index,
        };

        self.glyph_indices.push(glyph_index);

        Ok(())
    }

    pub fn finish(&mut self, glyph_buffer_builder: &GlyphBufferBuilder) -> Result<Batch, ()> {
        self.glyph_indices.sort();

        let (mut current_range, mut counts, mut start_indices) = (None, vec![], vec![]);
        for &glyph_index in &self.glyph_indices {
            let first_index = glyph_buffer_builder.descriptors[glyph_index as usize].start_index as
                usize;
            let last_index = match glyph_buffer_builder.descriptors.get(glyph_index as usize + 1) {
                Some(ref descriptor) => descriptor.start_index as usize,
                None => glyph_buffer_builder.indices.len(),
            };

            match current_range {
                Some((current_first, current_last)) if first_index == current_last => {
                    current_range = Some((current_first, last_index))
                }
                Some((current_first, current_last)) => {
                    counts.push((current_last - current_first) as GLsizei);
                    start_indices.push(current_first);
                    current_range = Some((first_index, last_index))
                }
                None => current_range = Some((first_index, last_index)),
            }
        }
        if let Some((current_first, current_last)) = current_range {
            counts.push((current_last - current_first) as GLsizei);
            start_indices.push(current_first);
        }

        // TODO(pcwalton): Try using `glMapBuffer` here.
        unsafe {
            let mut images = 0;
            gl::GenBuffers(1, &mut images);

            gl::BindBuffer(gl::UNIFORM_BUFFER, images);
            gl::BufferData(gl::UNIFORM_BUFFER,
                           (self.images.len() * mem::size_of::<ImageDescriptor>()) as GLsizeiptr,
                           self.images.as_ptr() as *const ImageDescriptor as *const c_void,
                           gl::DYNAMIC_DRAW);

            Ok(Batch {
                start_indices: start_indices,
                counts: counts,
                images: images,
            })
        }
    }
}

pub struct Batch {
    pub start_indices: Vec<usize>,
    pub counts: Vec<GLsizei>,
    pub images: GLuint,
}

impl Drop for Batch {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &mut self.images);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GlyphRange {
    pub start: u16,
    pub end: u16,
}

impl GlyphRange {
    #[inline]
    pub fn iter(&self) -> GlyphRangeIter {
        GlyphRangeIter {
            start: self.start,
            end: self.end,
        }
    }
}

#[derive(Clone)]
pub struct GlyphRangeIter {
    start: u16,
    end: u16,
}

impl Iterator for GlyphRangeIter {
    type Item = u16;

    #[inline]
    fn next(&mut self) -> Option<u16> {
        if self.start > self.end {
            None
        } else {
            let item = self.start;
            self.start += 1;
            Some(item)
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct ImageDescriptor {
    atlas_x: u32,
    atlas_y: u32,
    point_size: f32,
    glyph_index: u32,
}

