// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Glyph vectors, uploaded in a resolution-independent manner to the GPU.

use error::{FontError, GlError};
use euclid::{Point2D, Size2D};
use font::{Font, PointKind};
use gl::types::{GLsizeiptr, GLuint};
use gl;
use std::mem;
use std::os::raw::c_void;

static DUMMY_VERTEX: Vertex = Vertex {
    x: 0,
    y: 0,
    glyph_index: 0,
};

/// Packs up outlines for glyphs into a format that the GPU can process.
pub struct OutlineBuilder {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    descriptors: Vec<GlyphDescriptor>,
}

impl OutlineBuilder {
    /// Creates a new empty set of outlines.
    #[inline]
    pub fn new() -> OutlineBuilder {
        OutlineBuilder {
            vertices: vec![DUMMY_VERTEX],
            indices: vec![],
            descriptors: vec![],
        }
    }

    /// Begins a new path.
    pub fn create_path(&mut self) -> PathBuilder {
        let vertex_count = self.vertices.len();
        let index_count = self.indices.len();
        let descriptor_count = self.descriptors.len();

        PathBuilder {
            outline_builder: self,
            vbo_start_index: vertex_count as u32,
            vbo_end_index: vertex_count as u32,
            ibo_start_index: index_count as u32,
            point_index_in_path: 0,
            glyph_index: descriptor_count as u16,
        }
    }

    /// Adds a new glyph to the outline builder. Returns the glyph index, which is useful for later
    /// calls to `Atlas::pack_glyph()`.
    pub fn add_glyph(&mut self, font: &Font, glyph_id: u16) -> Result<u16, FontError> {
        let glyph_index = self.descriptors.len() as u16;
        let mut last_point_kind = PointKind::OnCurve;
        let mut control_point_index = 0;
        let mut control_points = [Point2D::zero(), Point2D::zero(), Point2D::zero()];
        let mut path_builder = self.create_path();

        try!(font.for_each_point(glyph_id, |point| {
            control_points[control_point_index] = point.position;
            control_point_index += 1;

            if point.index_in_contour == 0 {
                path_builder.move_to(&control_points[0]);
                control_point_index = 0
            } else if point.kind == PointKind::OnCurve {
                match last_point_kind {
                    PointKind::FirstCubicControl => {}
                    PointKind::SecondCubicControl => {
                        path_builder.cubic_curve_to(&control_points[0],
                                                    &control_points[1],
                                                    &control_points[2])
                    }
                    PointKind::QuadControl => {
                        path_builder.quad_curve_to(&control_points[0], &control_points[1])
                    }
                    PointKind::OnCurve => path_builder.line_to(&control_points[0]),
                }

                control_point_index = 0
            }

            last_point_kind = point.kind
        }));

        let bounds = try!(font.glyph_bounds(glyph_id));
        path_builder.finish(&bounds, font.units_per_em() as u32, glyph_id);

        Ok(glyph_index)
    }

    /// Uploads the outlines to the GPU.
    pub fn create_buffers(self) -> Result<Outlines, GlError> {
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

            Ok(Outlines {
                vertices_buffer: vertices,
                indices_buffer: indices,
                descriptors_buffer: descriptors,
                descriptors: self.descriptors,
                indices_count: self.indices.len(),
            })
        }
    }
}

/// Resolution-independent glyph vectors uploaded to the GPU.
pub struct Outlines {
    vertices_buffer: GLuint,
    indices_buffer: GLuint,
    descriptors_buffer: GLuint,
    descriptors: Vec<GlyphDescriptor>,
    indices_count: usize,
}

impl Drop for Outlines {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &mut self.descriptors_buffer);
            gl::DeleteBuffers(1, &mut self.indices_buffer);
            gl::DeleteBuffers(1, &mut self.vertices_buffer);
        }
    }
}

impl Outlines {
    #[doc(hidden)]
    #[inline]
    pub fn vertices_buffer(&self) -> GLuint {
        self.vertices_buffer
    }

    #[doc(hidden)]
    #[inline]
    pub fn indices_buffer(&self) -> GLuint {
        self.indices_buffer
    }

    #[doc(hidden)]
    #[inline]
    pub fn descriptors_buffer(&self) -> GLuint {
        self.descriptors_buffer
    }

    #[doc(hidden)]
    #[inline]
    pub fn descriptor(&self, glyph_index: u16) -> Option<&GlyphDescriptor> {
        self.descriptors.get(glyph_index as usize)
    }

    #[doc(hidden)]
    #[inline]
    pub fn indices_count(&self) -> usize {
        self.indices_count
    }

    /// Returns the glyph rectangle in font units.
    #[inline]
    pub fn glyph_bounds(&self, glyph_index: u32) -> GlyphBounds {
        self.descriptors[glyph_index as usize].bounds
    }

    /// Returns the glyph rectangle in fractional pixels.
    #[inline]
    pub fn glyph_subpixel_bounds(&self, glyph_index: u16, point_size: f32) -> GlyphSubpixelBounds {
        self.descriptors[glyph_index as usize].subpixel_bounds(point_size)
    }

    /// Returns the ID of the glyph with the given index.
    #[inline]
    pub fn glyph_id(&self, glyph_index: u16) -> u16 {
        self.descriptors[glyph_index as usize].glyph_id
    }

    /// Returns the units per em for the glyph with the given index.
    #[inline]
    pub fn glyph_units_per_em(&self, glyph_index: u16) -> u32 {
        self.descriptors[glyph_index as usize].units_per_em
    }
}

#[doc(hidden)]
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GlyphDescriptor {
    bounds: GlyphBounds,
    units_per_em: u32,
    start_point: u32,
    start_index: u32,
    glyph_id: u16,
}

