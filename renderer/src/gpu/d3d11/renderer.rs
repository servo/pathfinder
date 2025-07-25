// pathfinder/renderer/src/gpu/d3d11/renderer.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A GPU compute-based renderer that uses functionality available in Direct3D 11.
//! 
//! This renderer supports OpenGL at least 4.3, OpenGL ES at least 3.1, and Metal of any version.

use std::mem;
use crate::gpu::d3d11::shaders::{BOUND_WORKGROUP_SIZE, DICE_WORKGROUP_SIZE, BIN_WORKGROUP_SIZE};
use crate::gpu::d3d11::shaders::{PROPAGATE_WORKGROUP_SIZE, ProgramsD3D11, SORT_WORKGROUP_SIZE};
use crate::gpu::perf::TimeCategory;
use crate::gpu::renderer::{FramebufferFlags, RendererCore};
use crate::gpu_data::{AlphaTileD3D11, BackdropInfoD3D11, DiceMetadataD3D11, DrawTileBatchD3D11};
use crate::gpu_data::{Fill, FirstTileD3D11, MicrolineD3D11, PathSource, PropagateMetadataD3D11};
use crate::gpu_data::{SegmentIndicesD3D11, SegmentsD3D11, TileD3D11, TileBatchDataD3D11};
use crate::gpu_data::{TileBatchTexture, TilePathInfoD3D11};
use byte_slice_cast::AsSliceOf;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::Vector2F;
use pathfinder_gpu::allocator::{BufferTag, GeneralBufferID, GPUMemoryAllocator};
use pathfinder_gpu::{BufferTarget, ComputeDimensions, ComputeState, Device, ImageAccess};
use pathfinder_gpu::{RenderTarget, UniformData};
use pathfinder_resources::ResourceLoader;
use pathfinder_simd::default::{F32x4, I32x2};
use std::ops::Range;
use vec_map::VecMap;

const FILL_INDIRECT_DRAW_PARAMS_INSTANCE_COUNT_INDEX:   usize = 1;
const FILL_INDIRECT_DRAW_PARAMS_ALPHA_TILE_COUNT_INDEX: usize = 4;
const FILL_INDIRECT_DRAW_PARAMS_SIZE:                   usize = 8;

const BIN_INDIRECT_DRAW_PARAMS_MICROLINE_COUNT_INDEX:   usize = 3;

const LOAD_ACTION_CLEAR: i32 = 0;
const LOAD_ACTION_LOAD:  i32 = 1;

const INITIAL_ALLOCATED_MICROLINE_COUNT: u32 = 1024 * 16;
const INITIAL_ALLOCATED_FILL_COUNT: u32 = 1024 * 16;

pub(crate) struct RendererD3D11<D> where D: Device {
    programs: ProgramsD3D11<D>,
    allocated_microline_count: u32,
    allocated_fill_count: u32,
    scene_buffers: SceneBuffers,
    tile_batch_info: VecMap<TileBatchInfoD3D11>,
}

impl<D> RendererD3D11<D> where D: Device {
    pub(crate) fn new(core: &mut RendererCore<D>, resources: &dyn ResourceLoader)
                      -> RendererD3D11<D> {
        let programs = ProgramsD3D11::new(&core.device, resources);
        RendererD3D11 {
            programs,
            allocated_fill_count: INITIAL_ALLOCATED_FILL_COUNT,
            allocated_microline_count: INITIAL_ALLOCATED_MICROLINE_COUNT,
            scene_buffers: SceneBuffers::new(),
            tile_batch_info: VecMap::<TileBatchInfoD3D11>::new(),
        }
    }

    fn bound(&mut self,
             core: &mut RendererCore<D>,
             tiles_d3d11_buffer_id: GeneralBufferID,
             tile_count: u32,
             tile_path_info: &[TilePathInfoD3D11]) {
        let bound_program = &self.programs.bound_program;

        let path_info_buffer_id =
            core.allocator
                .allocate_general_buffer::<TilePathInfoD3D11>(&core.device,
                                                              tile_path_info.len() as u64,
                                                              BufferTag("TilePathInfoD3D11"));
        let tile_path_info_buffer = core.allocator.get_general_buffer(path_info_buffer_id);
        core.device.upload_to_buffer(tile_path_info_buffer,
                                     0,
                                     tile_path_info,
                                     BufferTarget::Storage);

        let tiles_buffer = core.allocator.get_general_buffer(tiles_d3d11_buffer_id);

        let timer_query = core.timer_query_cache.start_timing_draw_call(&core.device,
                                                                        &core.options);

        let compute_dimensions = ComputeDimensions {
            x: (tile_count + BOUND_WORKGROUP_SIZE - 1) / BOUND_WORKGROUP_SIZE,
            y: 1,
            z: 1,
        };
        core.device.dispatch_compute(compute_dimensions, &ComputeState {
            program: &bound_program.program,
            textures: &[],
            uniforms: &[
                (&bound_program.path_count_uniform, UniformData::Int(tile_path_info.len() as i32)),
                (&bound_program.tile_count_uniform, UniformData::Int(tile_count as i32)),
            ],
            images: &[],
            storage_buffers: &[
                (&bound_program.tile_path_info_storage_buffer, tile_path_info_buffer),
                (&bound_program.tiles_storage_buffer, tiles_buffer),
            ],
        });

        core.stats.drawcall_count += 1;
        core.finish_timing_draw_call(&timer_query);
        core.current_timer.as_mut().unwrap().push_query(TimeCategory::Other, timer_query);

        core.allocator.free_general_buffer(path_info_buffer_id);
    }

