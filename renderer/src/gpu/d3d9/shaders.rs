// pathfinder/renderer/src/gpu/d3d9/shaders.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Shaders and vertex specifications for the Direct3D 9-level renderer.

use crate::gpu::shaders::{TILE_INSTANCE_SIZE, TileProgramCommon};
use pathfinder_gpu::{BufferTarget, Device, VertexAttrClass, VertexAttrDescriptor, VertexAttrType};
use pathfinder_resources::ResourceLoader;

const FILL_INSTANCE_SIZE: usize = 12;
const CLIP_TILE_INSTANCE_SIZE: usize = 16;

pub(crate) struct FillVertexArrayD3D9<D> where D: Device {
    pub(crate) vertex_array: D::VertexArray,
}

impl<D> FillVertexArrayD3D9<D> where D: Device {
    pub(crate) fn new(device: &D,
                      fill_program: &FillProgramD3D9<D>,
                      vertex_buffer: &D::Buffer,
                      quad_vertex_positions_buffer: &D::Buffer,
                      quad_vertex_indices_buffer: &D::Buffer)
                      -> FillVertexArrayD3D9<D> {
        let vertex_array = device.create_vertex_array();

        let tess_coord_attr = device.get_vertex_attr(&fill_program.program, "TessCoord").unwrap();
        let line_segment_attr = device.get_vertex_attr(&fill_program.program, "LineSegment")
                                      .unwrap();
        let tile_index_attr = device.get_vertex_attr(&fill_program.program, "TileIndex").unwrap();

        device.bind_buffer(&vertex_array, quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &tess_coord_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U16,
            stride: 4,
            offset: 0,
            divisor: 0,
            buffer_index: 0,
        });
        device.bind_buffer(&vertex_array, &vertex_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &line_segment_attr, &VertexAttrDescriptor {
            size: 4,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U16,
            stride: FILL_INSTANCE_SIZE,
            offset: 0,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &tile_index_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I32,
            stride: FILL_INSTANCE_SIZE,
            offset: 8,
            divisor: 1,
            buffer_index: 1,
        });
        device.bind_buffer(&vertex_array, quad_vertex_indices_buffer, BufferTarget::Index);

        FillVertexArrayD3D9 { vertex_array }
    }
}

pub(crate) struct TileVertexArrayD3D9<D> where D: Device {
    pub(crate) vertex_array: D::VertexArray,
}

impl<D> TileVertexArrayD3D9<D> where D: Device {
    pub(crate) fn new(device: &D,
                      tile_program: &TileProgramD3D9<D>,
                      tile_vertex_buffer: &D::Buffer,
                      quad_vertex_positions_buffer: &D::Buffer,
                      quad_vertex_indices_buffer: &D::Buffer)
                      -> TileVertexArrayD3D9<D> {
        let vertex_array = device.create_vertex_array();

        let tile_offset_attr =
            device.get_vertex_attr(&tile_program.common.program, "TileOffset").unwrap();
        let tile_origin_attr =
            device.get_vertex_attr(&tile_program.common.program, "TileOrigin").unwrap();
        let mask_0_tex_coord_attr =
            device.get_vertex_attr(&tile_program.common.program, "MaskTexCoord0").unwrap();
        let ctrl_backdrop_attr =
            device.get_vertex_attr(&tile_program.common.program, "CtrlBackdrop").unwrap();
        let color_attr = device.get_vertex_attr(&tile_program.common.program, "Color").unwrap();
        let path_index_attr = device.get_vertex_attr(&tile_program.common.program, "PathIndex")
                                    .unwrap();

        device.bind_buffer(&vertex_array, quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &tile_offset_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: 4,
            offset: 0,
            divisor: 0,
            buffer_index: 0,
        });
        device.bind_buffer(&vertex_array, tile_vertex_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &tile_origin_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: TILE_INSTANCE_SIZE,
            offset: 0,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &mask_0_tex_coord_attr, &VertexAttrDescriptor {
            size: 4,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U8,
            stride: TILE_INSTANCE_SIZE,
            offset: 4,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &path_index_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I32,
            stride: TILE_INSTANCE_SIZE,
            offset: 8,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &color_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: TILE_INSTANCE_SIZE,
            offset: 12,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &ctrl_backdrop_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I8,
            stride: TILE_INSTANCE_SIZE,
            offset: 14,
            divisor: 1,
            buffer_index: 1,
        });
        device.bind_buffer(&vertex_array, quad_vertex_indices_buffer, BufferTarget::Index);

        TileVertexArrayD3D9 { vertex_array }
    }
}

pub(crate) struct ClipTileCopyVertexArrayD3D9<D> where D: Device {
    pub(crate) vertex_array: D::VertexArray,
}

