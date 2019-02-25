// pathfinder/demo/common/src/device.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! GPU rendering code specifically for the demo.

use crate::GRIDLINE_COUNT;
use gl::types::{GLsizei, GLvoid};
use pathfinder_gl::device::{Buffer, BufferTarget, BufferUploadMode, Device, Program, Uniform};
use pathfinder_gl::device::{VertexArray, VertexAttr};
use pathfinder_renderer::paint::ColorU;

pub struct DemoDevice {
    #[allow(dead_code)]
    device: Device,
}

impl DemoDevice {
    pub fn new(device: Device) -> DemoDevice {
        DemoDevice { device }
    }

    pub fn clear(&self, color: ColorU) {
        let color = color.to_f32();
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::ClearColor(color.r(), color.g(), color.b(), color.a());
            gl::ClearDepth(1.0);
            gl::ClearStencil(0);
            gl::DepthMask(gl::TRUE);
            gl::StencilMask(!0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT | gl::STENCIL_BUFFER_BIT);
        }
    }

    pub fn readback_pixels(&self, width: u32, height: u32) -> Vec<u8> {
        let mut pixels = vec![0; width as usize * height as usize * 4];
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::ReadPixels(0, 0,
                           width as GLsizei, height as GLsizei,
                           gl::RGBA,
                           gl::UNSIGNED_BYTE,
                           pixels.as_mut_ptr() as *mut GLvoid);
        }

        // Flip right-side-up.
        let stride = width as usize * 4;
        for y in 0..(height as usize / 2) {
            let (index_a, index_b) = (y * stride, (height as usize - y - 1) * stride);
            for offset in 0..stride {
                pixels.swap(index_a + offset, index_b + offset);
            }
        }

        pixels
    }
}

pub struct GroundProgram {
    pub program: Program,
    pub transform_uniform: Uniform,
    pub color_uniform: Uniform,
}

impl GroundProgram {
    pub fn new(device: &Device) -> GroundProgram {
        let program = device.create_program("demo_ground");
        let transform_uniform = Uniform::new(&program, "Transform");
        let color_uniform = Uniform::new(&program, "Color");
        GroundProgram { program, transform_uniform, color_uniform }
    }
}

pub struct GroundSolidVertexArray {
    pub vertex_array: VertexArray,
}

impl GroundSolidVertexArray {
    pub fn new(ground_program: &GroundProgram, quad_vertex_positions_buffer: &Buffer)
               -> GroundSolidVertexArray {
        let vertex_array = VertexArray::new();
        unsafe {
            let position_attr = VertexAttr::new(&ground_program.program, "Position");

            gl::BindVertexArray(vertex_array.gl_vertex_array);
            gl::UseProgram(ground_program.program.gl_program);
            gl::BindBuffer(gl::ARRAY_BUFFER, quad_vertex_positions_buffer.gl_buffer);
            position_attr.configure_float(2, gl::UNSIGNED_BYTE, false, 0, 0, 0);
        }

        GroundSolidVertexArray { vertex_array }
    }
}

pub struct GroundLineVertexArray {
    pub vertex_array: VertexArray,
    #[allow(dead_code)]
    grid_vertex_positions_buffer: Buffer,
}

impl GroundLineVertexArray {
    pub fn new(ground_program: &GroundProgram) -> GroundLineVertexArray {
        let grid_vertex_positions_buffer = Buffer::new();
        grid_vertex_positions_buffer.upload(&create_grid_vertex_positions(),
                                            BufferTarget::Vertex,
                                            BufferUploadMode::Static);

        let vertex_array = VertexArray::new();
        unsafe {
            let position_attr = VertexAttr::new(&ground_program.program, "Position");

            gl::BindVertexArray(vertex_array.gl_vertex_array);
            gl::UseProgram(ground_program.program.gl_program);
            gl::BindBuffer(gl::ARRAY_BUFFER, grid_vertex_positions_buffer.gl_buffer);
            position_attr.configure_float(2, gl::UNSIGNED_BYTE, false, 0, 0, 0);
        }

        GroundLineVertexArray { vertex_array, grid_vertex_positions_buffer }
    }
}

fn create_grid_vertex_positions() -> Vec<(u8, u8)> {
    let mut positions = vec![];
    for index in 0..(GRIDLINE_COUNT + 1) {
        positions.extend_from_slice(&[
            (0, index), (GRIDLINE_COUNT, index),
            (index, 0), (index, GRIDLINE_COUNT),
        ]);
    }
    positions
}