    fn upload_propagate_metadata(&mut self,
                                 core: &mut RendererCore<D>,
                                 propagate_metadata: &[PropagateMetadataD3D11],
                                 backdrops: &[BackdropInfoD3D11])
                                 -> PropagateMetadataBufferIDsD3D11 {
        let propagate_metadata_storage_id =
            core.allocator.allocate_general_buffer::<PropagateMetadataD3D11>(
                &core.device,
                propagate_metadata.len() as u64,
                BufferTag("PropagateMetadataD3D11"));
        let propagate_metadata_buffer =
            core.allocator.get_general_buffer(propagate_metadata_storage_id);
        core.device.upload_to_buffer(propagate_metadata_buffer,
                                     0,
                                     propagate_metadata,
                                     BufferTarget::Storage);

        let backdrops_storage_id = core.allocator.allocate_general_buffer::<BackdropInfoD3D11>(
            &core.device,
            backdrops.len() as u64,
            BufferTag("BackdropInfoD3D11"));

        PropagateMetadataBufferIDsD3D11 {
             propagate_metadata: propagate_metadata_storage_id,
             backdrops: backdrops_storage_id,
        }
    }

    fn upload_initial_backdrops(&self,
                                core: &RendererCore<D>,
                                backdrops_buffer_id: GeneralBufferID,
                                backdrops: &[BackdropInfoD3D11]) {
        let backdrops_buffer = core.allocator.get_general_buffer(backdrops_buffer_id);
        core.device.upload_to_buffer(backdrops_buffer, 0, backdrops, BufferTarget::Storage);
    }

    fn bin_segments(&mut self,
                    core: &mut RendererCore<D>,
                    microlines_storage: &MicrolinesBufferIDsD3D11,
                    propagate_metadata_buffer_ids: &PropagateMetadataBufferIDsD3D11,
                    tiles_d3d11_buffer_id: GeneralBufferID,
                    z_buffer_id: GeneralBufferID)
                    -> Option<FillBufferInfoD3D11> {
        let bin_program = &self.programs.bin_program;

        let fill_vertex_buffer_id =
            core.allocator.allocate_general_buffer::<Fill>(&core.device,
                                                           self.allocated_fill_count as u64,
                                                           BufferTag("Fill"));

        let fill_vertex_buffer = core.allocator.get_general_buffer(fill_vertex_buffer_id);
        let microlines_buffer = core.allocator.get_general_buffer(microlines_storage.buffer_id);
        let tiles_buffer = core.allocator.get_general_buffer(tiles_d3d11_buffer_id);
        let propagate_metadata_buffer =
            core.allocator.get_general_buffer(propagate_metadata_buffer_ids.propagate_metadata);
        let backdrops_buffer =
            core.allocator.get_general_buffer(propagate_metadata_buffer_ids.backdrops);

        // Upload fill indirect draw params to header of the Z-buffer.
        //
        // This is in the Z-buffer, not its own buffer, to work around the 8 SSBO limitation on
        // some drivers (#373).
        let z_buffer = core.allocator.get_general_buffer(z_buffer_id);
        let indirect_draw_params = [6, 0, 0, 0, 0, microlines_storage.count, 0, 0];
        core.device.upload_to_buffer::<u32>(&z_buffer,
                                            0,
                                            &indirect_draw_params,
                                            BufferTarget::Storage);

        let timer_query = core.timer_query_cache.start_timing_draw_call(&core.device,
                                                                        &core.options);

        let compute_dimensions = ComputeDimensions {
            x: (microlines_storage.count + BIN_WORKGROUP_SIZE - 1) / BIN_WORKGROUP_SIZE,
            y: 1,
            z: 1,
        };

        core.device.dispatch_compute(compute_dimensions, &ComputeState {
            program: &bin_program.program,
            textures: &[],
            uniforms: &[
                (&bin_program.microline_count_uniform,
                 UniformData::Int(microlines_storage.count as i32)),
                (&bin_program.max_fill_count_uniform,
                 UniformData::Int(self.allocated_fill_count as i32)),
            ],
            images: &[],
            storage_buffers: &[
                (&bin_program.microlines_storage_buffer, microlines_buffer),
                (&bin_program.metadata_storage_buffer, propagate_metadata_buffer),
                (&bin_program.indirect_draw_params_storage_buffer, z_buffer),
                (&bin_program.fills_storage_buffer, fill_vertex_buffer),
                (&bin_program.tiles_storage_buffer, tiles_buffer),
                (&bin_program.backdrops_storage_buffer, backdrops_buffer),
            ],
        });

        core.stats.drawcall_count += 1;
        core.finish_timing_draw_call(&timer_query);
        core.current_timer.as_mut().unwrap().push_query(TimeCategory::Bin, timer_query);

        let indirect_draw_params_receiver = core.device.read_buffer(z_buffer,
                                                                    BufferTarget::Storage,
                                                                    0..32);
        let indirect_draw_params = core.device.recv_buffer(&indirect_draw_params_receiver);
        let indirect_draw_params: &[u32] = indirect_draw_params.as_slice_of().unwrap();

        let needed_fill_count =
            indirect_draw_params[FILL_INDIRECT_DRAW_PARAMS_INSTANCE_COUNT_INDEX];
        if needed_fill_count > self.allocated_fill_count {
            self.allocated_fill_count = needed_fill_count.next_power_of_two();
            return None;
        }

        core.stats.fill_count += needed_fill_count as usize;

        Some(FillBufferInfoD3D11 { fill_vertex_buffer_id })
    }

