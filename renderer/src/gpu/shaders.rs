// pathfinder/renderer/src/gpu/shaders.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::gpu_data::FillBatchPrimitive;
use pathfinder_content::fill::FillRule;
use pathfinder_gpu::{BufferData, BufferTarget, BufferUploadMode, Device, VertexAttrClass};
use pathfinder_gpu::{VertexAttrDescriptor, VertexAttrType};
use pathfinder_gpu::resources::ResourceLoader;

// TODO(pcwalton): Replace with `mem::size_of` calls?
const FILL_INSTANCE_SIZE: usize = 8;
const SOLID_TILE_VERTEX_SIZE: usize = 12;
const ALPHA_TILE_VERTEX_SIZE: usize = 16;
const MASK_TILE_VERTEX_SIZE: usize = 12;

pub const MAX_FILLS_PER_BATCH: usize = 0x4000;

pub struct FillVertexArray<D>
where
    D: Device,
{
    pub vertex_array: D::VertexArray,
    pub vertex_buffer: D::Buffer,
}

impl<D> FillVertexArray<D>
where
    D: Device,
{
    pub fn new(
        device: &D,
        fill_program: &FillProgram<D>,
        quad_vertex_positions_buffer: &D::Buffer,
        quad_vertex_indices_buffer: &D::Buffer,
    ) -> FillVertexArray<D> {
        let vertex_array = device.create_vertex_array();

        let vertex_buffer = device.create_buffer();
        let vertex_buffer_data: BufferData<FillBatchPrimitive> =
            BufferData::Uninitialized(MAX_FILLS_PER_BATCH);
        device.allocate_buffer(
            &vertex_buffer,
            vertex_buffer_data,
            BufferTarget::Vertex,
            BufferUploadMode::Dynamic,
        );

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
        device.configure_vertex_attr(&vertex_array, &from_px_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U8,
            stride: FILL_INSTANCE_SIZE,
            offset: 0,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &to_px_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U8,
            stride: FILL_INSTANCE_SIZE,
            offset: 1,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &from_subpx_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::FloatNorm,
            attr_type: VertexAttrType::U8,
            stride: FILL_INSTANCE_SIZE,
            offset: 2,
            divisor: 1,
            buffer_index: 1,
        });
        device.configure_vertex_attr(&vertex_array, &to_subpx_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::FloatNorm,
            attr_type: VertexAttrType::U8,
            stride: FILL_INSTANCE_SIZE,
            offset: 4,
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

        FillVertexArray { vertex_array, vertex_buffer }
    }
}

pub struct MaskTileVertexArray<D> where D: Device {
    pub vertex_array: D::VertexArray,
    pub vertex_buffer: D::Buffer,
}

impl<D> MaskTileVertexArray<D> where D: Device {
    pub fn new(device: &D,
               mask_tile_program: &MaskTileProgram<D>,
               quads_vertex_indices_buffer: &D::Buffer)
               -> MaskTileVertexArray<D> {
        let (vertex_array, vertex_buffer) = (device.create_vertex_array(), device.create_buffer());

        let position_attr = device.get_vertex_attr(&mask_tile_program.program, "Position")
                                  .unwrap();
        let fill_tex_coord_attr = device.get_vertex_attr(&mask_tile_program.program,
                                                         "FillTexCoord").unwrap();
        let backdrop_attr = device.get_vertex_attr(&mask_tile_program.program, "Backdrop")
                                  .unwrap();

        device.bind_buffer(&vertex_array, &vertex_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &position_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::FloatNorm,
            attr_type: VertexAttrType::U16,
            stride: MASK_TILE_VERTEX_SIZE,
            offset: 0,
            divisor: 0,
            buffer_index: 0,
        });
        device.configure_vertex_attr(&vertex_array, &fill_tex_coord_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::FloatNorm,
            attr_type: VertexAttrType::U16,
            stride: MASK_TILE_VERTEX_SIZE,
            offset: 4,
            divisor: 0,
            buffer_index: 0,
        });
        device.configure_vertex_attr(&vertex_array, &backdrop_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: MASK_TILE_VERTEX_SIZE,
            offset: 8,
            divisor: 0,
            buffer_index: 0,
        });
        device.bind_buffer(&vertex_array, quads_vertex_indices_buffer, BufferTarget::Index);

        MaskTileVertexArray { vertex_array, vertex_buffer }
    }
}

pub struct AlphaTileVertexArray<D> where D: Device {
    pub vertex_array: D::VertexArray,
}

