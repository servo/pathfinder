// pathfinder/renderer/src/gpu/shaders.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::gpu::options::RendererOptions;
use crate::gpu::renderer::{MASK_TILES_ACROSS, MASK_TILES_DOWN};
use crate::tiles::{TILE_HEIGHT, TILE_WIDTH};
use pathfinder_gpu::{BufferTarget, BufferUploadMode, ComputeDimensions, Device, FeatureLevel};
use pathfinder_gpu::{VertexAttrClass, VertexAttrDescriptor, VertexAttrType};
use pathfinder_resources::ResourceLoader;

// TODO(pcwalton): Replace with `mem::size_of` calls?
pub(crate) const TILE_INSTANCE_SIZE: usize = 12;
const FILL_INSTANCE_SIZE: usize = 8;
const CLIP_TILE_INSTANCE_SIZE: usize = 8;

pub const MAX_FILLS_PER_BATCH: usize = 0x10000;
pub const MAX_TILES_PER_BATCH: usize = MASK_TILES_ACROSS as usize * MASK_TILES_DOWN as usize;

pub struct BlitVertexArray<D> where D: Device {
    pub vertex_array: D::VertexArray,
}

impl<D> BlitVertexArray<D> where D: Device {
    pub fn new(device: &D,
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

pub struct ClearVertexArray<D> where D: Device {
    pub vertex_array: D::VertexArray,
}

impl<D> ClearVertexArray<D> where D: Device {
    pub fn new(device: &D,
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

pub struct FillVertexArray<D> where D: Device {
    pub vertex_array: D::VertexArray,
}

impl<D> FillVertexArray<D>
where
    D: Device,
{
    pub fn new(
        device: &D,
        fill_program: &FillRasterProgram<D>,
        vertex_buffer: &D::Buffer,
        quad_vertex_positions_buffer: &D::Buffer,
        quad_vertex_indices_buffer: &D::Buffer,
    ) -> FillVertexArray<D> {
        let vertex_array = device.create_vertex_array();

        let tess_coord_attr = device.get_vertex_attr(&fill_program.program, "TessCoord").unwrap();
        let from_px_attr = device.get_vertex_attr(&fill_program.program, "FromPx").unwrap();
        let to_px_attr = device.get_vertex_attr(&fill_program.program, "ToPx").unwrap();
        let from_subpx_attr = device.get_vertex_attr(&fill_program.program, "FromSubpx").unwrap();
        let to_subpx_attr = device.get_vertex_attr(&fill_program.program, "ToSubpx").unwrap();
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
        device.configure_vertex_attr(&vertex_array, &from_subpx_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::FloatNorm,
            attr_type: VertexAttrType::U8,
            stride: FILL_INSTANCE_SIZE,
            offset: 0,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &to_subpx_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::FloatNorm,
            attr_type: VertexAttrType::U8,
            stride: FILL_INSTANCE_SIZE,
            offset: 2,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &from_px_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U8,
            stride: FILL_INSTANCE_SIZE,
            offset: 4,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &to_px_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U8,
            stride: FILL_INSTANCE_SIZE,
            offset: 5,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &tile_index_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U16,
            stride: FILL_INSTANCE_SIZE,
            offset: 6,
            divisor: 1,
            buffer_index: 1,
        });
        device.bind_buffer(&vertex_array, quad_vertex_indices_buffer, BufferTarget::Index);

        FillVertexArray { vertex_array }
    }
}

pub struct TileVertexArray<D> where D: Device {
    pub vertex_array: D::VertexArray,
}

impl<D> TileVertexArray<D> where D: Device {
    pub fn new(device: &D,
               tile_program: &TileProgram<D>,
               tile_vertex_buffer: &D::Buffer,
               quad_vertex_positions_buffer: &D::Buffer,
               quad_vertex_indices_buffer: &D::Buffer)
               -> TileVertexArray<D> {
        let vertex_array = device.create_vertex_array();

        let tile_offset_attr =
            device.get_vertex_attr(&tile_program.program, "TileOffset").unwrap();
        let tile_origin_attr =
            device.get_vertex_attr(&tile_program.program, "TileOrigin").unwrap();
        let mask_0_tex_coord_attr =
            device.get_vertex_attr(&tile_program.program, "MaskTexCoord0").unwrap();
        let mask_backdrop_attr =
            device.get_vertex_attr(&tile_program.program, "MaskBackdrop").unwrap();
        let color_attr = device.get_vertex_attr(&tile_program.program, "Color").unwrap();
        let tile_ctrl_attr = device.get_vertex_attr(&tile_program.program, "TileCtrl").unwrap();

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
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U8,
            stride: TILE_INSTANCE_SIZE,
            offset: 4,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &mask_backdrop_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I8,
            stride: TILE_INSTANCE_SIZE,
            offset: 6,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &color_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: TILE_INSTANCE_SIZE,
            offset: 8,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &tile_ctrl_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: TILE_INSTANCE_SIZE,
            offset: 10,
            divisor: 1,
            buffer_index: 1,
        });
        device.bind_buffer(&vertex_array, quad_vertex_indices_buffer, BufferTarget::Index);

        TileVertexArray { vertex_array }
    }
}

pub struct CopyTileVertexArray<D> where D: Device {
    pub vertex_array: D::VertexArray,
}

impl<D> CopyTileVertexArray<D> where D: Device {
    pub fn new(
        device: &D,
        copy_tile_program: &CopyTileProgram<D>,
        copy_tile_vertex_buffer: &D::Buffer,
        quads_vertex_indices_buffer: &D::Buffer,
    ) -> CopyTileVertexArray<D> {
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

pub struct ClipTileVertexArray<D> where D: Device {
    pub vertex_array: D::VertexArray,
    pub vertex_buffer: D::Buffer,
}

impl<D> ClipTileVertexArray<D> where D: Device {
    pub fn new(device: &D,
               clip_tile_program: &ClipTileProgram<D>,
               quad_vertex_positions_buffer: &D::Buffer,
               quad_vertex_indices_buffer: &D::Buffer)
               -> ClipTileVertexArray<D> {
        let vertex_array = device.create_vertex_array();
        let vertex_buffer = device.create_buffer(BufferUploadMode::Dynamic);

        let tile_offset_attr =
            device.get_vertex_attr(&clip_tile_program.program, "TileOffset").unwrap();
        let dest_tile_origin_attr =
            device.get_vertex_attr(&clip_tile_program.program, "DestTileOrigin").unwrap();
        let src_tile_origin_attr =
            device.get_vertex_attr(&clip_tile_program.program, "SrcTileOrigin").unwrap();
        let src_backdrop_attr =
            device.get_vertex_attr(&clip_tile_program.program, "SrcBackdrop").unwrap();

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
        device.configure_vertex_attr(&vertex_array, &dest_tile_origin_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U8,
            stride: CLIP_TILE_INSTANCE_SIZE,
            offset: 0,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &src_tile_origin_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U8,
            stride: CLIP_TILE_INSTANCE_SIZE,
            offset: 2,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &src_backdrop_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I8,
            stride: CLIP_TILE_INSTANCE_SIZE,
            offset: 4,
            divisor: 1,
            buffer_index: 1,
        });
        device.bind_buffer(&vertex_array, quad_vertex_indices_buffer, BufferTarget::Index);

        ClipTileVertexArray { vertex_array, vertex_buffer }
    }
}


pub struct BlitProgram<D> where D: Device {
    pub program: D::Program,
    pub src_texture: D::TextureParameter,
}

impl<D> BlitProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> BlitProgram<D> {
        let program = device.create_raster_program(resources, "blit");
        let src_texture = device.get_texture_parameter(&program, "Src");
        BlitProgram { program, src_texture }
    }
}

pub struct ClearProgram<D> where D: Device {
    pub program: D::Program,
    pub rect_uniform: D::Uniform,
    pub framebuffer_size_uniform: D::Uniform,
    pub color_uniform: D::Uniform,
}

impl<D> ClearProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> ClearProgram<D> {
        let program = device.create_raster_program(resources, "clear");
        let rect_uniform = device.get_uniform(&program, "Rect");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let color_uniform = device.get_uniform(&program, "Color");
        ClearProgram { program, rect_uniform, framebuffer_size_uniform, color_uniform }
    }
}