impl<D> ClipTileCopyVertexArrayD3D9<D> where D: Device {
    pub(crate) fn new(device: &D,
                      clip_tile_copy_program: &ClipTileCopyProgramD3D9<D>,
                      vertex_buffer: &D::Buffer,
                      quad_vertex_positions_buffer: &D::Buffer,
                      quad_vertex_indices_buffer: &D::Buffer)
                      -> ClipTileCopyVertexArrayD3D9<D> {
        let vertex_array = device.create_vertex_array();

        let tile_offset_attr =
            device.get_vertex_attr(&clip_tile_copy_program.program, "TileOffset").unwrap();
        let tile_index_attr =
            device.get_vertex_attr(&clip_tile_copy_program.program, "TileIndex").unwrap();

        device.bind_buffer(&vertex_array, quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &tile_offset_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: 4,
            offset: 0,
            divisor: 0,
            buffer_index: 0,
        });
        device.bind_buffer(&vertex_array, &vertex_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &tile_index_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I32,
            stride: CLIP_TILE_INSTANCE_SIZE / 2,
            offset: 0,
            divisor: 1,
            buffer_index: 1,
        });
        device.bind_buffer(&vertex_array, quad_vertex_indices_buffer, BufferTarget::Index);

        ClipTileCopyVertexArrayD3D9 { vertex_array }
    }
}

pub(crate) struct ClipTileCombineVertexArrayD3D9<D> where D: Device {
    pub(crate) vertex_array: D::VertexArray,
}

impl<D> ClipTileCombineVertexArrayD3D9<D> where D: Device {
    pub(crate) fn new(device: &D,
                      clip_tile_combine_program: &ClipTileCombineProgramD3D9<D>,
                      vertex_buffer: &D::Buffer,
                      quad_vertex_positions_buffer: &D::Buffer,
                      quad_vertex_indices_buffer: &D::Buffer)
                      -> ClipTileCombineVertexArrayD3D9<D> {
        let vertex_array = device.create_vertex_array();

        let tile_offset_attr =
            device.get_vertex_attr(&clip_tile_combine_program.program, "TileOffset").unwrap();
        let dest_tile_index_attr =
            device.get_vertex_attr(&clip_tile_combine_program.program, "DestTileIndex").unwrap();
        let dest_backdrop_attr =
            device.get_vertex_attr(&clip_tile_combine_program.program, "DestBackdrop").unwrap();
        let src_tile_index_attr =
            device.get_vertex_attr(&clip_tile_combine_program.program, "SrcTileIndex").unwrap();
        let src_backdrop_attr =
            device.get_vertex_attr(&clip_tile_combine_program.program, "SrcBackdrop").unwrap();

        device.bind_buffer(&vertex_array, quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &tile_offset_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: 4,
            offset: 0,
            divisor: 0,
            buffer_index: 0,
        });
        device.bind_buffer(&vertex_array, &vertex_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &dest_tile_index_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I32,
            stride: CLIP_TILE_INSTANCE_SIZE,
            offset: 0,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &dest_backdrop_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I32,
            stride: CLIP_TILE_INSTANCE_SIZE,
            offset: 4,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &src_tile_index_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I32,
            stride: CLIP_TILE_INSTANCE_SIZE,
            offset: 8,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &src_backdrop_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I32,
            stride: CLIP_TILE_INSTANCE_SIZE,
            offset: 12,
            divisor: 1,
            buffer_index: 1,
        });
        device.bind_buffer(&vertex_array, quad_vertex_indices_buffer, BufferTarget::Index);

        ClipTileCombineVertexArrayD3D9 { vertex_array }
    }
}

pub(crate) struct CopyTileVertexArray<D> where D: Device {
    pub(crate) vertex_array: D::VertexArray,
}

impl<D> CopyTileVertexArray<D> where D: Device {
    pub(crate) fn new(device: &D,
                      copy_tile_program: &CopyTileProgram<D>,
                      copy_tile_vertex_buffer: &D::Buffer,
                      quads_vertex_indices_buffer: &D::Buffer)
                      -> CopyTileVertexArray<D> {
        let vertex_array = device.create_vertex_array();

        let tile_position_attr =
            device.get_vertex_attr(&copy_tile_program.program, "TilePosition").unwrap();

        device.bind_buffer(&vertex_array, copy_tile_vertex_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &tile_position_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: TILE_INSTANCE_SIZE,
            offset: 0,
            divisor: 0,
            buffer_index: 0,
        });
        device.bind_buffer(&vertex_array, quads_vertex_indices_buffer, BufferTarget::Index);

        CopyTileVertexArray { vertex_array }
    }
}

pub(crate) struct FillProgramD3D9<D> where D: Device {
    pub(crate) program: D::Program,
    pub(crate) framebuffer_size_uniform: D::Uniform,
    pub(crate) tile_size_uniform: D::Uniform,
    pub(crate) area_lut_texture: D::TextureParameter,
}