    pub(crate) fn upload_scene(&mut self,
                               core: &mut RendererCore<D>,
                               draw_segments: &SegmentsD3D11,
                               clip_segments: &SegmentsD3D11) {
        self.scene_buffers.upload(&mut core.allocator, &core.device, draw_segments, clip_segments);
    }

    fn allocate_tiles(&mut self, core: &mut RendererCore<D>, tile_count: u32) -> GeneralBufferID {
        core.allocator.allocate_general_buffer::<TileD3D11>(&core.device,
                                                            tile_count as u64,
                                                            BufferTag("TileD3D11"))
    }

    fn dice_segments(&mut self,
                     core: &mut RendererCore<D>,
                     dice_metadata: &[DiceMetadataD3D11],
                     batch_segment_count: u32,
                     path_source: PathSource,
                     transform: Transform2F)
                     -> Option<MicrolinesBufferIDsD3D11> {
        let dice_program = &self.programs.dice_program;

        let microlines_buffer_id = core.allocator.allocate_general_buffer::<MicrolineD3D11>(
            &core.device,
            self.allocated_microline_count as u64,
            BufferTag("MicrolineD3D11"));
        let dice_metadata_buffer_id = core.allocator.allocate_general_buffer::<DiceMetadataD3D11>(
            &core.device,
            dice_metadata.len() as u64,
            BufferTag("DiceMetadataD3D11"));
        let dice_indirect_draw_params_buffer_id = core.allocator.allocate_general_buffer::<u32>(
            &core.device,
            8,
            BufferTag("DiceIndirectDrawParamsD3D11"));

        let microlines_buffer = core.allocator.get_general_buffer(microlines_buffer_id);
        let dice_metadata_storage_buffer =
            core.allocator.get_general_buffer(dice_metadata_buffer_id);
        let dice_indirect_draw_params_buffer =
            core.allocator.get_general_buffer(dice_indirect_draw_params_buffer_id);

        let scene_buffers = &self.scene_buffers;
        let scene_source_buffers = match path_source {
            PathSource::Draw => &scene_buffers.draw,
            PathSource::Clip => &scene_buffers.clip,
        };
        let SceneSourceBuffers {
            points_buffer: points_buffer_id,
            point_indices_buffer: point_indices_buffer_id,
            point_indices_count,
            ..
        } = *scene_source_buffers;

        let points_buffer = core.allocator.get_general_buffer(
            points_buffer_id.expect("Where's the points buffer?"));
        let point_indices_buffer = core.allocator.get_general_buffer(
            point_indices_buffer_id.expect("Where's the point indices buffer?"));

        core.device.upload_to_buffer(dice_indirect_draw_params_buffer,
                                     0,
                                     &[0, 0, 0, 0, point_indices_count, 0, 0, 0],
                                     BufferTarget::Storage);
        core.device.upload_to_buffer(dice_metadata_storage_buffer,
                                     0,
                                     dice_metadata,
                                     BufferTarget::Storage);

        let timer_query = core.timer_query_cache.start_timing_draw_call(&core.device,
                                                                        &core.options);

        let workgroup_count = (batch_segment_count + DICE_WORKGROUP_SIZE - 1) /
            DICE_WORKGROUP_SIZE;
        let compute_dimensions = ComputeDimensions { x: workgroup_count, y: 1, z: 1 };

        core.device.dispatch_compute(compute_dimensions, &ComputeState {
            program: &dice_program.program,
            textures: &[],
            uniforms: &[
                (&dice_program.transform_uniform, UniformData::Mat2(transform.matrix.0)),
                (&dice_program.translation_uniform, UniformData::Vec2(transform.vector.0)),
                (&dice_program.path_count_uniform,
                 UniformData::Int(dice_metadata.len() as i32)),
                (&dice_program.last_batch_segment_index_uniform,
                 UniformData::Int(batch_segment_count as i32)),
                (&dice_program.max_microline_count_uniform,
                 UniformData::Int(self.allocated_microline_count as i32)),
            ],
            images: &[],
            storage_buffers: &[
                (&dice_program.compute_indirect_params_storage_buffer,
                 dice_indirect_draw_params_buffer),
                (&dice_program.points_storage_buffer, points_buffer),
                (&dice_program.input_indices_storage_buffer, point_indices_buffer),
                (&dice_program.microlines_storage_buffer, microlines_buffer),
                (&dice_program.dice_metadata_storage_buffer, &dice_metadata_storage_buffer),
            ],
        });

        core.stats.drawcall_count += 1;
        core.finish_timing_draw_call(&timer_query);
        core.current_timer.as_mut().unwrap().push_query(TimeCategory::Dice, timer_query);

        let indirect_compute_params_receiver =
            core.device.read_buffer(&dice_indirect_draw_params_buffer,
                                    BufferTarget::Storage,
                                    0..32);
        let indirect_compute_params = core.device.recv_buffer(&indirect_compute_params_receiver);
        let indirect_compute_params: &[u32] = indirect_compute_params.as_slice_of().unwrap();

        core.allocator.free_general_buffer(dice_metadata_buffer_id);
        core.allocator.free_general_buffer(dice_indirect_draw_params_buffer_id);

        let microline_count =
            indirect_compute_params[BIN_INDIRECT_DRAW_PARAMS_MICROLINE_COUNT_INDEX];
        if microline_count > self.allocated_microline_count {
            self.allocated_microline_count = microline_count.next_power_of_two();
            return None;
        }

        Some(MicrolinesBufferIDsD3D11 { buffer_id: microlines_buffer_id, count: microline_count })
    }

