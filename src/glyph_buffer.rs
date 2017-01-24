// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use compute_shader::buffer::{Buffer, BufferData, HostAllocatedData, Protection};
use compute_shader::device::Device;
use euclid::{Point2D, Rect, Size2D};
use gl::types::{GLsizeiptr, GLuint};
use gl;
use otf::glyf::GlyfTable;
use otf::head::HeadTable;
use otf::loca::LocaTable;
use std::mem;
use std::os::raw::c_void;

pub struct GlyphBufferBuilder {
    pub coordinates: Vec<(i16, i16)>,

    /// TODO(pcwalton): Try omitting this and binary search the glyph descriptors in the vertex
    /// shader. Might or might not help.
    pub glyph_indices: Vec<u16>,

    pub operations: Vec<u8>,
    pub descriptors: Vec<GlyphDescriptor>,
}

impl GlyphBufferBuilder {
    #[inline]
    pub fn new() -> GlyphBufferBuilder {
        GlyphBufferBuilder {
            coordinates: vec![],
            glyph_indices: vec![],
            operations: vec![],
            descriptors: vec![],
        }
    }

    pub fn add_glyph(&mut self,
                     glyph_id: u32,
                     head_table: &HeadTable,
                     loca_table: &LocaTable,
                     glyf_table: &GlyfTable)
                     -> Result<(), ()> {
        let glyph_index = self.descriptors.len() as u16;

        let mut point_index = self.coordinates.len() / 2;
        let start_point = point_index;
        let mut operations = if point_index % 4 == 0 {
            0
        } else {
            self.operations.pop().unwrap()
        };

        try!(glyf_table.for_each_point(loca_table, glyph_id, |point| {
            self.coordinates.push((point.position.x, point.position.y));
            self.glyph_indices.push(glyph_index);

            let operation = if point.first_point_in_contour {
                0
            } else if point.on_curve {
                1
            } else {
                2
            };

            operations |= operation << (point_index % 4 * 2);

            point_index += 1;
            if point_index % 4 == 0 {
                self.operations.push(operation)
            }
        }));

        if point_index % 4 != 0 {
            self.operations.push(operations)
        }

        // Add a glyph descriptor.
        let bounding_rect = try!(glyf_table.bounding_rect(loca_table, glyph_id));
        self.descriptors.push(GlyphDescriptor {
            left: bounding_rect.origin.x as i32,
            bottom: bounding_rect.origin.y as i32,
            right: bounding_rect.max_x() as i32,
            top: bounding_rect.max_y() as i32,
            units_per_em: head_table.units_per_em as u32,
            point_count: (point_index - start_point) as u32,
            start_point: start_point as u32,
            pad: 0,
        });

        Ok(())
    }

    pub fn finish(&self, device: &Device) -> Result<GlyphBuffers, ()> {
        // TODO(pcwalton): Try using `glMapBuffer` here. Requires precomputing contours.
        unsafe {
            let (mut coordinates, mut descriptors) = (0, 0);
            gl::GenBuffers(1, &mut coordinates);
            gl::GenBuffers(1, &mut descriptors);

            let length = self.coordinates.len() * mem::size_of::<(i16, i16)>();
            gl::BindBuffer(gl::ARRAY_BUFFER, coordinates);
            gl::BufferData(gl::ARRAY_BUFFER,
                           length as GLsizeiptr,
                           self.coordinates.as_ptr() as *const (i16, i16) as *const c_void,
                           gl::STATIC_DRAW);

            let length = self.descriptors.len() * mem::size_of::<GlyphDescriptor>();
            gl::BindBuffer(gl::UNIFORM_BUFFER, descriptors);
            gl::BufferData(gl::UNIFORM_BUFFER,
                           length as GLsizeiptr,
                           self.descriptors.as_ptr() as *const GlyphDescriptor as *const c_void,
                           gl::STATIC_DRAW);

            Ok(GlyphBuffers {
                coordinates: coordinates,
                descriptors: descriptors,
            })
        }
    }
}

pub struct GlyphBuffers {
    pub coordinates: GLuint,
    pub descriptors: GLuint,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GlyphDescriptor {
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub top: i32,
    pub units_per_em: u32,
    pub point_count: u32,
    pub start_point: u32,
    pub pad: u32,
}

impl GlyphDescriptor {
    #[inline]
    pub fn pixel_rect(&self, point_size: f32) -> Rect<f32> {
        let pixels_per_unit = point_size / self.units_per_em as f32;
        Rect::new(Point2D::new(self.left as f32, self.bottom as f32),
                  Size2D::new((self.right - self.left) as f32,
                              (self.bottom - self.top) as f32)) * pixels_per_unit
    }
}

