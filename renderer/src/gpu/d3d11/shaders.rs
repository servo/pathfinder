// pathfinder/renderer/src/gpu/d3d11/shaders.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::gpu::shaders::TileProgramCommon;
use crate::tiles::{TILE_HEIGHT, TILE_WIDTH};
use pathfinder_gpu::{ComputeDimensions, Device};
use pathfinder_resources::ResourceLoader;

pub const BOUND_WORKGROUP_SIZE: u32 = 64;
pub const DICE_WORKGROUP_SIZE: u32 = 64;
pub const BIN_WORKGROUP_SIZE: u32 = 64;
pub const PROPAGATE_WORKGROUP_SIZE: u32 = 64;
pub const SORT_WORKGROUP_SIZE: u32 = 64;

pub struct ProgramsD3D11<D> where D: Device {
    pub bound_program: BoundProgramD3D11<D>,
    pub dice_program: DiceProgramD3D11<D>,
    pub bin_program: BinProgramD3D11<D>,
    pub propagate_program: PropagateProgramD3D11<D>,
    pub sort_program: SortProgramD3D11<D>,
    pub fill_program: FillProgramD3D11<D>,
    pub tile_program: TileProgramD3D11<D>,
}

impl<D> ProgramsD3D11<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> ProgramsD3D11<D> {
        ProgramsD3D11 {
            bound_program: BoundProgramD3D11::new(device, resources),
            dice_program: DiceProgramD3D11::new(device, resources),
            bin_program: BinProgramD3D11::new(device, resources),
            propagate_program: PropagateProgramD3D11::new(device, resources),
            sort_program: SortProgramD3D11::new(device, resources),
            fill_program: FillProgramD3D11::new(device, resources),
            tile_program: TileProgramD3D11::new(device, resources),
        }
    }
}

pub struct PropagateProgramD3D11<D> where D: Device {
    pub program: D::Program,
    pub framebuffer_tile_size_uniform: D::Uniform,
    pub column_count_uniform: D::Uniform,
    pub first_alpha_tile_index_uniform: D::Uniform,
    pub draw_metadata_storage_buffer: D::StorageBuffer,
    pub clip_metadata_storage_buffer: D::StorageBuffer,
    pub backdrops_storage_buffer: D::StorageBuffer,
    pub draw_tiles_storage_buffer: D::StorageBuffer,
    pub clip_tiles_storage_buffer: D::StorageBuffer,
    pub z_buffer_storage_buffer: D::StorageBuffer,
    pub first_tile_map_storage_buffer: D::StorageBuffer,
    pub indirect_draw_params_storage_buffer: D::StorageBuffer,
    pub alpha_tiles_storage_buffer: D::StorageBuffer,
}

impl<D> PropagateProgramD3D11<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> PropagateProgramD3D11<D> {
        let mut program = device.create_compute_program(resources, "d3d11/propagate");
        let local_size = ComputeDimensions { x: PROPAGATE_WORKGROUP_SIZE, y: 1, z: 1 };
        device.set_compute_program_local_size(&mut program, local_size);

        let framebuffer_tile_size_uniform = device.get_uniform(&program, "FramebufferTileSize");
        let column_count_uniform = device.get_uniform(&program, "ColumnCount");
        let first_alpha_tile_index_uniform = device.get_uniform(&program, "FirstAlphaTileIndex");
        let draw_metadata_storage_buffer = device.get_storage_buffer(&program, "DrawMetadata", 0);
        let clip_metadata_storage_buffer = device.get_storage_buffer(&program, "ClipMetadata", 1);
        let backdrops_storage_buffer = device.get_storage_buffer(&program, "Backdrops", 2);
        let draw_tiles_storage_buffer = device.get_storage_buffer(&program, "DrawTiles", 3);
        let clip_tiles_storage_buffer = device.get_storage_buffer(&program, "ClipTiles", 4);
        let z_buffer_storage_buffer = device.get_storage_buffer(&program, "ZBuffer", 5);
        let first_tile_map_storage_buffer = device.get_storage_buffer(&program, "FirstTileMap", 6);
        let indirect_draw_params_storage_buffer =
            device.get_storage_buffer(&program, "IndirectDrawParams", 7);
        let alpha_tiles_storage_buffer = device.get_storage_buffer(&program, "AlphaTiles", 8);

        PropagateProgramD3D11 {
            program,
            framebuffer_tile_size_uniform,
            column_count_uniform,
            first_alpha_tile_index_uniform,
            draw_metadata_storage_buffer,
            clip_metadata_storage_buffer,
            backdrops_storage_buffer,
            draw_tiles_storage_buffer,
            clip_tiles_storage_buffer,
            z_buffer_storage_buffer,
            first_tile_map_storage_buffer,
            indirect_draw_params_storage_buffer,
            alpha_tiles_storage_buffer,
        }
    }
}