pub enum FillProgram<D> where D: Device {
    Raster(FillRasterProgram<D>),
    Compute(FillComputeProgram<D>),
}

impl<D> FillProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader, options: &RendererOptions)
               -> FillProgram<D> {
        match (options.no_compute, device.feature_level()) {
            (false, FeatureLevel::D3D11) => {
                FillProgram::Compute(FillComputeProgram::new(device, resources))
            }
            (_, FeatureLevel::D3D10) | (true, _) => {
                FillProgram::Raster(FillRasterProgram::new(device, resources))
            }
        }
    }
}

pub struct FillRasterProgram<D> where D: Device {
    pub program: D::Program,
    pub framebuffer_size_uniform: D::Uniform,
    pub tile_size_uniform: D::Uniform,
    pub area_lut_texture: D::TextureParameter,
}

impl<D> FillRasterProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> FillRasterProgram<D> {
        let program = device.create_raster_program(resources, "fill");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let tile_size_uniform = device.get_uniform(&program, "TileSize");
        let area_lut_texture = device.get_texture_parameter(&program, "AreaLUT");
        FillRasterProgram {
            program,
            framebuffer_size_uniform,
            tile_size_uniform,
            area_lut_texture,
        }
    }
}

pub struct FillComputeProgram<D> where D: Device {
    pub program: D::Program,
    pub dest_image: D::ImageParameter,
    pub area_lut_texture: D::TextureParameter,
    pub first_tile_index_uniform: D::Uniform,
    pub fills_storage_buffer: D::StorageBuffer,
    pub next_fills_storage_buffer: D::StorageBuffer,
    pub fill_tile_map_storage_buffer: D::StorageBuffer,
}

