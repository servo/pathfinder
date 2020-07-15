// pathfinder/renderer/src/gpu/shaders.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use pathfinder_gpu::{BufferTarget, BufferUploadMode, Device, VertexAttrClass};
use pathfinder_gpu::{VertexAttrDescriptor, VertexAttrType};
use pathfinder_resources::ResourceLoader;

// TODO(pcwalton): Replace with `mem::size_of` calls?
pub(crate) const TILE_INSTANCE_SIZE: usize = 16;

pub(crate) struct BlitVertexArray<D> where D: Device {
    pub(crate) vertex_array: D::VertexArray,
}

impl<D> BlitVertexArray<D> where D: Device {
    pub(crate) fn new(device: &D,
                      blit_program: &BlitProgram<D>,
                      quad_vertex_positions_buffer: &D::Buffer,
                      quad_vertex_indices_buffer: &D::Buffer)
                      -> BlitVertexArray<D> {
        let vertex_array = device.create_vertex_array();
        let position_attr = device.get_vertex_attr(&blit_program.program, "Position").unwrap();

        device.bind_buffer(&vertex_array, quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &position_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: 4,
            offset: 0,
            divisor: 0,
            buffer_index: 0,
        });
        device.bind_buffer(&vertex_array, quad_vertex_indices_buffer, BufferTarget::Index);

        BlitVertexArray { vertex_array }
    }
}

pub(crate) struct VertexArraysCore<D> where D: Device {
    pub(crate) blit_vertex_array: BlitVertexArray<D>,
}

impl<D> VertexArraysCore<D> where D: Device {
    pub(crate) fn new(device: &D,
               programs: &ProgramsCore<D>,
               quad_vertex_positions_buffer: &D::Buffer,
               quad_vertex_indices_buffer: &D::Buffer)
               -> VertexArraysCore<D> {
        VertexArraysCore {
            blit_vertex_array: BlitVertexArray::new(device,
                                                    &programs.blit_program,
                                                    quad_vertex_positions_buffer,
                                                    quad_vertex_indices_buffer),
        }
    }
}

pub(crate) struct ClearVertexArray<D> where D: Device {
    pub(crate) vertex_array: D::VertexArray,
}

impl<D> ClearVertexArray<D> where D: Device {
    pub(crate) fn new(device: &D,
                      clear_program: &ClearProgram<D>,
                      quad_vertex_positions_buffer: &D::Buffer,
                      quad_vertex_indices_buffer: &D::Buffer)
                      -> ClearVertexArray<D> {
        let vertex_array = device.create_vertex_array();
        let position_attr = device.get_vertex_attr(&clear_program.program, "Position").unwrap();

        device.bind_buffer(&vertex_array, quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &position_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: 4,
            offset: 0,
            divisor: 0,
            buffer_index: 0,
        });
        device.bind_buffer(&vertex_array, quad_vertex_indices_buffer, BufferTarget::Index);

        ClearVertexArray { vertex_array }
    }
}

pub(crate) struct BlitProgram<D> where D: Device {
    pub(crate) program: D::Program,
    pub(crate) dest_rect_uniform: D::Uniform,
    pub(crate) framebuffer_size_uniform: D::Uniform,
    pub(crate) src_texture: D::TextureParameter,
}

impl<D> BlitProgram<D> where D: Device {
    pub(crate) fn new(device: &D, resources: &dyn ResourceLoader) -> BlitProgram<D> {
        let program = device.create_raster_program(resources, "blit");
        let dest_rect_uniform = device.get_uniform(&program, "DestRect");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let src_texture = device.get_texture_parameter(&program, "Src");
        BlitProgram { program, dest_rect_uniform, framebuffer_size_uniform, src_texture }
    }
}

pub(crate) struct ProgramsCore<D> where D: Device {
    pub(crate) blit_program: BlitProgram<D>,
}

impl<D> ProgramsCore<D> where D: Device {
    pub(crate) fn new(device: &D, resources: &dyn ResourceLoader) -> ProgramsCore<D> {
        ProgramsCore {
            blit_program: BlitProgram::new(device, resources),
        }
    }
}

pub(crate) struct ClearProgram<D> where D: Device {
    pub(crate) program: D::Program,
    pub(crate) rect_uniform: D::Uniform,
    pub(crate) framebuffer_size_uniform: D::Uniform,
    pub(crate) color_uniform: D::Uniform,
}

impl<D> ClearProgram<D> where D: Device {
    pub(crate) fn new(device: &D, resources: &dyn ResourceLoader) -> ClearProgram<D> {
        let program = device.create_raster_program(resources, "clear");
        let rect_uniform = device.get_uniform(&program, "Rect");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let color_uniform = device.get_uniform(&program, "Color");
        ClearProgram { program, rect_uniform, framebuffer_size_uniform, color_uniform }
    }
}