    fn draw_fills(&mut self,
                  core: &mut RendererCore<D>,
                  fill_storage_info: &FillBufferInfoD3D11,
                  tiles_d3d11_buffer_id: GeneralBufferID,
                  alpha_tiles_buffer_id: GeneralBufferID,
                  propagate_tiles_info: &PropagateTilesInfoD3D11) {
        let &FillBufferInfoD3D11 { fill_vertex_buffer_id } = fill_storage_info;
        let &PropagateTilesInfoD3D11 { ref alpha_tile_range } = propagate_tiles_info;

        let fill_program = &self.programs.fill_program;
        let fill_vertex_buffer = core.allocator.get_general_buffer(fill_vertex_buffer_id);

        let mask_storage = core.mask_storage.as_ref().expect("Where's the mask storage?");
        let mask_framebuffer_id = mask_storage.framebuffer_id;
        let mask_framebuffer = core.allocator.get_framebuffer(mask_framebuffer_id);
        let image_texture = core.device.framebuffer_texture(mask_framebuffer);

        let tiles_d3d11_buffer = core.allocator.get_general_buffer(tiles_d3d11_buffer_id);
        let alpha_tiles_buffer = core.allocator.get_general_buffer(alpha_tiles_buffer_id);

        let area_lut_texture = core.allocator.get_texture(core.area_lut_texture_id);

        let timer_query = core.timer_query_cache.start_timing_draw_call(&core.device,
                                                                        &core.options);

        // This setup is an annoying workaround for the 64K limit of compute invocation in OpenGL.
        let alpha_tile_count = alpha_tile_range.end - alpha_tile_range.start;
        let dimensions = ComputeDimensions {
            x: alpha_tile_count.min(1 << 15) as u32,
            y: ((alpha_tile_count + (1 << 15) - 1) >> 15) as u32,
            z: 1,
        };

        core.device.dispatch_compute(dimensions, &ComputeState {
            program: &fill_program.program,
            textures: &[(&fill_program.area_lut_texture, area_lut_texture)],
            images: &[(&fill_program.dest_image, image_texture, ImageAccess::ReadWrite)],
            uniforms: &[
                (&fill_program.alpha_tile_range_uniform,
                 UniformData::IVec2(I32x2::new(alpha_tile_range.start as i32,
                                               alpha_tile_range.end as i32))),
            ],
            storage_buffers: &[
                (&fill_program.fills_storage_buffer, fill_vertex_buffer),
                (&fill_program.tiles_storage_buffer, tiles_d3d11_buffer),
                (&fill_program.alpha_tiles_storage_buffer, &alpha_tiles_buffer),
            ],
        });

        core.stats.drawcall_count += 1;
        core.finish_timing_draw_call(&timer_query);
        core.current_timer.as_mut().unwrap().push_query(TimeCategory::Fill, timer_query);

        core.framebuffer_flags.insert(FramebufferFlags::MASK_FRAMEBUFFER_IS_DIRTY);
    }

    pub(crate) fn prepare_and_draw_tiles(&mut self,
                                         core: &mut RendererCore<D>,
                                         batch: &DrawTileBatchD3D11) {
        let tile_batch_id = batch.tile_batch_data.batch_id;
        self.prepare_tiles(core, &batch.tile_batch_data);
        let batch_info = self.tile_batch_info[tile_batch_id.0 as usize].clone();
        self.draw_tiles(core,
                        batch_info.tiles_d3d11_buffer_id,
                        batch_info.first_tile_map_buffer_id,
                        batch.color_texture);
    }