pub struct FillProgramD3D11<D> where D: Device {
    pub program: D::Program,
    pub dest_image: D::ImageParameter,
    pub area_lut_texture: D::TextureParameter,
    pub alpha_tile_range_uniform: D::Uniform,
    pub fills_storage_buffer: D::StorageBuffer,
    pub tiles_storage_buffer: D::StorageBuffer,
    pub alpha_tiles_storage_buffer: D::StorageBuffer,
}

impl<D> FillProgramD3D11<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> FillProgramD3D11<D> {
        let mut program = device.create_compute_program(resources, "d3d11/fill");
        let local_size = ComputeDimensions { x: TILE_WIDTH, y: TILE_HEIGHT / 4, z: 1 };
        device.set_compute_program_local_size(&mut program, local_size);

        let dest_image = device.get_image_parameter(&program, "Dest");
        let area_lut_texture = device.get_texture_parameter(&program, "AreaLUT");
        let alpha_tile_range_uniform = device.get_uniform(&program, "AlphaTileRange");
        let fills_storage_buffer = device.get_storage_buffer(&program, "Fills", 0);
        let tiles_storage_buffer = device.get_storage_buffer(&program, "Tiles", 1);
        let alpha_tiles_storage_buffer = device.get_storage_buffer(&program, "AlphaTiles", 2);

        FillProgramD3D11 {
            program,
            dest_image,
            area_lut_texture,
            alpha_tile_range_uniform,
            fills_storage_buffer,
            tiles_storage_buffer,
            alpha_tiles_storage_buffer,
        }
    }
}

pub struct TileProgramD3D11<D> where D: Device {
    pub common: TileProgramCommon<D>,
    pub load_action_uniform: D::Uniform,
    pub clear_color_uniform: D::Uniform,
    pub framebuffer_tile_size_uniform: D::Uniform,
    pub dest_image: D::ImageParameter,
    pub tiles_storage_buffer: D::StorageBuffer,
    pub first_tile_map_storage_buffer: D::StorageBuffer,
}

impl<D> TileProgramD3D11<D> where D: Device {
    fn new(device: &D, resources: &dyn ResourceLoader) -> TileProgramD3D11<D> {
        let mut program = device.create_compute_program(resources, "d3d11/tile");
        device.set_compute_program_local_size(&mut program,
                                              ComputeDimensions { x: 16, y: 4, z: 1 });

        let load_action_uniform = device.get_uniform(&program, "LoadAction");
        let clear_color_uniform = device.get_uniform(&program, "ClearColor");
        let framebuffer_tile_size_uniform = device.get_uniform(&program, "FramebufferTileSize");
        let dest_image = device.get_image_parameter(&program, "DestImage");
        let tiles_storage_buffer = device.get_storage_buffer(&program, "Tiles", 0);
        let first_tile_map_storage_buffer = device.get_storage_buffer(&program, "FirstTileMap", 1);

        let common = TileProgramCommon::new(device, program);
        TileProgramD3D11 {
            common,
            load_action_uniform,
            clear_color_uniform,
            framebuffer_tile_size_uniform,
            dest_image,
            tiles_storage_buffer,
            first_tile_map_storage_buffer,
        }
    }
}

pub struct BinProgramD3D11<D> where D: Device {
    pub program: D::Program,
    pub microline_count_uniform: D::Uniform,
    pub max_fill_count_uniform: D::Uniform,
    pub microlines_storage_buffer: D::StorageBuffer,
    pub metadata_storage_buffer: D::StorageBuffer,
    pub indirect_draw_params_storage_buffer: D::StorageBuffer,
    pub fills_storage_buffer: D::StorageBuffer,
    pub tiles_storage_buffer: D::StorageBuffer,
    pub backdrops_storage_buffer: D::StorageBuffer,
}

impl<D> BinProgramD3D11<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> BinProgramD3D11<D> {
        let mut program = device.create_compute_program(resources, "d3d11/bin");
        let dimensions = ComputeDimensions { x: BIN_WORKGROUP_SIZE, y: 1, z: 1 };
        device.set_compute_program_local_size(&mut program, dimensions);

        let microline_count_uniform = device.get_uniform(&program, "MicrolineCount");
        let max_fill_count_uniform = device.get_uniform(&program, "MaxFillCount");

        let microlines_storage_buffer = device.get_storage_buffer(&program, "Microlines", 0);
        let metadata_storage_buffer = device.get_storage_buffer(&program, "Metadata", 1);
        let indirect_draw_params_storage_buffer =
            device.get_storage_buffer(&program, "IndirectDrawParams", 2);
        let fills_storage_buffer = device.get_storage_buffer(&program, "Fills", 3);
        let tiles_storage_buffer = device.get_storage_buffer(&program, "Tiles", 4);
        let backdrops_storage_buffer = device.get_storage_buffer(&program, "Backdrops", 5);

        BinProgramD3D11 {
            program,
            microline_count_uniform,
            max_fill_count_uniform,
            metadata_storage_buffer,
            indirect_draw_params_storage_buffer,
            fills_storage_buffer,
            tiles_storage_buffer,
            microlines_storage_buffer,
            backdrops_storage_buffer,
        }
    }
}