pub(crate) struct TileProgramCommon<D> where D: Device {
    pub(crate) program: D::Program,
    pub(crate) tile_size_uniform: D::Uniform,
    pub(crate) texture_metadata_texture: D::TextureParameter,
    pub(crate) texture_metadata_size_uniform: D::Uniform,
    pub(crate) z_buffer_texture: D::TextureParameter,
    pub(crate) z_buffer_texture_size_uniform: D::Uniform,
    pub(crate) color_texture_0: D::TextureParameter,
    pub(crate) color_texture_size_0_uniform: D::Uniform,
    pub(crate) mask_texture_0: D::TextureParameter,
    pub(crate) mask_texture_size_0_uniform: D::Uniform,
    pub(crate) gamma_lut_texture: D::TextureParameter,
    pub(crate) framebuffer_size_uniform: D::Uniform,
}

impl<D> TileProgramCommon<D> where D: Device {
    pub(crate) fn new(device: &D, program: D::Program) -> TileProgramCommon<D> {
        let tile_size_uniform = device.get_uniform(&program, "TileSize");
        let texture_metadata_texture = device.get_texture_parameter(&program, "TextureMetadata");
        let texture_metadata_size_uniform = device.get_uniform(&program, "TextureMetadataSize");
        let z_buffer_texture = device.get_texture_parameter(&program, "ZBuffer");
        let z_buffer_texture_size_uniform = device.get_uniform(&program, "ZBufferSize");
        let color_texture_0 = device.get_texture_parameter(&program, "ColorTexture0");
        let color_texture_size_0_uniform = device.get_uniform(&program, "ColorTextureSize0");
        let mask_texture_0 = device.get_texture_parameter(&program, "MaskTexture0");
        let mask_texture_size_0_uniform = device.get_uniform(&program, "MaskTextureSize0");
        let gamma_lut_texture = device.get_texture_parameter(&program, "GammaLUT");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");

        TileProgramCommon {
            program,
            tile_size_uniform,
            texture_metadata_texture,
            texture_metadata_size_uniform,
            z_buffer_texture,
            z_buffer_texture_size_uniform,
            color_texture_0,
            color_texture_size_0_uniform,
            mask_texture_0,
            mask_texture_size_0_uniform,
            gamma_lut_texture,
            framebuffer_size_uniform,
        }
    }
}

pub(crate) struct StencilProgram<D> where D: Device {
    pub(crate) program: D::Program,
}

impl<D> StencilProgram<D> where D: Device {
    pub(crate) fn new(device: &D, resources: &dyn ResourceLoader) -> StencilProgram<D> {
        let program = device.create_raster_program(resources, "stencil");
        StencilProgram { program }
    }
}

pub(crate) struct StencilVertexArray<D> where D: Device {
    pub(crate) vertex_array: D::VertexArray,
    pub(crate) vertex_buffer: D::Buffer,
    pub(crate) index_buffer: D::Buffer,
}

impl<D> StencilVertexArray<D> where D: Device {
    pub(crate) fn new(device: &D, stencil_program: &StencilProgram<D>) -> StencilVertexArray<D> {
        let vertex_array = device.create_vertex_array();
        let vertex_buffer = device.create_buffer(BufferUploadMode::Static);
        let index_buffer = device.create_buffer(BufferUploadMode::Static);

        let position_attr = device.get_vertex_attr(&stencil_program.program, "Position").unwrap();

        device.bind_buffer(&vertex_array, &vertex_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &position_attr, &VertexAttrDescriptor {
            size: 3,
            class: VertexAttrClass::Float,
            attr_type: VertexAttrType::F32,
            stride: 4 * 4,
            offset: 0,
            divisor: 0,
            buffer_index: 0,
        });
        device.bind_buffer(&vertex_array, &index_buffer, BufferTarget::Index);

        StencilVertexArray { vertex_array, vertex_buffer, index_buffer }
    }
}

pub(crate) struct ReprojectionProgram<D> where D: Device {
    pub(crate) program: D::Program,
    pub(crate) old_transform_uniform: D::Uniform,
    pub(crate) new_transform_uniform: D::Uniform,
    pub(crate) texture: D::TextureParameter,
}

impl<D> ReprojectionProgram<D> where D: Device {
    pub(crate) fn new(device: &D, resources: &dyn ResourceLoader) -> ReprojectionProgram<D> {
        let program = device.create_raster_program(resources, "reproject");
        let old_transform_uniform = device.get_uniform(&program, "OldTransform");
        let new_transform_uniform = device.get_uniform(&program, "NewTransform");
        let texture = device.get_texture_parameter(&program, "Texture");
        ReprojectionProgram { program, old_transform_uniform, new_transform_uniform, texture }
    }
}

pub(crate) struct ReprojectionVertexArray<D> where D: Device {
    pub(crate) vertex_array: D::VertexArray,
}

impl<D> ReprojectionVertexArray<D> where D: Device {
    pub(crate) fn new(device: &D,
                      reprojection_program: &ReprojectionProgram<D>,
                      quad_vertex_positions_buffer: &D::Buffer,
                      quad_vertex_indices_buffer: &D::Buffer)
                      -> ReprojectionVertexArray<D> {
        let vertex_array = device.create_vertex_array();
        let position_attr = device.get_vertex_attr(&reprojection_program.program, "Position")
                                  .unwrap();

        device.bind_buffer(&vertex_array, quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &position_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: 4,
            offset: 0,
            divisor: 0,
            buffer_index: 0,
        });
        device.bind_buffer(&vertex_array, quad_vertex_indices_buffer, BufferTarget::Index);

        ReprojectionVertexArray { vertex_array }
    }
}
