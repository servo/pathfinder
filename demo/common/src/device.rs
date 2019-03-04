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
use pathfinder_gpu::{BufferTarget, BufferUploadMode, Device, Resources, VertexAttrType};

/*
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
*/

pub struct GroundProgram<D> where D: Device {
    pub program: D::Program,
    pub transform_uniform: D::Uniform,
    pub color_uniform: D::Uniform,
}

impl<D> GroundProgram<D> where D: Device {
    pub fn new(device: &D, resources: &Resources) -> GroundProgram<D> {
        let program = device.create_program(resources, "demo_ground");
        let transform_uniform = device.get_uniform(&program, "Transform");
        let color_uniform = device.get_uniform(&program, "Color");
        GroundProgram { program, transform_uniform, color_uniform }
    }
}

pub struct GroundSolidVertexArray<D> where D: Device {
    pub vertex_array: D::VertexArray,
}

impl<D> GroundSolidVertexArray<D> where D: Device {
    pub fn new(device: &D,
               ground_program: &GroundProgram<D>,
               quad_vertex_positions_buffer: &D::Buffer)
               -> GroundSolidVertexArray<D> {
        let vertex_array = device.create_vertex_array();

        let position_attr = device.get_vertex_attr(&ground_program.program, "Position");

        device.bind_vertex_array(&vertex_array);
        device.use_program(&ground_program.program);
        device.bind_buffer(quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_float_vertex_attr(&position_attr, 2, VertexAttrType::U8, false, 0, 0, 0);

        GroundSolidVertexArray { vertex_array }
    }
}

pub struct GroundLineVertexArray<D> where D: Device {
    pub vertex_array: D::VertexArray,
    #[allow(dead_code)]
    grid_vertex_positions_buffer: D::Buffer,
}

impl<D> GroundLineVertexArray<D> where D: Device {
    pub fn new(device: &D, ground_program: &GroundProgram<D>) -> GroundLineVertexArray<D> {
        let grid_vertex_positions_buffer = device.create_buffer();
        device.upload_to_buffer(&grid_vertex_positions_buffer,
                                &create_grid_vertex_positions(),
                                BufferTarget::Vertex,
                                BufferUploadMode::Static);

        let vertex_array = device.create_vertex_array();

        let position_attr = device.get_vertex_attr(&ground_program.program, "Position");

        device.bind_vertex_array(&vertex_array);
        device.use_program(&ground_program.program);
        device.bind_buffer(&grid_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_float_vertex_attr(&position_attr, 2, VertexAttrType::U8, false, 0, 0, 0);

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