impl GlyphDescriptor {
    #[doc(hidden)]
    #[inline]
    pub fn start_index(&self) -> u32 {
        self.start_index
    }

    #[doc(hidden)]
    #[inline]
    fn subpixel_bounds(&self, point_size: f32) -> GlyphSubpixelBounds {
        self.bounds.subpixel_bounds(self.units_per_em as u16, point_size)
    }
}

#[doc(hidden)]
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct Vertex {
    x: i16,
    y: i16,
    glyph_index: u16,
}

/// The boundaries of the glyph in fractional pixels.
#[derive(Copy, Clone, Debug)]
pub struct GlyphSubpixelBounds {
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
    pub top: f32,
}

impl GlyphSubpixelBounds {
    /// Scales the bounds by the given amount.
    #[inline]
    pub fn scale(&mut self, factor: f32) {
        self.left *= factor;
        self.bottom *= factor;
        self.right *= factor;
        self.top *= factor;
    }

    /// Rounds these bounds out to the nearest pixel.
    #[inline]
    pub fn round_out(&self) -> GlyphPixelBounds {
        GlyphPixelBounds {
            left: self.left.floor() as i32,
            bottom: self.bottom.floor() as i32,
            right: self.right.ceil() as i32,
            top: self.top.ceil() as i32,
        }
    }

    /// Returns the total size of the glyph in fractional pixels.
    #[inline]
    pub fn size(&self) -> Size2D<f32> {
        Size2D::new(self.right - self.left, self.top - self.bottom)
    }
}

/// The boundaries of the glyph, rounded out to the nearest pixel.
#[derive(Copy, Clone, Debug)]
pub struct GlyphPixelBounds {
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub top: i32,
}

impl GlyphPixelBounds {
    /// Returns the total size of the glyph in whole pixels.
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        Size2D::new(self.right - self.left, self.top - self.bottom)
    }
}

/// The boundaries of a glyph in font units.
#[derive(Copy, Clone, Default, Debug)]
pub struct GlyphBounds {
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub top: i32,
}

impl GlyphBounds {
    /// Given the units per em of the font and the point size, returns the fractional boundaries of
    /// this glyph.
    #[inline]
    pub fn subpixel_bounds(&self, units_per_em: u16, point_size: f32) -> GlyphSubpixelBounds {
        let pixels_per_unit = point_size / units_per_em as f32;
        GlyphSubpixelBounds {
            left: self.left as f32 * pixels_per_unit,
            bottom: self.bottom as f32 * pixels_per_unit,
            right: self.right as f32 * pixels_per_unit,
            top: self.top as f32 * pixels_per_unit,
        }
    }

    /// Returns the total size of the glyph in font units.
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        Size2D::new(self.right - self.left, self.top - self.bottom)
    }
}

/// A helper object to construct a single path.
pub struct PathBuilder<'a> {
    outline_builder: &'a mut OutlineBuilder,
    vbo_start_index: u32,
    vbo_end_index: u32,
    ibo_start_index: u32,
    point_index_in_path: u16,
    glyph_index: u16,
}

impl<'a> PathBuilder<'a> {
    fn add_point(&mut self, point: &Point2D<i16>) {
        self.outline_builder.vertices.push(Vertex {
            x: point.x,
            y: point.y,
            glyph_index: self.glyph_index,
        });

        self.point_index_in_path += 1;
        self.vbo_end_index += 1;
    }

    /// Moves the pen to the given point.
    pub fn move_to(&mut self, point: &Point2D<i16>) {
        self.add_point(point)
    }

    /// Draws a straight line to the given point.
    ///
    /// Panics if a `move_to` has not been issued anywhere prior to this operation.
    pub fn line_to(&mut self, point: &Point2D<i16>) {
        if self.point_index_in_path == 0 {
            panic!("`line_to` must not be the first operation in a path")
        }

        self.add_point(point);

        self.outline_builder.indices.extend_from_slice(&[
            self.vbo_end_index - 2,
            0,
            0,
            self.vbo_end_index - 1,
        ])
    }

    /// Draws a quadratic Bézier curve to the given point.
    ///
    /// Panics if a `move_to` has not been issued anywhere prior to this operation.
    pub fn quad_curve_to(&mut self, p1: &Point2D<i16>, p2: &Point2D<i16>) {
        if self.point_index_in_path == 0 {
            panic!("`quad_curve_to` must not be the first operation in a path")
        }

        self.add_point(p1);
        self.add_point(p2);

        self.outline_builder.indices.extend_from_slice(&[
            self.vbo_end_index - 3,
            self.vbo_end_index - 2,
            self.vbo_end_index - 2,
            self.vbo_end_index - 1,
        ])
    }

    /// Draws a cubic Bézier curve to the given point.
    ///
    /// Panics if a `move_to` has not been issued anywhere prior to this operation.
    pub fn cubic_curve_to(&mut self, p1: &Point2D<i16>, p2: &Point2D<i16>, p3: &Point2D<i16>) {
        if self.point_index_in_path == 0 {
            panic!("`cubic_curve_to` must not be the first operation in a path")
        }

        self.add_point(p1);
        self.add_point(p2);
        self.add_point(p3);

        self.outline_builder.indices.extend_from_slice(&[
            self.vbo_end_index - 4,
            self.vbo_end_index - 3,
            self.vbo_end_index - 2,
            self.vbo_end_index - 1
        ])
    }

    /// Finishes the path.
    pub fn finish(self, bounds: &GlyphBounds, units_per_em: u32, glyph_id: u16) {
        self.outline_builder.descriptors.push(GlyphDescriptor {
            bounds: *bounds,
            units_per_em: units_per_em,
            start_point: self.vbo_start_index,
            start_index: self.ibo_start_index,
            glyph_id: glyph_id,
        })
    }
}

