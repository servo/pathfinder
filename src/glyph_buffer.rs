// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use euclid::{Point2D, Rect, Size2D};
use gl::types::{GLsizeiptr, GLuint};
use gl;
use otf::Font;
use std::mem;
use std::os::raw::c_void;

static DUMMY_VERTEX: Vertex = Vertex {
    x: 0,
    y: 0,
    glyph_index: 0,
};

pub struct GlyphBufferBuilder {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub descriptors: Vec<GlyphDescriptor>,
}

impl GlyphBufferBuilder {
    #[inline]
    pub fn new() -> GlyphBufferBuilder {
        GlyphBufferBuilder {
            vertices: vec![DUMMY_VERTEX],
            indices: vec![],
            descriptors: vec![],
        }
    }

    pub fn add_glyph(&mut self, font: &Font, glyph_id: u16) -> Result<(), ()> {
        let glyph_index = self.descriptors.len() as u16;

        let mut point_index = self.vertices.len() as u32;
        let start_index = self.indices.len() as u32;
        let start_point = point_index;
        let mut last_point_on_curve = true;

        let glyf_table = try!(font.glyf.ok_or(()));
        let loca_table = try!(font.loca.as_ref().ok_or(()));

        try!(glyf_table.for_each_point(&font.head, loca_table, glyph_id, |point| {
            self.vertices.push(Vertex {
                x: point.position.x,
                y: point.position.y,
                glyph_index: glyph_index,
            });

            if !point.first_point_in_contour && point.on_curve {
                let indices = if last_point_on_curve {
                    [point_index - 1, 0, point_index]
                } else {
                    [point_index - 2, point_index - 1, point_index]
                };
                self.indices.extend(indices.iter().cloned());
            }

            point_index += 1;
            last_point_on_curve = point.on_curve
        }));

        // Add a glyph descriptor.
        self.descriptors.push(GlyphDescriptor {
            bounds: try!(glyf_table.glyph_bounds(&font.head, loca_table, glyph_id)),
            units_per_em: font.head.units_per_em as u32,
            start_point: start_point as u32,
            start_index: start_index,
            glyph_id: glyph_id,
        });

        Ok(())
    }

    /// Returns the glyph rectangle in units.
    #[inline]
    pub fn glyph_bounds(&self, glyph_index: u32) -> GlyphBounds {
        self.descriptors[glyph_index as usize].bounds
    }

    pub fn create_buffers(&self) -> Result<GlyphBuffers, ()> {
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

            Ok(GlyphBuffers {
                vertices: vertices,
                indices: indices,
                descriptors: descriptors,
            })
        }
    }
}

pub struct GlyphBuffers {
    pub vertices: GLuint,
    pub indices: GLuint,
    pub descriptors: GLuint,
}

impl Drop for GlyphBuffers {
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
    pub bounds: GlyphBounds,
    pub units_per_em: u32,
    pub start_point: u32,
    pub start_index: u32,
    pub glyph_id: u16,
}

impl GlyphDescriptor {
    #[inline]
    pub fn pixel_rect(&self, point_size: f32) -> Rect<f32> {
        let pixels_per_unit = point_size / self.units_per_em as f32;
        Rect::new(Point2D::new(self.bounds.left as f32, self.bounds.bottom as f32),
                  Size2D::new((self.bounds.right - self.bounds.left) as f32,
                              (self.bounds.top - self.bounds.bottom) as f32)) * pixels_per_unit
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
pub struct GlyphBounds {
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub top: i32,
}