    // Computes backdrops, performs clipping, and populates Z buffers on GPU.
    pub(crate) fn prepare_tiles(&mut self,
                                core: &mut RendererCore<D>,
                                batch: &TileBatchDataD3D11) {
        core.stats.total_tile_count += batch.tile_count as usize;

        // Upload tiles to GPU or allocate them as appropriate.
        let tiles_d3d11_buffer_id = self.allocate_tiles(core, batch.tile_count);

        // Fetch and/or allocate clip storage as needed.
        let clip_buffer_ids = match batch.clipped_path_info {
            Some(ref clipped_path_info) => {
                let clip_batch_id = clipped_path_info.clip_batch_id;
                let clip_tile_batch_info = &self.tile_batch_info[clip_batch_id.0 as usize];
                let metadata = clip_tile_batch_info.propagate_metadata_buffer_id;
                let tiles = clip_tile_batch_info.tiles_d3d11_buffer_id;
                Some(ClipBufferIDs { metadata: Some(metadata), tiles })
            }
            None => None,
        };

        // Allocate a Z-buffer.
        let z_buffer_id = self.allocate_z_buffer(core);

        // Propagate backdrops, bin fills, render fills, and/or perform clipping on GPU if
        // necessary.
        // Allocate space for tile lists.
        let first_tile_map_buffer_id = self.allocate_first_tile_map(core);

        let propagate_metadata_buffer_ids =
            self.upload_propagate_metadata(core,
                                           &batch.prepare_info.propagate_metadata,
                                           &batch.prepare_info.backdrops);

        // Dice (flatten) segments into microlines. We might have to do this twice if our
        // first attempt runs out of space in the storage buffer.
        let mut microlines_storage = None;
        for _ in 0..2 {
            microlines_storage = self.dice_segments(core,
                                                    &batch.prepare_info.dice_metadata,
                                                    batch.segment_count,
                                                    batch.path_source,
                                                    batch.prepare_info.transform);
            if microlines_storage.is_some() {
                break;
            }
        }
        let microlines_storage =
            microlines_storage.expect("Ran out of space for microlines when dicing!");

        // Initialize tiles, and bin segments. We might have to do this twice if our first
        // attempt runs out of space in the fill buffer.
        let mut fill_buffer_info = None;
        for _ in 0..2 {
            self.bound(core,
                       tiles_d3d11_buffer_id,
                       batch.tile_count,
                       &batch.prepare_info.tile_path_info);

            self.upload_initial_backdrops(core,
                                          propagate_metadata_buffer_ids.backdrops,
                                          &batch.prepare_info.backdrops);

            fill_buffer_info = self.bin_segments(core,
                                                 &microlines_storage,
                                                 &propagate_metadata_buffer_ids,
                                                 tiles_d3d11_buffer_id,
                                                 z_buffer_id);
            if fill_buffer_info.is_some() {
                break;
            }
        }
        let fill_buffer_info =
            fill_buffer_info.expect("Ran out of space for fills when binning!");

        core.allocator.free_general_buffer(microlines_storage.buffer_id);

        // TODO(pcwalton): If we run out of space for alpha tile indices, propagate
        // multiple times.

        let alpha_tiles_buffer_id = self.allocate_alpha_tile_info(core, batch.tile_count);

        let propagate_tiles_info =
            self.propagate_tiles(core,
                                 batch.prepare_info.backdrops.len() as u32,
                                 tiles_d3d11_buffer_id,
                                 z_buffer_id,
                                 first_tile_map_buffer_id,
                                 alpha_tiles_buffer_id,
                                 &propagate_metadata_buffer_ids,
                                 clip_buffer_ids.as_ref());

        core.allocator.free_general_buffer(propagate_metadata_buffer_ids.backdrops);

        // FIXME(pcwalton): Don't unconditionally pass true for copying here.
        core.reallocate_alpha_tile_pages_if_necessary(true);
        self.draw_fills(core,
                        &fill_buffer_info,
                        tiles_d3d11_buffer_id,
                        alpha_tiles_buffer_id,
                        &propagate_tiles_info);

        core.allocator.free_general_buffer(fill_buffer_info.fill_vertex_buffer_id);
        core.allocator.free_general_buffer(alpha_tiles_buffer_id);

        // FIXME(pcwalton): This seems like the wrong place to do this...
        self.sort_tiles(core, tiles_d3d11_buffer_id, first_tile_map_buffer_id, z_buffer_id);

        // Record tile batch info.
        self.tile_batch_info.insert(batch.batch_id.0 as usize, TileBatchInfoD3D11 {
            tile_count: batch.tile_count,
            z_buffer_id,
            tiles_d3d11_buffer_id,
            propagate_metadata_buffer_id: propagate_metadata_buffer_ids.propagate_metadata,
            first_tile_map_buffer_id,
        });
    }