pub struct DiceProgramD3D11<D> where D: Device {
    pub program: D::Program,
    pub transform_uniform: D::Uniform,
    pub translation_uniform: D::Uniform,
    pub path_count_uniform: D::Uniform,
    pub last_batch_segment_index_uniform: D::Uniform,
    pub max_microline_count_uniform: D::Uniform,
    pub compute_indirect_params_storage_buffer: D::StorageBuffer,
    pub dice_metadata_storage_buffer: D::StorageBuffer,
    pub points_storage_buffer: D::StorageBuffer,
    pub input_indices_storage_buffer: D::StorageBuffer,
    pub microlines_storage_buffer: D::StorageBuffer,
}

impl<D> DiceProgramD3D11<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> DiceProgramD3D11<D> {
        let mut program = device.create_compute_program(resources, "d3d11/dice");
        let dimensions = ComputeDimensions { x: DICE_WORKGROUP_SIZE, y: 1, z: 1 };
        device.set_compute_program_local_size(&mut program, dimensions);

        let transform_uniform = device.get_uniform(&program, "Transform");
        let translation_uniform = device.get_uniform(&program, "Translation");
        let path_count_uniform = device.get_uniform(&program, "PathCount");
        let last_batch_segment_index_uniform = device.get_uniform(&program,
                                                                  "LastBatchSegmentIndex");
        let max_microline_count_uniform = device.get_uniform(&program, "MaxMicrolineCount");

        let compute_indirect_params_storage_buffer =
            device.get_storage_buffer(&program, "ComputeIndirectParams", 0);
        let dice_metadata_storage_buffer = device.get_storage_buffer(&program, "DiceMetadata", 1);
        let points_storage_buffer = device.get_storage_buffer(&program, "Points", 2);
        let input_indices_storage_buffer = device.get_storage_buffer(&program, "InputIndices", 3);
        let microlines_storage_buffer = device.get_storage_buffer(&program, "Microlines", 4);

        DiceProgramD3D11 {
            program,
            transform_uniform,
            translation_uniform,
            path_count_uniform,
            last_batch_segment_index_uniform,
            max_microline_count_uniform,
            compute_indirect_params_storage_buffer,
            dice_metadata_storage_buffer,
            points_storage_buffer,
            input_indices_storage_buffer,
            microlines_storage_buffer,
        }
    }
}

pub struct BoundProgramD3D11<D> where D: Device {
    pub program: D::Program,
    pub path_count_uniform: D::Uniform,
    pub tile_count_uniform: D::Uniform,
    pub tile_path_info_storage_buffer: D::StorageBuffer,
    pub tiles_storage_buffer: D::StorageBuffer,
}

impl<D> BoundProgramD3D11<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> BoundProgramD3D11<D> {
        let mut program = device.create_compute_program(resources, "d3d11/bound");
        let dimensions = ComputeDimensions { x: BOUND_WORKGROUP_SIZE, y: 1, z: 1 };
        device.set_compute_program_local_size(&mut program, dimensions);

        let path_count_uniform = device.get_uniform(&program, "PathCount");
        let tile_count_uniform = device.get_uniform(&program, "TileCount");

        let tile_path_info_storage_buffer = device.get_storage_buffer(&program, "TilePathInfo", 0);
        let tiles_storage_buffer = device.get_storage_buffer(&program, "Tiles", 1);

        BoundProgramD3D11 {
            program,
            path_count_uniform,
            tile_count_uniform,
            tile_path_info_storage_buffer,
            tiles_storage_buffer,
        }
    }
}

pub struct SortProgramD3D11<D> where D: Device {
    pub program: D::Program,
    pub tile_count_uniform: D::Uniform,
    pub tiles_storage_buffer: D::StorageBuffer,
    pub first_tile_map_storage_buffer: D::StorageBuffer,
    pub z_buffer_storage_buffer: D::StorageBuffer,
}

impl<D> SortProgramD3D11<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader) -> SortProgramD3D11<D> {
        let mut program = device.create_compute_program(resources, "d3d11/sort");
        let dimensions = ComputeDimensions { x: SORT_WORKGROUP_SIZE, y: 1, z: 1 };
        device.set_compute_program_local_size(&mut program, dimensions);

        let tile_count_uniform = device.get_uniform(&program, "TileCount");
        let tiles_storage_buffer = device.get_storage_buffer(&program, "Tiles", 0);
        let first_tile_map_storage_buffer = device.get_storage_buffer(&program, "FirstTileMap", 1);
        let z_buffer_storage_buffer = device.get_storage_buffer(&program, "ZBuffer", 2);

        SortProgramD3D11 {
            program,
            tile_count_uniform,
            tiles_storage_buffer,
            first_tile_map_storage_buffer,
            z_buffer_storage_buffer,
        }
    }
}