impl<D> FillProgramD3D9<D> where D: Device {
    fn new(device: &D, resources: &dyn ResourceLoader) -> FillProgramD3D9<D> {
        let program = device.create_raster_program(resources, "d3d9/fill");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let tile_size_uniform = device.get_uniform(&program, "TileSize");
        let area_lut_texture = device.get_texture_parameter(&program, "AreaLUT");
        FillProgramD3D9 {
            program,
            framebuffer_size_uniform,
            tile_size_uniform,
            area_lut_texture,
        }
    }
}

pub(crate) struct TileProgramD3D9<D> where D: Device {
    pub(crate) common: TileProgramCommon<D>,
    pub(crate) dest_texture: D::TextureParameter,
    pub(crate) transform_uniform: D::Uniform,
}

impl<D> TileProgramD3D9<D> where D: Device {
    fn new(device: &D, resources: &dyn ResourceLoader) -> TileProgramD3D9<D> {
        let program = device.create_raster_program(resources, "d3d9/tile");
        let dest_texture = device.get_texture_parameter(&program, "DestTexture");
        let transform_uniform = device.get_uniform(&program, "Transform");
        let common = TileProgramCommon::new(device, program);
        TileProgramD3D9 { common, dest_texture, transform_uniform }
    }
}

pub(crate) struct ClipTileCombineProgramD3D9<D> where D: Device {
    pub(crate) program: D::Program,
    pub(crate) src_texture: D::TextureParameter,
    pub(crate) framebuffer_size_uniform: D::Uniform,
}

impl<D> ClipTileCombineProgramD3D9<D> where D: Device {
    pub(crate) fn new(device: &D, resources: &dyn ResourceLoader)
                      -> ClipTileCombineProgramD3D9<D> {
        let program = device.create_raster_program(resources, "d3d9/tile_clip_combine");
        let src_texture = device.get_texture_parameter(&program, "Src");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        ClipTileCombineProgramD3D9 { program, src_texture, framebuffer_size_uniform }
    }
}

pub(crate) struct ClipTileCopyProgramD3D9<D> where D: Device {
    pub(crate) program: D::Program,
    pub(crate) src_texture: D::TextureParameter,
    pub(crate) framebuffer_size_uniform: D::Uniform,
}

impl<D> ClipTileCopyProgramD3D9<D> where D: Device {
    pub(crate) fn new(device: &D, resources: &dyn ResourceLoader) -> ClipTileCopyProgramD3D9<D> {
        let program = device.create_raster_program(resources, "d3d9/tile_clip_copy");
        let src_texture = device.get_texture_parameter(&program, "Src");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        ClipTileCopyProgramD3D9 { program, src_texture, framebuffer_size_uniform }
    }
}

pub(crate) struct CopyTileProgram<D> where D: Device {
    pub(crate) program: D::Program,
    pub(crate) transform_uniform: D::Uniform,
    pub(crate) tile_size_uniform: D::Uniform,
    pub(crate) framebuffer_size_uniform: D::Uniform,
    pub(crate) src_texture: D::TextureParameter,
}

impl<D> CopyTileProgram<D> where D: Device {
    pub(crate) fn new(device: &D, resources: &dyn ResourceLoader) -> CopyTileProgram<D> {
        let program = device.create_raster_program(resources, "d3d9/tile_copy");
        let transform_uniform = device.get_uniform(&program, "Transform");
        let tile_size_uniform = device.get_uniform(&program, "TileSize");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let src_texture = device.get_texture_parameter(&program, "Src");
        CopyTileProgram {
            program,
            transform_uniform,
            tile_size_uniform,
            framebuffer_size_uniform,
            src_texture,
        }
    }
}

pub(crate) struct ProgramsD3D9<D> where D: Device {
    pub(crate) fill_program: FillProgramD3D9<D>,
    pub(crate) tile_program: TileProgramD3D9<D>,
    pub(crate) tile_clip_copy_program: ClipTileCopyProgramD3D9<D>,
    pub(crate) tile_clip_combine_program: ClipTileCombineProgramD3D9<D>,
    pub(crate) tile_copy_program: CopyTileProgram<D>,
}

impl<D> ProgramsD3D9<D> where D: Device {
    pub(crate) fn new(device: &D, resources: &dyn ResourceLoader) -> ProgramsD3D9<D> {
        ProgramsD3D9 {
            fill_program: FillProgramD3D9::new(device, resources),
            tile_program: TileProgramD3D9::new(device, resources),
            tile_clip_copy_program: ClipTileCopyProgramD3D9::new(device, resources),
            tile_clip_combine_program: ClipTileCombineProgramD3D9::new(device, resources),
            tile_copy_program: CopyTileProgram::new(device, resources),
        }
    }
}