    fn propagate_tiles(&mut self,
                       core: &mut RendererCore<D>,
                       column_count: u32,
                       tiles_d3d11_buffer_id: GeneralBufferID,
                       z_buffer_id: GeneralBufferID,
                       first_tile_map_buffer_id: GeneralBufferID,
                       alpha_tiles_buffer_id: GeneralBufferID,
                       propagate_metadata_buffer_ids: &PropagateMetadataBufferIDsD3D11,
                       clip_buffer_ids: Option<&ClipBufferIDs>)
                       -> PropagateTilesInfoD3D11 {
        let propagate_program = &self.programs.propagate_program;

        let tiles_d3d11_buffer = core.allocator.get_general_buffer(tiles_d3d11_buffer_id);
        let propagate_metadata_storage_buffer =
            core.allocator.get_general_buffer(propagate_metadata_buffer_ids.propagate_metadata);
        let backdrops_storage_buffer =
            core.allocator.get_general_buffer(propagate_metadata_buffer_ids.backdrops);

        // TODO(pcwalton): Zero out the Z-buffer on GPU?
        let z_buffer = core.allocator.get_general_buffer(z_buffer_id);
        let z_buffer_size = core.tile_size();
        let tile_area = z_buffer_size.area() as usize;
        core.device.upload_to_buffer(z_buffer, FILL_INDIRECT_DRAW_PARAMS_SIZE * mem::size_of::<i32>(), &vec![0i32; tile_area], BufferTarget::Storage);

        // TODO(pcwalton): Initialize the first tiles buffer on GPU?
        let first_tile_map_storage_buffer = core.allocator
                                                .get_general_buffer(first_tile_map_buffer_id);
        core.device.upload_to_buffer::<FirstTileD3D11>(&first_tile_map_storage_buffer,
                                                       0,
                                                       &vec![FirstTileD3D11::default(); tile_area],
                                                       BufferTarget::Storage);

        let alpha_tiles_storage_buffer = core.allocator.get_general_buffer(alpha_tiles_buffer_id);

        let mut storage_buffers = vec![
            (&propagate_program.draw_metadata_storage_buffer, propagate_metadata_storage_buffer),
            (&propagate_program.backdrops_storage_buffer, &backdrops_storage_buffer),
            (&propagate_program.draw_tiles_storage_buffer, tiles_d3d11_buffer),
            (&propagate_program.z_buffer_storage_buffer, z_buffer),
            (&propagate_program.first_tile_map_storage_buffer, first_tile_map_storage_buffer),
            (&propagate_program.alpha_tiles_storage_buffer, alpha_tiles_storage_buffer),
        ];

        match clip_buffer_ids {
            Some(clip_buffer_ids) => {
                let clip_metadata_buffer_id =
                    clip_buffer_ids.metadata.expect("Where's the clip metadata storage?");
                let clip_metadata_buffer = core.allocator
                                               .get_general_buffer(clip_metadata_buffer_id);
                let clip_tile_buffer = core.allocator.get_general_buffer(clip_buffer_ids.tiles);
                storage_buffers.push((&propagate_program.clip_metadata_storage_buffer,
                                    clip_metadata_buffer));
                storage_buffers.push((&propagate_program.clip_tiles_storage_buffer,
                                      clip_tile_buffer));
            }
            None => {
                // Just attach any old buffers to these, to satisfy Metal.
                storage_buffers.push((&propagate_program.clip_metadata_storage_buffer,
                                      propagate_metadata_storage_buffer));
                storage_buffers.push((&propagate_program.clip_tiles_storage_buffer,
                                      tiles_d3d11_buffer));
            }
        }

        let timer_query = core.timer_query_cache.start_timing_draw_call(&core.device,
                                                                        &core.options);

        let dimensions = ComputeDimensions {
            x: (column_count + PROPAGATE_WORKGROUP_SIZE - 1) / PROPAGATE_WORKGROUP_SIZE,
            y: 1,
            z: 1,
        };
        core.device.dispatch_compute(dimensions, &ComputeState {
            program: &propagate_program.program,
            textures: &[],
            images: &[],
            uniforms: &[
                (&propagate_program.framebuffer_tile_size_uniform,
                 UniformData::IVec2(core.framebuffer_tile_size().0)),
                (&propagate_program.column_count_uniform, UniformData::Int(column_count as i32)),
                (&propagate_program.first_alpha_tile_index_uniform,
                 UniformData::Int(core.alpha_tile_count as i32)),
            ],
            storage_buffers: &storage_buffers,
        });

        core.stats.drawcall_count += 1;
        core.finish_timing_draw_call(&timer_query);
        core.current_timer.as_mut().unwrap().push_query(TimeCategory::Other, timer_query);

        let fill_indirect_draw_params_receiver =
            core.device.read_buffer(&z_buffer, BufferTarget::Storage, 0..32);
        let fill_indirect_draw_params = core.device
                                            .recv_buffer(&fill_indirect_draw_params_receiver);
        let fill_indirect_draw_params: &[u32] = fill_indirect_draw_params.as_slice_of().unwrap();

        let batch_alpha_tile_count =
            fill_indirect_draw_params[FILL_INDIRECT_DRAW_PARAMS_ALPHA_TILE_COUNT_INDEX];

        let alpha_tile_start = core.alpha_tile_count;
        core.alpha_tile_count += batch_alpha_tile_count;
        core.stats.alpha_tile_count += batch_alpha_tile_count as usize;
        let alpha_tile_end = core.alpha_tile_count;

        PropagateTilesInfoD3D11 { alpha_tile_range: alpha_tile_start..alpha_tile_end }
    }

