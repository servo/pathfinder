// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use error::GlError;
use euclid::{Point2D, Rect, Size2D};
use gl::types::{GLsizeiptr, GLuint};
use gl;
use otf::{self, Font};
use std::mem;
use std::os::raw::c_void;

static DUMMY_VERTEX: Vertex = Vertex {
    x: 0,
    y: 0,
    glyph_index: 0,
};

pub struct OutlineBuilder {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub descriptors: Vec<GlyphDescriptor>,
}

impl OutlineBuilder {
    #[inline]
    pub fn new() -> OutlineBuilder {
        OutlineBuilder {
            vertices: vec![DUMMY_VERTEX],
            indices: vec![],
            descriptors: vec![],
        }
    }

    /// Adds a new glyph to the outline builder. Returns the glyph index, useful for calls to
    /// `Atlas::pack_glyph()`.
    pub fn add_glyph(&mut self, font: &Font, glyph_id: u16) -> Result<u16, otf::Error> {
        let glyph_index = self.descriptors.len() as u16;

        let mut point_index = self.vertices.len() as u32;
        let start_index = self.indices.len() as u32;
        let start_point = point_index;
        let mut last_point_on_curve = true;

        try!(font.for_each_point(glyph_id, |point| {
            self.vertices.push(Vertex {
                x: point.position.x,
                y: point.position.y,
                glyph_index: glyph_index,
            });

            if point.index_in_contour > 0 && point.on_curve {
                let indices = if !last_point_on_curve {
                    [point_index - 2, point_index - 1, point_index]
                } else {
                    [point_index - 1, 0, point_index]
                };
                self.indices.extend(indices.iter().cloned());
            }

            point_index += 1;
            last_point_on_curve = point.on_curve
        }));

        // Add a glyph descriptor.
        self.descriptors.push(GlyphDescriptor {
            bounds: try!(font.glyph_bounds(glyph_id)),
            units_per_em: font.units_per_em() as u32,
            start_point: start_point as u32,
            start_index: start_index,
            glyph_id: glyph_id,
        });

        Ok(glyph_index)
    }

    /// Returns the glyph rectangle in units.
    #[inline]
    pub fn glyph_bounds(&self, glyph_index: u32) -> GlyphBoundsI {
        self.descriptors[glyph_index as usize].bounds
    }

    /// Returns the glyph rectangle in fractional pixels.
    #[inline]
    pub fn glyph_pixel_bounds_f(&self, glyph_index: u32, point_size: f32) -> GlyphBoundsF {
        self.descriptors[glyph_index as usize].pixel_rect_f(point_size)
    }

    /// Returns the glyph rectangle, rounded out to the nearest pixel.
    #[inline]
    pub fn glyph_pixel_bounds_i(&self, glyph_index: u32, point_size: f32) -> GlyphBoundsI {
        self.descriptors[glyph_index as usize].pixel_rect_i(point_size)
    }

    /// Returns the ID of the glyph with the given index.
    #[inline]
    pub fn glyph_id(&self, glyph_index: u32) -> u16 {
        self.descriptors[glyph_index as usize].glyph_id
    }

    pub fn create_buffers(&self) -> Result<OutlineBuffers, GlError> {
        // TODO(pcwalton): Try using `glMapBuffer` here. Requires precomputing contour types and
        // counts.
        unsafe {
            let (mut vertices, mut indices, mut descriptors) = (0, 0, 0);
            gl::GenBuffers(1, &mut vertices);
            gl::GenBuffers(1, &mut indices);
            gl::GenBuffers(1, &mut descriptors);

            gl::BindBuffer(gl::ARRAY_BUFFER, vertices);
            gl::BufferData(gl::ARRAY_BUFFER,
                           (self.vertices.len() * mem::size_of::<Vertex>()) as GLsizeiptr,
                           self.vertices.as_ptr() as *const Vertex as *const c_void,
                           gl::STATIC_DRAW);

            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, indices);
            gl::BufferData(gl::ELEMENT_ARRAY_BUFFER,
                           (self.indices.len() * mem::size_of::<u32>()) as GLsizeiptr,
                           self.indices.as_ptr() as *const u32 as *const c_void,
                           gl::STATIC_DRAW);

            let length = self.descriptors.len() * mem::size_of::<GlyphDescriptor>();
            gl::BindBuffer(gl::UNIFORM_BUFFER, descriptors);
            gl::BufferData(gl::UNIFORM_BUFFER,
                           length as GLsizeiptr,
                           self.descriptors.as_ptr() as *const GlyphDescriptor as *const c_void,
                           gl::STATIC_DRAW);

            Ok(OutlineBuffers {
                vertices: vertices,
                indices: indices,
                descriptors: descriptors,
            })
        }
    }
}

pub struct OutlineBuffers {
    pub vertices: GLuint,
    pub indices: GLuint,
    pub descriptors: GLuint,
}

impl Drop for OutlineBuffers {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &mut self.descriptors);
            gl::DeleteBuffers(1, &mut self.indices);
            gl::DeleteBuffers(1, &mut self.vertices);
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GlyphDescriptor {
    pub bounds: GlyphBoundsI,
    pub units_per_em: u32,
    pub start_point: u32,
    pub start_index: u32,
    pub glyph_id: u16,
}

impl GlyphDescriptor {
    #[inline]
    fn pixel_rect_f(&self, point_size: f32) -> GlyphBoundsF {
        self.bounds.pixel_rect_f(self.units_per_em as u16, point_size)
    }

    #[inline]
    fn pixel_rect_i(&self, point_size: f32) -> GlyphBoundsI {
        self.bounds.pixel_rect_f(self.units_per_em as u16, point_size).to_i()
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct Vertex {
    pub x: i16,
    pub y: i16,
    /// TODO(pcwalton): Try omitting this and binary search the glyph descriptors in the vertex
    /// shader. Might or might not help.
    pub glyph_index: u16,
}

#[derive(Copy, Clone, Debug)]
pub struct GlyphBoundsF {
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
    pub top: f32,
}

impl GlyphBoundsF {
    #[inline]
    pub fn to_i(&self) -> GlyphBoundsI {
        GlyphBoundsI {
            left: self.left.floor() as i32,
            bottom: self.bottom.floor() as i32,
            right: self.right.ceil() as i32,
            top: self.top.ceil() as i32,
        }
    }

    #[inline]
    pub fn size(&self) -> Size2D<f32> {
        Size2D::new(self.right - self.left, self.top - self.bottom)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct GlyphBoundsI {
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub top: i32,
}

impl GlyphBoundsI {
    #[inline]
    pub fn pixel_rect_f(&self, units_per_em: u16, point_size: f32) -> GlyphBoundsF {
        let pixels_per_unit = point_size / units_per_em as f32;
        GlyphBoundsF {
            left: self.left as f32 * pixels_per_unit,
            bottom: self.bottom as f32 * pixels_per_unit,
            right: self.right as f32 * pixels_per_unit,
            top: self.top as f32 * pixels_per_unit,
        }
    }

    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        Size2D::new(self.right - self.left, self.top - self.bottom)
    }
}

