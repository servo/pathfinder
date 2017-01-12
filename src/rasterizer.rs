// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use batch::Batch;
use compute_shader::device::Device;
use compute_shader::event::Event;
use compute_shader::program::Program;
use compute_shader::queue::{Queue, Uniform};
use compute_shader::texture::Texture;
use coverage::CoverageBuffer;
use euclid::rect::Rect;
use glyph_buffer::GlyphBuffers;

// TODO(pcwalton): Don't force that these be compiled in.
// TODO(pcwalton): GLSL version.
static ACCUM_CL_SHADER: &'static str = include_str!("../resources/shaders/accum.cl");
static DRAW_CL_SHADER: &'static str = include_str!("../resources/shaders/draw.cl");

pub struct Rasterizer {
    pub device: Device,
    pub queue: Queue,
    accum_program: Program,
    draw_program: Program,
}

impl Rasterizer {
    pub fn new(device: Device, queue: Queue) -> Result<Rasterizer, ()> {
        // TODO(pcwalton): GLSL version.
        // FIXME(pcwalton): Don't panic if these fail to compile; just return an error.
        let accum_program = device.create_program(ACCUM_CL_SHADER).unwrap();
        let draw_program = device.create_program(DRAW_CL_SHADER).unwrap();
        Ok(Rasterizer {
            device: device,
            queue: queue,
            accum_program: accum_program,
            draw_program: draw_program,
        })
    }

    pub fn draw_atlas(&self,
                      atlas_rect: &Rect<u32>,
                      atlas_shelf_height: u32,
                      glyph_buffers: &GlyphBuffers,
                      batch: &Batch,
                      coverage_buffer: &CoverageBuffer,
                      texture: &Texture)
                      -> Result<Event, ()> {
        let draw_uniforms = [
            (0, Uniform::Buffer(&batch.images)),
            (1, Uniform::Buffer(&glyph_buffers.descriptors)),
            (2, Uniform::Buffer(&glyph_buffers.coordinates)),
            (3, Uniform::Buffer(&glyph_buffers.operations)),
            (4, Uniform::Buffer(&batch.indices)),
            (5, Uniform::Buffer(&coverage_buffer.buffer)),
            (6, Uniform::U32(try!(texture.width().map_err(drop)))),
        ];

        let draw_event = try!(self.queue.submit_compute(&self.draw_program,
                                                        &[batch.point_count],
                                                        &draw_uniforms,
                                                        &[]).map_err(drop));

        let atlas_rect_uniform = [
            atlas_rect.origin.x,
            atlas_rect.origin.y,
            atlas_rect.max_x(),
            atlas_rect.max_y()
        ];

        let accum_uniforms = [
            (0, Uniform::Buffer(&coverage_buffer.buffer)),
            (1, Uniform::Texture(texture)),
            (2, Uniform::UVec4(atlas_rect_uniform)),
            (3, Uniform::U32(atlas_shelf_height)),
        ];

        let accum_columns = atlas_rect.size.width * (atlas_rect.size.height / atlas_shelf_height);

        self.queue.submit_compute(&self.accum_program,
                                  &[accum_columns],
                                  &accum_uniforms,
                                  &[draw_event]).map_err(drop)
    }
}