    fn sort_tiles(&mut self,
                  core: &mut RendererCore<D>,
                  tiles_d3d11_buffer_id: GeneralBufferID,
                  first_tile_map_buffer_id: GeneralBufferID,
                  z_buffer_id: GeneralBufferID) {
        let sort_program = &self.programs.sort_program;

        let tiles_d3d11_buffer = core.allocator.get_general_buffer(tiles_d3d11_buffer_id);
        let first_tile_map_buffer = core.allocator.get_general_buffer(first_tile_map_buffer_id);
        let z_buffer = core.allocator.get_general_buffer(z_buffer_id);

        let tile_count = core.framebuffer_tile_size().area();

        let timer_query = core.timer_query_cache.start_timing_draw_call(&core.device,
                                                                        &core.options);

        let dimensions = ComputeDimensions {
            x: (tile_count as u32 + SORT_WORKGROUP_SIZE - 1) / SORT_WORKGROUP_SIZE,
            y: 1,
            z: 1,
        };
        core.device.dispatch_compute(dimensions, &ComputeState {
            program: &sort_program.program,
            textures: &[],
            images: &[],
            uniforms: &[(&sort_program.tile_count_uniform, UniformData::Int(tile_count))],
            storage_buffers: &[
                (&sort_program.tiles_storage_buffer, tiles_d3d11_buffer),
                (&sort_program.first_tile_map_storage_buffer, first_tile_map_buffer),
                (&sort_program.z_buffer_storage_buffer, z_buffer),
            ],
        });

        core.stats.drawcall_count += 1;
        core.finish_timing_draw_call(&timer_query);
        core.current_timer.as_mut().unwrap().push_query(TimeCategory::Other, timer_query);
    }

    fn allocate_first_tile_map(&mut self, core: &mut RendererCore<D>) -> GeneralBufferID {
        core.allocator.allocate_general_buffer::<FirstTileD3D11>(&core.device,
                                                                 core.tile_size().area() as u64,
                                                                 BufferTag("FirstTileD3D11"))
    }

    fn allocate_alpha_tile_info(&mut self, core: &mut RendererCore<D>, index_count: u32)
                                -> GeneralBufferID {
        core.allocator.allocate_general_buffer::<AlphaTileD3D11>(&core.device,
                                                                 index_count as u64,
                                                                 BufferTag("AlphaTileD3D11"))
    }

    fn allocate_z_buffer(&mut self, core: &mut RendererCore<D>) -> GeneralBufferID {
        // This includes the fill indirect draw params because some drivers limit the number of
        // SSBOs to 8 (#373).
        let size = core.tile_size().area() as u64 + FILL_INDIRECT_DRAW_PARAMS_SIZE as u64;
        core.allocator.allocate_general_buffer::<i32>(&core.device,
                                                      size,
                                                      BufferTag("ZBufferD3D11"))
    }

    pub(crate) fn draw_tiles(&mut self,
                             core: &mut RendererCore<D>,
                             tiles_d3d11_buffer_id: GeneralBufferID,
                             first_tile_map_buffer_id: GeneralBufferID,
                             color_texture_0: Option<TileBatchTexture>) {
        let timer_query = core.timer_query_cache.start_timing_draw_call(&core.device,
                                                                        &core.options);

        let tile_program = &self.programs.tile_program;

        let (mut textures, mut uniforms, mut images) = (vec![], vec![], vec![]);

        core.set_uniforms_for_drawing_tiles(&tile_program.common,
                                            &mut textures,
                                            &mut uniforms,
                                            color_texture_0);

        uniforms.push((&tile_program.framebuffer_tile_size_uniform,
                       UniformData::IVec2(core.framebuffer_tile_size().0)));

        match core.draw_render_target() {
            RenderTarget::Default => panic!("Can't draw to the default framebuffer with compute!"),
            RenderTarget::Framebuffer(ref framebuffer) => {
                let dest_texture = core.device.framebuffer_texture(framebuffer);
                images.push((&tile_program.dest_image, dest_texture, ImageAccess::ReadWrite));
            }
        }

        let clear_color = core.clear_color_for_draw_operation();
        match clear_color {
            None => {
                uniforms.push((&tile_program.load_action_uniform,
                               UniformData::Int(LOAD_ACTION_LOAD)));
                uniforms.push((&tile_program.clear_color_uniform,
                               UniformData::Vec4(F32x4::default())));
            }
            Some(clear_color) => {
                uniforms.push((&tile_program.load_action_uniform,
                               UniformData::Int(LOAD_ACTION_CLEAR)));
                uniforms.push((&tile_program.clear_color_uniform,
                               UniformData::Vec4(clear_color.0)));
            }
        }

        let tiles_d3d11_buffer = core.allocator.get_general_buffer(tiles_d3d11_buffer_id);
        let first_tile_map_storage_buffer = core.allocator
                                                .get_general_buffer(first_tile_map_buffer_id);

        let framebuffer_tile_size = core.framebuffer_tile_size().0;
        let compute_dimensions = ComputeDimensions {
            x: framebuffer_tile_size.x() as u32,
            y: framebuffer_tile_size.y() as u32,
            z: 1,
        };

        core.device.dispatch_compute(compute_dimensions, &ComputeState {
            program: &tile_program.common.program,
            textures: &textures,
            images: &images,
            storage_buffers: &[
                (&tile_program.tiles_storage_buffer, tiles_d3d11_buffer),
                (&tile_program.first_tile_map_storage_buffer, first_tile_map_storage_buffer),
            ],
            uniforms: &uniforms,
        });

        core.stats.drawcall_count += 1;
        core.finish_timing_draw_call(&timer_query);
        core.current_timer.as_mut().unwrap().push_query(TimeCategory::Composite, timer_query);

        core.preserve_draw_framebuffer();
    }