impl<D> FillComputeProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> FillComputeProgram<D> {
        let mut program = device.create_compute_program(resources, "fill");
        let local_size = ComputeDimensions { x: TILE_WIDTH, y: TILE_HEIGHT / 4, z: 1 };
        device.set_compute_program_local_size(&mut program, local_size);

        let dest_image = device.get_image_parameter(&program, "Dest");
        let area_lut_texture = device.get_texture_parameter(&program, "AreaLUT");
        let first_tile_index_uniform = device.get_uniform(&program, "FirstTileIndex");
        let fills_storage_buffer = device.get_storage_buffer(&program, "Fills", 0);
        let next_fills_storage_buffer = device.get_storage_buffer(&program, "NextFills", 1);
        let fill_tile_map_storage_buffer = device.get_storage_buffer(&program, "FillTileMap", 2);

        FillComputeProgram {
            program,
            dest_image,
            area_lut_texture,
            first_tile_index_uniform,
            fills_storage_buffer,
            next_fills_storage_buffer,
            fill_tile_map_storage_buffer,
        }
    }
}

pub struct TileProgram<D> where D: Device {
    pub program: D::Program,
    pub transform_uniform: D::Uniform,
    pub tile_size_uniform: D::Uniform,
    pub texture_metadata_texture: D::TextureParameter,
    pub texture_metadata_size_uniform: D::Uniform,
    pub dest_texture: D::TextureParameter,
    pub color_texture_0: D::TextureParameter,
    pub color_texture_size_0_uniform: D::Uniform,
    pub color_texture_1: D::TextureParameter,
    pub mask_texture_0: D::TextureParameter,
    pub mask_texture_size_0_uniform: D::Uniform,
    pub gamma_lut_texture: D::TextureParameter,
    pub filter_params_0_uniform: D::Uniform,
    pub filter_params_1_uniform: D::Uniform,
    pub filter_params_2_uniform: D::Uniform,
    pub framebuffer_size_uniform: D::Uniform,
    pub ctrl_uniform: D::Uniform,
}