impl<D> AlphaTileVertexArray<D> where D: Device {
    pub fn new(
        device: &D,
        alpha_tile_program: &AlphaTileProgram<D>,
        alpha_tile_vertex_buffer: &D::Buffer,
        quads_vertex_indices_buffer: &D::Buffer,
    ) -> AlphaTileVertexArray<D> {
        let vertex_array = device.create_vertex_array();

        let tile_position_attr =
            device.get_vertex_attr(&alpha_tile_program.program, "TilePosition").unwrap();
        let color_tex_coord_attr = device.get_vertex_attr(&alpha_tile_program.program,
                                                          "ColorTexCoord").unwrap();
        let mask_tex_coord_attr = device.get_vertex_attr(&alpha_tile_program.program,
                                                         "MaskTexCoord").unwrap();

        device.bind_buffer(&vertex_array, alpha_tile_vertex_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &tile_position_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: ALPHA_TILE_VERTEX_SIZE,
            offset: 0,
            divisor: 0,
            buffer_index: 0,
        });
        device.configure_vertex_attr(&vertex_array, &mask_tex_coord_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::FloatNorm,
            attr_type: VertexAttrType::U16,
            stride: ALPHA_TILE_VERTEX_SIZE,
            offset: 4,
            divisor: 0,
            buffer_index: 0,
        });
        device.configure_vertex_attr(&vertex_array, &color_tex_coord_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::FloatNorm,
            attr_type: VertexAttrType::U16,
            stride: ALPHA_TILE_VERTEX_SIZE,
            offset: 8,
            divisor: 0,
            buffer_index: 0,
        });
        device.bind_buffer(&vertex_array, quads_vertex_indices_buffer, BufferTarget::Index);

        AlphaTileVertexArray { vertex_array }
    }
}

pub struct SolidTileVertexArray<D>
where
    D: Device,
{
    pub vertex_array: D::VertexArray,
    pub vertex_buffer: D::Buffer,
}

impl<D> SolidTileVertexArray<D>
where
    D: Device,
{
    pub fn new(
        device: &D,
        solid_tile_program: &SolidTileProgram<D>,
        quads_vertex_indices_buffer: &D::Buffer,
    ) -> SolidTileVertexArray<D> {
        let (vertex_array, vertex_buffer) = (device.create_vertex_array(), device.create_buffer());

        let tile_position_attr =
            device.get_vertex_attr(&solid_tile_program.program, "TilePosition").unwrap();
        let color_tex_coord_attr =
            device.get_vertex_attr(&solid_tile_program.program, "ColorTexCoord").unwrap();

        // NB: The tile origin must be of type short, not unsigned short, to work around a macOS
        // Radeon driver bug.
        device.bind_buffer(&vertex_array, &vertex_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&vertex_array, &tile_position_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: SOLID_TILE_VERTEX_SIZE,
            offset: 0,
            divisor: 0,
            buffer_index: 0,
        });
        device.configure_vertex_attr(&vertex_array,
                                     &color_tex_coord_attr,
                                     &VertexAttrDescriptor {
                                        size: 2,
                                        class: VertexAttrClass::FloatNorm,
                                        attr_type: VertexAttrType::U16,
                                        stride: SOLID_TILE_VERTEX_SIZE,
                                        offset: 4,
                                        divisor: 0,
                                        buffer_index: 0,
                                     });
        device.bind_buffer(&vertex_array, quads_vertex_indices_buffer, BufferTarget::Index);

        SolidTileVertexArray { vertex_array, vertex_buffer }
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
            stride: ALPHA_TILE_VERTEX_SIZE,
            offset: 0,
            divisor: 0,
            buffer_index: 0,
        });
        device.bind_buffer(&vertex_array, quads_vertex_indices_buffer, BufferTarget::Index);

        CopyTileVertexArray { vertex_array }
    }
}

pub struct FillProgram<D>
where
    D: Device,
{
    pub program: D::Program,
    pub framebuffer_size_uniform: D::Uniform,
    pub tile_size_uniform: D::Uniform,
    pub area_lut_uniform: D::Uniform,
}

impl<D> FillProgram<D>
where
    D: Device,
{
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> FillProgram<D> {
        let program = device.create_program(resources, "fill");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let tile_size_uniform = device.get_uniform(&program, "TileSize");
        let area_lut_uniform = device.get_uniform(&program, "AreaLUT");
        FillProgram {
            program,
            framebuffer_size_uniform,
            tile_size_uniform,
            area_lut_uniform,
        }
    }
}

pub struct MaskTileProgram<D> where D: Device {
    pub program: D::Program,
    pub fill_texture_uniform: D::Uniform,
}

impl<D> MaskTileProgram<D> where D: Device {
    pub fn new(fill_rule: FillRule, device: &D, resources: &dyn ResourceLoader)
               -> MaskTileProgram<D> {
        let program_name = match fill_rule {
            FillRule::Winding => "mask_winding",
            FillRule::EvenOdd => "mask_evenodd",
        };

        let program = device.create_program_from_shader_names(resources,
                                                              program_name,
                                                              "mask",
                                                              program_name);

        let fill_texture_uniform = device.get_uniform(&program, "FillTexture");
        MaskTileProgram { program, fill_texture_uniform }
    }
}