    pub(crate) fn end_frame(&mut self, core: &mut RendererCore<D>) {
        self.free_tile_batch_buffers(core);
    }

    fn free_tile_batch_buffers(&mut self, core: &mut RendererCore<D>) {
        for (_, tile_batch_info) in self.tile_batch_info.drain() {
            core.allocator.free_general_buffer(tile_batch_info.z_buffer_id);
            core.allocator.free_general_buffer(tile_batch_info.tiles_d3d11_buffer_id);
            core.allocator.free_general_buffer(tile_batch_info.propagate_metadata_buffer_id);
            core.allocator.free_general_buffer(tile_batch_info.first_tile_map_buffer_id);
        }
    }
}

// Buffer data

#[derive(Clone)]
struct TileBatchInfoD3D11 {
    tile_count: u32,
    z_buffer_id: GeneralBufferID,
    tiles_d3d11_buffer_id: GeneralBufferID,
    propagate_metadata_buffer_id: GeneralBufferID,
    first_tile_map_buffer_id: GeneralBufferID,
}

#[derive(Clone)]
struct FillBufferInfoD3D11 {
    fill_vertex_buffer_id: GeneralBufferID,
}

#[derive(Debug)]
struct PropagateMetadataBufferIDsD3D11 {
    propagate_metadata: GeneralBufferID,
    backdrops: GeneralBufferID,
}

struct MicrolinesBufferIDsD3D11 {
    buffer_id: GeneralBufferID,
    count: u32,
}

#[derive(Clone, Debug)]
struct ClipBufferIDs {
    metadata: Option<GeneralBufferID>,
    tiles: GeneralBufferID,
}

struct SceneBuffers {
    draw: SceneSourceBuffers,
    clip: SceneSourceBuffers,
}

struct SceneSourceBuffers {
    points_buffer: Option<GeneralBufferID>,
    points_capacity: u32,
    point_indices_buffer: Option<GeneralBufferID>,
    point_indices_count: u32,
    point_indices_capacity: u32,
}

#[derive(Clone)]
struct PropagateTilesInfoD3D11 {
    alpha_tile_range: Range<u32>,
}

impl SceneBuffers {
    fn new() -> SceneBuffers {
        SceneBuffers { draw: SceneSourceBuffers::new(), clip: SceneSourceBuffers::new() }
    }

    fn upload<D>(&mut self,
                 allocator: &mut GPUMemoryAllocator<D>,
                 device: &D,
                 draw_segments: &SegmentsD3D11,
                 clip_segments: &SegmentsD3D11)
                 where D: Device {
        self.draw.upload(allocator, device, draw_segments);
        self.clip.upload(allocator, device, clip_segments);
    }
}

impl SceneSourceBuffers {
    fn new() -> SceneSourceBuffers {
        SceneSourceBuffers {
            points_buffer: None,
            points_capacity: 0,
            point_indices_buffer: None,
            point_indices_count: 0,
            point_indices_capacity: 0,
        }
    }

    fn upload<D>(&mut self,
                 allocator: &mut GPUMemoryAllocator<D>,
                 device: &D,
                 segments: &SegmentsD3D11)
                 where D: Device {
        let needed_points_capacity = (segments.points.len() as u32).next_power_of_two();
        let needed_point_indices_capacity = (segments.indices.len() as u32).next_power_of_two();
        if self.points_capacity < needed_points_capacity {
            self.points_buffer =
                Some(allocator.allocate_general_buffer::<Vector2F>(device,
                                                                   needed_points_capacity as u64,
                                                                   BufferTag("PointsD3D11")));
            self.points_capacity = needed_points_capacity;
        }
        if self.point_indices_capacity < needed_point_indices_capacity {
            self.point_indices_buffer =
                Some(allocator.allocate_general_buffer::<SegmentIndicesD3D11>(
                    device,
                    needed_point_indices_capacity as u64,
                    BufferTag("PointIndicesD3D11")));
            self.point_indices_capacity = needed_point_indices_capacity;
        }
        device.upload_to_buffer(allocator.get_general_buffer(self.points_buffer.unwrap()),
                                0,
                                &segments.points,
                                BufferTarget::Storage);
        device.upload_to_buffer(allocator.get_general_buffer(self.point_indices_buffer.unwrap()),
                                0,
                                &segments.indices,
                                BufferTarget::Storage);
        self.point_indices_count = segments.indices.len() as u32;
    }
}