impl<D> TileProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> TileProgram<D> {
        let program = device.create_raster_program(resources, "tile");
        let transform_uniform = device.get_uniform(&program, "Transform");
        let tile_size_uniform = device.get_uniform(&program, "TileSize");
        let texture_metadata_texture = device.get_texture_parameter(&program, "TextureMetadata");
        let texture_metadata_size_uniform = device.get_uniform(&program, "TextureMetadataSize");
        let dest_texture = device.get_texture_parameter(&program, "DestTexture");
        let color_texture_0 = device.get_texture_parameter(&program, "ColorTexture0");
        let color_texture_size_0_uniform = device.get_uniform(&program, "ColorTextureSize0");
        let color_texture_1 = device.get_texture_parameter(&program, "ColorTexture1");
        let mask_texture_0 = device.get_texture_parameter(&program, "MaskTexture0");
        let mask_texture_size_0_uniform = device.get_uniform(&program, "MaskTextureSize0");
        let gamma_lut_texture = device.get_texture_parameter(&program, "GammaLUT");
        let filter_params_0_uniform = device.get_uniform(&program, "FilterParams0");
        let filter_params_1_uniform = device.get_uniform(&program, "FilterParams1");
        let filter_params_2_uniform = device.get_uniform(&program, "FilterParams2");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let ctrl_uniform = device.get_uniform(&program, "Ctrl");
        TileProgram {
            program,
            transform_uniform,
            tile_size_uniform,
            texture_metadata_texture,
            texture_metadata_size_uniform,
            dest_texture,
            color_texture_0,
            color_texture_size_0_uniform,
            color_texture_1,
            mask_texture_0,
            mask_texture_size_0_uniform,
            gamma_lut_texture,
            filter_params_0_uniform,
            filter_params_1_uniform,
            filter_params_2_uniform,
            framebuffer_size_uniform,
            ctrl_uniform,
        }
    }
}

pub struct CopyTileProgram<D> where D: Device {
    pub program: D::Program,
    pub transform_uniform: D::Uniform,
    pub tile_size_uniform: D::Uniform,
    pub framebuffer_size_uniform: D::Uniform,
    pub src_texture: D::TextureParameter,
}

impl<D> CopyTileProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> CopyTileProgram<D> {
        let program = device.create_raster_program(resources, "tile_copy");
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

pub struct ClipTileProgram<D> where D: Device {
    pub program: D::Program,
    pub src_texture: D::TextureParameter,
}

impl<D> ClipTileProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> ClipTileProgram<D> {
        let program = device.create_raster_program(resources, "tile_clip");
        let src_texture = device.get_texture_parameter(&program, "Src");
        ClipTileProgram { program, src_texture }
    }
}

pub struct StencilProgram<D>
where
    D: Device,
{
    pub program: D::Program,
}

impl<D> StencilProgram<D>
where
    D: Device,
{
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> StencilProgram<D> {
        let program = device.create_raster_program(resources, "stencil");
        StencilProgram { program }
    }
}

pub struct StencilVertexArray<D>
where
    D: Device,
{
    pub vertex_array: D::VertexArray,
    pub vertex_buffer: D::Buffer,
    pub index_buffer: D::Buffer,
}

impl<D> StencilVertexArray<D>
where
    D: Device,
{
    pub fn new(device: &D, stencil_program: &StencilProgram<D>) -> StencilVertexArray<D> {
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

pub struct ReprojectionProgram<D> where D: Device {
    pub program: D::Program,
    pub old_transform_uniform: D::Uniform,
    pub new_transform_uniform: D::Uniform,
    pub texture: D::TextureParameter,
}

impl<D> ReprojectionProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> ReprojectionProgram<D> {
        let program = device.create_raster_program(resources, "reproject");
        let old_transform_uniform = device.get_uniform(&program, "OldTransform");
        let new_transform_uniform = device.get_uniform(&program, "NewTransform");
        let texture = device.get_texture_parameter(&program, "Texture");
        ReprojectionProgram { program, old_transform_uniform, new_transform_uniform, texture }
    }
}

pub struct ReprojectionVertexArray<D>
where
    D: Device,
{
    pub vertex_array: D::VertexArray,
}

impl<D> ReprojectionVertexArray<D>
where
    D: Device,
{
    pub fn new(
        device: &D,
        reprojection_program: &ReprojectionProgram<D>,
        quad_vertex_positions_buffer: &D::Buffer,
        quad_vertex_indices_buffer: &D::Buffer,
    ) -> ReprojectionVertexArray<D> {
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