pub struct SolidTileProgram<D> where D: Device {
    pub program: D::Program,
    pub transform_uniform: D::Uniform,
    pub tile_size_uniform: D::Uniform,
    pub paint_texture_uniform: D::Uniform,
}

impl<D> SolidTileProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> SolidTileProgram<D> {
        let program = device.create_program(resources, "tile_solid");
        let transform_uniform = device.get_uniform(&program, "Transform");
        let tile_size_uniform = device.get_uniform(&program, "TileSize");
        let paint_texture_uniform = device.get_uniform(&program, "PaintTexture");
        SolidTileProgram {
            program,
            transform_uniform,
            tile_size_uniform,
            paint_texture_uniform,
        }
    }
}

pub struct AlphaTileProgram<D> where D: Device {
    pub program: D::Program,
    pub transform_uniform: D::Uniform,
    pub tile_size_uniform: D::Uniform,
    pub framebuffer_size_uniform: D::Uniform,
    pub stencil_texture_uniform: D::Uniform,
    pub paint_texture_uniform: D::Uniform,
}

impl<D> AlphaTileProgram<D> where D: Device {
    #[inline]
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> AlphaTileProgram<D> {
        AlphaTileProgram::from_fragment_shader_name(device, resources, "tile_alpha")
    }

    fn from_fragment_shader_name(device: &D,
                                 resources: &dyn ResourceLoader,
                                 fragment_shader_name: &str)
                                 -> AlphaTileProgram<D> {
        let program = device.create_program_from_shader_names(resources,
                                                              fragment_shader_name,
                                                              "tile_alpha",
                                                              fragment_shader_name);
        let transform_uniform = device.get_uniform(&program, "Transform");
        let tile_size_uniform = device.get_uniform(&program, "TileSize");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let stencil_texture_uniform = device.get_uniform(&program, "StencilTexture");
        let paint_texture_uniform = device.get_uniform(&program, "PaintTexture");
        AlphaTileProgram {
            program,
            transform_uniform,
            tile_size_uniform,
            framebuffer_size_uniform,
            stencil_texture_uniform,
            paint_texture_uniform,
        }
    }
}

pub struct CopyTileProgram<D> where D: Device {
    pub program: D::Program,
    pub transform_uniform: D::Uniform,
    pub tile_size_uniform: D::Uniform,
    pub framebuffer_size_uniform: D::Uniform,
    pub src_uniform: D::Uniform,
}

impl<D> CopyTileProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> CopyTileProgram<D> {
        let program = device.create_program(resources, "tile_copy");
        let transform_uniform = device.get_uniform(&program, "Transform");
        let tile_size_uniform = device.get_uniform(&program, "TileSize");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let src_uniform = device.get_uniform(&program, "Src");
        CopyTileProgram {
            program,
            transform_uniform,
            tile_size_uniform,
            framebuffer_size_uniform,
            src_uniform,
        }
    }
}

pub struct AlphaTileHSLProgram<D> where D: Device {
    pub alpha_tile_program: AlphaTileProgram<D>,
    pub dest_uniform: D::Uniform,
    pub blend_hsl_uniform: D::Uniform,
}

impl<D> AlphaTileHSLProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> AlphaTileHSLProgram<D> {
        let alpha_tile_program = AlphaTileProgram::from_fragment_shader_name(device,
                                                                             resources,
                                                                             "tile_alpha_hsl");
        let dest_uniform = device.get_uniform(&alpha_tile_program.program, "Dest");
        let blend_hsl_uniform = device.get_uniform(&alpha_tile_program.program, "BlendHSL");
        AlphaTileHSLProgram { alpha_tile_program, dest_uniform, blend_hsl_uniform }
    }
}

pub struct AlphaTileOverlayProgram<D> where D: Device {
    pub alpha_tile_program: AlphaTileProgram<D>,
    pub dest_uniform: D::Uniform,
    pub blend_mode_uniform: D::Uniform,
}

impl<D> AlphaTileOverlayProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> AlphaTileOverlayProgram<D> {
        let alpha_tile_program = AlphaTileProgram::from_fragment_shader_name(device,
                                                                             resources,
                                                                             "tile_alpha_overlay");
        let dest_uniform = device.get_uniform(&alpha_tile_program.program, "Dest");
        let blend_mode_uniform = device.get_uniform(&alpha_tile_program.program, "BlendMode");
        AlphaTileOverlayProgram { alpha_tile_program, dest_uniform, blend_mode_uniform }
    }
}

pub struct AlphaTileDodgeBurnProgram<D> where D: Device {
    pub alpha_tile_program: AlphaTileProgram<D>,
    pub dest_uniform: D::Uniform,
    pub burn_uniform: D::Uniform,
}

impl<D> AlphaTileDodgeBurnProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> AlphaTileDodgeBurnProgram<D> {
        let alpha_tile_program =
            AlphaTileProgram::from_fragment_shader_name(device, resources, "tile_alpha_dodgeburn");
        let dest_uniform = device.get_uniform(&alpha_tile_program.program, "Dest");
        let burn_uniform = device.get_uniform(&alpha_tile_program.program, "Burn");
        AlphaTileDodgeBurnProgram { alpha_tile_program, dest_uniform, burn_uniform }
    }
}

pub struct FilterBasicProgram<D> where D: Device {
    pub program: D::Program,
    pub source_uniform: D::Uniform,
    pub framebuffer_size_uniform: D::Uniform,
}

impl<D> FilterBasicProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> FilterBasicProgram<D> {
        let program = device.create_program_from_shader_names(resources,
                                                              "filter_basic",
                                                              "filter",
                                                              "filter_basic");
        let source_uniform = device.get_uniform(&program, "Source");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        FilterBasicProgram { program, source_uniform, framebuffer_size_uniform }
    }
}

pub struct FilterBasicVertexArray<D> where D: Device {
    pub vertex_array: D::VertexArray,
}

impl<D> FilterBasicVertexArray<D> where D: Device {
    pub fn new(
        device: &D,
        fill_basic_program: &FilterBasicProgram<D>,
        quad_vertex_positions_buffer: &D::Buffer,
        quad_vertex_indices_buffer: &D::Buffer,
    ) -> FilterBasicVertexArray<D> {
        let vertex_array = device.create_vertex_array();
        let position_attr = device.get_vertex_attr(&fill_basic_program.program, "Position")
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

        FilterBasicVertexArray { vertex_array }
    }
}

pub struct FilterTextProgram<D> where D: Device {
    pub program: D::Program,
    pub source_uniform: D::Uniform,
    pub source_size_uniform: D::Uniform,
    pub framebuffer_size_uniform: D::Uniform,
    pub kernel_uniform: D::Uniform,
    pub gamma_lut_uniform: D::Uniform,
    pub gamma_correction_enabled_uniform: D::Uniform,
    pub fg_color_uniform: D::Uniform,
    pub bg_color_uniform: D::Uniform,
}

impl<D> FilterTextProgram<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> FilterTextProgram<D> {
        let program = device.create_program_from_shader_names(resources,
                                                              "filter_text",
                                                              "filter",
                                                              "filter_text");
        let source_uniform = device.get_uniform(&program, "Source");
        let source_size_uniform = device.get_uniform(&program, "SourceSize");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let kernel_uniform = device.get_uniform(&program, "Kernel");
        let gamma_lut_uniform = device.get_uniform(&program, "GammaLUT");
        let gamma_correction_enabled_uniform = device.get_uniform(&program,
                                                                  "GammaCorrectionEnabled");
        let fg_color_uniform = device.get_uniform(&program, "FGColor");
        let bg_color_uniform = device.get_uniform(&program, "BGColor");
        FilterTextProgram {
            program,
            source_uniform,
            source_size_uniform,
            framebuffer_size_uniform,
            kernel_uniform,
            gamma_lut_uniform,
            gamma_correction_enabled_uniform,
            fg_color_uniform,
            bg_color_uniform,
        }
    }
}

pub struct FilterTextVertexArray<D> where D: Device {
    pub vertex_array: D::VertexArray,
}

impl<D> FilterTextVertexArray<D> where D: Device {
    pub fn new(
        device: &D,
        fill_text_program: &FilterTextProgram<D>,
        quad_vertex_positions_buffer: &D::Buffer,
        quad_vertex_indices_buffer: &D::Buffer,
    ) -> FilterTextVertexArray<D> {
        let vertex_array = device.create_vertex_array();
        let position_attr = device.get_vertex_attr(&fill_text_program.program, "Position")
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

        FilterTextVertexArray { vertex_array }
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
        let program = device.create_program(resources, "stencil");
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
        let (vertex_buffer, index_buffer) = (device.create_buffer(), device.create_buffer());

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

pub struct ReprojectionProgram<D>
where
    D: Device,
{
    pub program: D::Program,
    pub old_transform_uniform: D::Uniform,
    pub new_transform_uniform: D::Uniform,
    pub texture_uniform: D::Uniform,
}

impl<D> ReprojectionProgram<D>
where
    D: Device,
{
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> ReprojectionProgram<D> {
        let program = device.create_program(resources, "reproject");
        let old_transform_uniform = device.get_uniform(&program, "OldTransform");
        let new_transform_uniform = device.get_uniform(&program, "NewTransform");
        let texture_uniform = device.get_uniform(&program, "Texture");

        ReprojectionProgram {
            program,
            old_transform_uniform,
            new_transform_uniform,
            texture_uniform,
        }
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
