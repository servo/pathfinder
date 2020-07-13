// pathfinder/renderer/src/gpu/d3d9/renderer.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A hybrid CPU-GPU renderer that only relies on functionality available in Direct3D 9.
//! 
//! This renderer supports OpenGL at least 3.0, OpenGL ES at least 3.0, Metal of any version, and
//! WebGL at least 2.0.

use crate::gpu::blend::{BlendModeExt, ToBlendState};
use crate::gpu::perf::TimeCategory;
use crate::gpu::renderer::{FramebufferFlags, MASK_FRAMEBUFFER_HEIGHT, MASK_FRAMEBUFFER_WIDTH};
use crate::gpu::renderer::{RendererCore, RendererFlags};
use crate::gpu::d3d9::shaders::{ClipTileCombineVertexArrayD3D9, ClipTileCopyVertexArrayD3D9};
use crate::gpu::d3d9::shaders::{CopyTileVertexArray, FillVertexArrayD3D9};
use crate::gpu::d3d9::shaders::{ProgramsD3D9, TileVertexArrayD3D9};
use crate::gpu_data::{Clip, DrawTileBatchD3D9, Fill, TileBatchTexture, TileObjectPrimitive};
use crate::tile_map::DenseTileMap;
use crate::tiles::{TILE_HEIGHT, TILE_WIDTH};
use byte_slice_cast::AsByteSlice;
use pathfinder_color::ColorF;
use pathfinder_content::effects::BlendMode;
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::transform3d::Transform4F;
use pathfinder_geometry::vector::{Vector2I, Vector4F, vec2i};
use pathfinder_gpu::allocator::{BufferTag, FramebufferID, FramebufferTag, GeneralBufferID};
use pathfinder_gpu::allocator::{IndexBufferID, TextureID, TextureTag};
use pathfinder_gpu::{BlendFactor, BlendState, BufferTarget, ClearOps, Device, Primitive};
use pathfinder_gpu::{RenderOptions, RenderState, RenderTarget, StencilFunc, StencilState};
use pathfinder_gpu::{TextureDataRef, TextureFormat, UniformData};
use pathfinder_resources::ResourceLoader;
use pathfinder_simd::default::F32x2;
use std::u32;

const MAX_FILLS_PER_BATCH: usize = 0x10000;

pub(crate) struct RendererD3D9<D> where D: Device {
    // Basic data
    programs: ProgramsD3D9<D>,
    quads_vertex_indices_buffer_id: Option<IndexBufferID>,
    quads_vertex_indices_length: usize,

    // Fills.
    buffered_fills: Vec<Fill>,
    pending_fills: Vec<Fill>,

    // Temporary framebuffers
    dest_blend_framebuffer_id: FramebufferID,
}

impl<D> RendererD3D9<D> where D: Device {
    pub(crate) fn new(core: &mut RendererCore<D>, resources: &dyn ResourceLoader)
                      -> RendererD3D9<D> {
        let programs = ProgramsD3D9::new(&core.device, resources);

        let window_size = core.options.dest.window_size(&core.device);
        let dest_blend_framebuffer_id =
            core.allocator.allocate_framebuffer(&core.device,
                                                window_size,
                                                TextureFormat::RGBA8,
                                                FramebufferTag("DestBlendD3D9"));

        RendererD3D9 {
            programs,
            quads_vertex_indices_buffer_id: None,
            quads_vertex_indices_length: 0,

            buffered_fills: vec![],
            pending_fills: vec![],

            dest_blend_framebuffer_id,
        }
    }

    pub(crate) fn upload_and_draw_tiles(&mut self,
                                        core: &mut RendererCore<D>,
                                        batch: &DrawTileBatchD3D9) {
        if !batch.clips.is_empty() {
            let clip_buffer_info = self.upload_clip_tiles(core, &batch.clips);
            self.clip_tiles(core, &clip_buffer_info);
            core.allocator.free_general_buffer(clip_buffer_info.clip_buffer_id);
        }

        let tile_buffer = self.upload_tiles(core, &batch.tiles);
        let z_buffer_texture_id = self.upload_z_buffer(core, &batch.z_buffer_data);

        self.draw_tiles(core,
                        batch.tiles.len() as u32,
                        tile_buffer.tile_vertex_buffer_id,
                        batch.color_texture,
                        batch.blend_mode,
                        z_buffer_texture_id);

        core.allocator.free_texture(z_buffer_texture_id);
        core.allocator.free_general_buffer(tile_buffer.tile_vertex_buffer_id);
    }

    fn upload_tiles(&mut self, core: &mut RendererCore<D>, tiles: &[TileObjectPrimitive])
                    -> TileBufferD3D9 {
        let tile_vertex_buffer_id =
            core.allocator.allocate_general_buffer::<TileObjectPrimitive>(&core.device,
                                                                          tiles.len() as u64,
                                                                          BufferTag("TileD3D9"));
        let tile_vertex_buffer = &core.allocator.get_general_buffer(tile_vertex_buffer_id);
        core.device.upload_to_buffer(tile_vertex_buffer, 0, tiles, BufferTarget::Vertex);
        self.ensure_index_buffer(core, tiles.len());

        TileBufferD3D9 { tile_vertex_buffer_id }
    }


    fn ensure_index_buffer(&mut self, core: &mut RendererCore<D>, mut length: usize) {
        length = length.next_power_of_two();
        if self.quads_vertex_indices_length >= length {
            return;
        }

        // TODO(pcwalton): Generate these with SIMD.
        let mut indices: Vec<u32> = Vec::with_capacity(length * 6);
        for index in 0..(length as u32) {
            indices.extend_from_slice(&[
                index * 4 + 0, index * 4 + 1, index * 4 + 2,
                index * 4 + 1, index * 4 + 3, index * 4 + 2,
            ]);
        }

        if let Some(quads_vertex_indices_buffer_id) = self.quads_vertex_indices_buffer_id.take() {
            core.allocator.free_index_buffer(quads_vertex_indices_buffer_id);
        }
        let quads_vertex_indices_buffer_id =
            core.allocator.allocate_index_buffer::<u32>(&core.device,
                                                        indices.len() as u64,
                                                        BufferTag("QuadsVertexIndicesD3D9"));
        let quads_vertex_indices_buffer =
            core.allocator.get_index_buffer(quads_vertex_indices_buffer_id);
        core.device.upload_to_buffer(quads_vertex_indices_buffer,
                                     0,
                                     &indices,
                                     BufferTarget::Index);
        self.quads_vertex_indices_buffer_id = Some(quads_vertex_indices_buffer_id);
        self.quads_vertex_indices_length = length;
    }

    pub(crate) fn add_fills(&mut self, core: &mut RendererCore<D>, fill_batch: &[Fill]) {
        if fill_batch.is_empty() {
            return;
        }

        core.stats.fill_count += fill_batch.len();

        let preserve_alpha_mask_contents = core.alpha_tile_count > 0;

        self.pending_fills.reserve(fill_batch.len());
        for fill in fill_batch {
            core.alpha_tile_count = core.alpha_tile_count.max(fill.link + 1);
            self.pending_fills.push(*fill);
        }

        core.stats.alpha_tile_count = core.alpha_tile_count as usize;

        core.reallocate_alpha_tile_pages_if_necessary(preserve_alpha_mask_contents);

        if self.buffered_fills.len() + self.pending_fills.len() > MAX_FILLS_PER_BATCH {
            self.draw_buffered_fills(core);
        }

        self.buffered_fills.extend(self.pending_fills.drain(..));
    }

    pub(crate) fn draw_buffered_fills(&mut self, core: &mut RendererCore<D>) {
        if self.buffered_fills.is_empty() {
            return;
        }

        let fill_storage_info = self.upload_buffered_fills(core);
        self.draw_fills(core, fill_storage_info.fill_buffer_id, fill_storage_info.fill_count);
        core.allocator.free_general_buffer(fill_storage_info.fill_buffer_id);
    }

    fn upload_buffered_fills(&mut self, core: &mut RendererCore<D>) -> FillBufferInfoD3D9 {
        let buffered_fills = &mut self.buffered_fills;
        debug_assert!(!buffered_fills.is_empty());

        let fill_buffer_id = core.allocator
                                 .allocate_general_buffer::<Fill>(&core.device,
                                                                  MAX_FILLS_PER_BATCH as u64,
                                                                  BufferTag("Fill"));
        let fill_vertex_buffer = core.allocator.get_general_buffer(fill_buffer_id);
        debug_assert!(buffered_fills.len() <= u32::MAX as usize);
        core.device.upload_to_buffer(fill_vertex_buffer, 0, &buffered_fills, BufferTarget::Vertex);

        let fill_count = buffered_fills.len() as u32;
        buffered_fills.clear();

        FillBufferInfoD3D9 { fill_buffer_id, fill_count }
    }

    fn draw_fills(&mut self,
                  core: &mut RendererCore<D>,
                  fill_buffer_id: GeneralBufferID,
                  fill_count: u32) {
        let fill_raster_program = &self.programs.fill_program;

        let fill_vertex_buffer = core.allocator.get_general_buffer(fill_buffer_id);
        let quad_vertex_positions_buffer =
            core.allocator.get_general_buffer(core.quad_vertex_positions_buffer_id);
        let quad_vertex_indices_buffer = core.allocator
                                             .get_index_buffer(core.quad_vertex_indices_buffer_id);

        let area_lut_texture = core.allocator.get_texture(core.area_lut_texture_id);

        let mask_viewport = self.mask_viewport(core);
        let mask_storage = core.mask_storage.as_ref().expect("Where's the mask storage?");
        let mask_framebuffer_id = mask_storage.framebuffer_id;
        let mask_framebuffer = core.allocator.get_framebuffer(mask_framebuffer_id);

        let fill_vertex_array = FillVertexArrayD3D9::new(&core.device,
                                                         fill_raster_program,
                                                         fill_vertex_buffer,
                                                         quad_vertex_positions_buffer,
                                                         quad_vertex_indices_buffer);

        let mut clear_color = None;
        if !core.framebuffer_flags.contains(FramebufferFlags::MASK_FRAMEBUFFER_IS_DIRTY) {
            clear_color = Some(ColorF::default());
        };

        let timer_query = core.timer_query_cache.start_timing_draw_call(&core.device,
                                                                        &core.options);

        core.device.draw_elements_instanced(6, fill_count, &RenderState {
            target: &RenderTarget::Framebuffer(mask_framebuffer),
            program: &fill_raster_program.program,
            vertex_array: &fill_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &[(&fill_raster_program.area_lut_texture, area_lut_texture)],
            uniforms: &[
                (&fill_raster_program.framebuffer_size_uniform,
                 UniformData::Vec2(mask_viewport.size().to_f32().0)),
                (&fill_raster_program.tile_size_uniform,
                 UniformData::Vec2(F32x2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32))),
            ],
            images: &[],
            storage_buffers: &[],
            viewport: mask_viewport,
            options: RenderOptions {
                blend: Some(BlendState {
                    src_rgb_factor: BlendFactor::One,
                    src_alpha_factor: BlendFactor::One,
                    dest_rgb_factor: BlendFactor::One,
                    dest_alpha_factor: BlendFactor::One,
                    ..BlendState::default()
                }),
                clear_ops: ClearOps { color: clear_color, ..ClearOps::default() },
                ..RenderOptions::default()
            },
        });

        core.stats.drawcall_count += 1;
        core.finish_timing_draw_call(&timer_query);
        core.current_timer.as_mut().unwrap().push_query(TimeCategory::Fill, timer_query);

        core.framebuffer_flags.insert(FramebufferFlags::MASK_FRAMEBUFFER_IS_DIRTY);
    }

    fn clip_tiles(&mut self, core: &mut RendererCore<D>, clip_buffer_info: &ClipBufferInfo) {
        // Allocate temp mask framebuffer.
        let mask_temp_framebuffer_id =
            core.allocator.allocate_framebuffer(&core.device,
                                                self.mask_viewport(core).size(),
                                                core.mask_texture_format(),
                                                FramebufferTag("TempClipMaskD3D9"));
        let mask_temp_framebuffer = core.allocator.get_framebuffer(mask_temp_framebuffer_id);

        let mask_storage = core.mask_storage.as_ref().expect("Where's the mask storage?");
        let mask_framebuffer_id = mask_storage.framebuffer_id;
        let mask_framebuffer = core.allocator.get_framebuffer(mask_framebuffer_id);
        let mask_texture = core.device.framebuffer_texture(mask_framebuffer);
        let mask_texture_size = core.device.texture_size(&mask_texture);

        let clip_vertex_buffer = core.allocator
                                     .get_general_buffer(clip_buffer_info.clip_buffer_id);
        let quad_vertex_positions_buffer =
            core.allocator.get_general_buffer(core.quad_vertex_positions_buffer_id);
        let quad_vertex_indices_buffer = core.allocator
                                             .get_index_buffer(core.quad_vertex_indices_buffer_id);

        let tile_clip_copy_vertex_array =   
            ClipTileCopyVertexArrayD3D9::new(&core.device,
                                             &self.programs.tile_clip_copy_program,
                                             clip_vertex_buffer,
                                             quad_vertex_positions_buffer,
                                             quad_vertex_indices_buffer);
        let tile_clip_combine_vertex_array =   
            ClipTileCombineVertexArrayD3D9::new(&core.device,
                                                &self.programs.tile_clip_combine_program,
                                                clip_vertex_buffer,
                                                quad_vertex_positions_buffer,
                                                quad_vertex_indices_buffer);

        let timer_query = core.timer_query_cache.start_timing_draw_call(&core.device,
                                                                        &core.options);

        // Copy out tiles.
        //
        // TODO(pcwalton): Don't do this on GL4.
        core.device.draw_elements_instanced(6, clip_buffer_info.clip_count * 2, &RenderState {
            target: &RenderTarget::Framebuffer(mask_temp_framebuffer),
            program: &self.programs.tile_clip_copy_program.program,
            vertex_array: &tile_clip_copy_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &[
                (&self.programs.tile_clip_copy_program.src_texture,
                 core.device.framebuffer_texture(mask_framebuffer)),
            ],
            images: &[],
            uniforms: &[
                (&self.programs.tile_clip_copy_program.framebuffer_size_uniform,
                 UniformData::Vec2(mask_texture_size.to_f32().0)),
            ],
            storage_buffers: &[],
            viewport: RectI::new(Vector2I::zero(), mask_texture_size),
            options: RenderOptions::default(),
        });

        core.stats.drawcall_count += 1;
        core.finish_timing_draw_call(&timer_query);
        core.current_timer.as_mut().unwrap().push_query(TimeCategory::Other, timer_query);
        let timer_query = core.timer_query_cache.start_timing_draw_call(&core.device,
                                                                        &core.options);

        // Combine clip tiles.
        core.device.draw_elements_instanced(6, clip_buffer_info.clip_count, &RenderState {
            target: &RenderTarget::Framebuffer(mask_framebuffer),
            program: &self.programs.tile_clip_combine_program.program,
            vertex_array: &tile_clip_combine_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &[
                (&self.programs.tile_clip_combine_program.src_texture,
                 core.device.framebuffer_texture(&mask_temp_framebuffer)),
            ],
            images: &[],
            uniforms: &[
                (&self.programs.tile_clip_combine_program.framebuffer_size_uniform,
                 UniformData::Vec2(mask_texture_size.to_f32().0)),
            ],
            storage_buffers: &[],
            viewport: RectI::new(Vector2I::zero(), mask_texture_size),
            options: RenderOptions::default(),
        });

        core.stats.drawcall_count += 1;
        core.finish_timing_draw_call(&timer_query);
        core.current_timer.as_mut().unwrap().push_query(TimeCategory::Other, timer_query);

        core.allocator.free_framebuffer(mask_temp_framebuffer_id);
    }

    fn upload_z_buffer(&mut self, core: &mut RendererCore<D>, z_buffer_map: &DenseTileMap<i32>)
                       -> TextureID {
        let z_buffer_texture_id = core.allocator.allocate_texture(&core.device,
                                                                  z_buffer_map.rect.size(),
                                                                  TextureFormat::RGBA8,
                                                                  TextureTag("ZBufferD3D9"));
        let z_buffer_texture = core.allocator.get_texture(z_buffer_texture_id);
        debug_assert_eq!(z_buffer_map.rect.origin(), Vector2I::default());
        let z_data: &[u8] = z_buffer_map.data.as_byte_slice();
        core.device.upload_to_texture(z_buffer_texture,
                                      z_buffer_map.rect,
                                      TextureDataRef::U8(&z_data));
        z_buffer_texture_id
    }

    // Uploads clip tiles from CPU to GPU.
    fn upload_clip_tiles(&mut self, core: &mut RendererCore<D>, clips: &[Clip]) -> ClipBufferInfo {
        let clip_buffer_id = core.allocator.allocate_general_buffer::<Clip>(&core.device,
                                                                            clips.len() as u64,
                                                                            BufferTag("ClipD3D9"));
        let clip_buffer = core.allocator.get_general_buffer(clip_buffer_id);
        core.device.upload_to_buffer(clip_buffer, 0, clips, BufferTarget::Vertex);
        ClipBufferInfo { clip_buffer_id, clip_count: clips.len() as u32 }
    }

    fn draw_tiles(&mut self,
                  core: &mut RendererCore<D>,
                  tile_count: u32,
                  tile_vertex_buffer_id: GeneralBufferID,
                  color_texture_0: Option<TileBatchTexture>,
                  blend_mode: BlendMode,
                  z_buffer_texture_id: TextureID) {
        // TODO(pcwalton): Disable blend for solid tiles.

        if tile_count == 0 {
            return;
        }

        core.stats.total_tile_count += tile_count as usize;

        let needs_readable_framebuffer = blend_mode.needs_readable_framebuffer();
        if needs_readable_framebuffer {
            self.copy_alpha_tiles_to_dest_blend_texture(core, tile_count, tile_vertex_buffer_id);
        }

        let clear_color = core.clear_color_for_draw_operation();
        let draw_viewport = core.draw_viewport();

        let timer_query = core.timer_query_cache.start_timing_draw_call(&core.device,
                                                                        &core.options);

        let tile_raster_program = &self.programs.tile_program;

        let tile_vertex_buffer = core.allocator.get_general_buffer(tile_vertex_buffer_id);
        let quad_vertex_positions_buffer =
            core.allocator.get_general_buffer(core.quad_vertex_positions_buffer_id);
        let quad_vertex_indices_buffer = core.allocator
                                             .get_index_buffer(core.quad_vertex_indices_buffer_id);
        let dest_blend_framebuffer = core.allocator
                                         .get_framebuffer(self.dest_blend_framebuffer_id);

        let (mut textures, mut uniforms) = (vec![], vec![]);

        core.set_uniforms_for_drawing_tiles(&tile_raster_program.common,
                                            &mut textures,
                                            &mut uniforms,
                                            color_texture_0);

        uniforms.push((&tile_raster_program.transform_uniform,
                       UniformData::Mat4(self.tile_transform(core).to_columns())));
        textures.push((&tile_raster_program.dest_texture,
                        core.device.framebuffer_texture(dest_blend_framebuffer)));

        let z_buffer_texture = core.allocator.get_texture(z_buffer_texture_id);
        textures.push((&tile_raster_program.common.z_buffer_texture, z_buffer_texture));
        uniforms.push((&tile_raster_program.common.z_buffer_texture_size_uniform,
                       UniformData::IVec2(core.device.texture_size(z_buffer_texture).0)));

        let tile_vertex_array = TileVertexArrayD3D9::new(&core.device,
                                                         &self.programs.tile_program,
                                                         tile_vertex_buffer,
                                                         quad_vertex_positions_buffer,
                                                         quad_vertex_indices_buffer);

        core.device.draw_elements_instanced(6, tile_count, &RenderState {
            target: &core.draw_render_target(),
            program: &tile_raster_program.common.program,
            vertex_array: &tile_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &textures,
            images: &[],
            storage_buffers: &[],
            uniforms: &uniforms,
            viewport: draw_viewport,
            options: RenderOptions {
                blend: blend_mode.to_blend_state(),
                stencil: self.stencil_state(core),
                clear_ops: ClearOps { color: clear_color, ..ClearOps::default() },
                ..RenderOptions::default()
            },
        });

        core.stats.drawcall_count += 1;
        core.finish_timing_draw_call(&timer_query);
        core.current_timer.as_mut().unwrap().push_query(TimeCategory::Composite, timer_query);

        core.preserve_draw_framebuffer();
    }

    fn copy_alpha_tiles_to_dest_blend_texture(&mut self,
                                              core: &mut RendererCore<D>,
                                              tile_count: u32,
                                              vertex_buffer_id: GeneralBufferID) {
        let draw_viewport = core.draw_viewport();

        let mut textures = vec![];
        let mut uniforms = vec![
            (&self.programs.tile_copy_program.transform_uniform,
             UniformData::Mat4(self.tile_transform(core).to_columns())),
            (&self.programs.tile_copy_program.tile_size_uniform,
             UniformData::Vec2(F32x2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32))),
        ];

        let draw_framebuffer = match core.draw_render_target() {
            RenderTarget::Framebuffer(framebuffer) => framebuffer,
            RenderTarget::Default => panic!("Can't copy alpha tiles from default framebuffer!"),
        };
        let draw_texture = core.device.framebuffer_texture(&draw_framebuffer);

        textures.push((&self.programs.tile_copy_program.src_texture, draw_texture));
        uniforms.push((&self.programs.tile_copy_program.framebuffer_size_uniform,
                       UniformData::Vec2(draw_viewport.size().to_f32().0)));

        let quads_vertex_indices_buffer_id = self.quads_vertex_indices_buffer_id
                                                 .expect("Where's the quads vertex buffer?");
        let quads_vertex_indices_buffer = core.allocator
                                              .get_index_buffer(quads_vertex_indices_buffer_id);
        let vertex_buffer = core.allocator.get_general_buffer(vertex_buffer_id);

        let tile_copy_vertex_array = CopyTileVertexArray::new(&core.device,
                                                              &self.programs.tile_copy_program,
                                                              vertex_buffer,
                                                              quads_vertex_indices_buffer);

        let dest_blend_framebuffer = core.allocator
                                         .get_framebuffer(self.dest_blend_framebuffer_id);

        core.device.draw_elements(tile_count * 6, &RenderState {
            target: &RenderTarget::Framebuffer(dest_blend_framebuffer),
            program: &self.programs.tile_copy_program.program,
            vertex_array: &tile_copy_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &textures,
            images: &[],
            storage_buffers: &[],
            uniforms: &uniforms,
            viewport: draw_viewport,
            options: RenderOptions {
                clear_ops: ClearOps {
                    color: Some(ColorF::new(1.0, 0.0, 0.0, 1.0)),
                    ..ClearOps::default()
                },
                ..RenderOptions::default()
            },
        });

        core.stats.drawcall_count += 1;
    }

    fn stencil_state(&self, core: &RendererCore<D>) -> Option<StencilState> {
        if !core.renderer_flags.contains(RendererFlags::USE_DEPTH) {
            return None;
        }

        Some(StencilState {
            func: StencilFunc::Equal,
            reference: 1,
            mask: 1,
            write: false,
        })
    }

    fn mask_viewport(&self, core: &RendererCore<D>) -> RectI {
        let page_count = match core.mask_storage {
            Some(ref mask_storage) => mask_storage.allocated_page_count as i32,
            None => 0,
        };
        let height = MASK_FRAMEBUFFER_HEIGHT * page_count;
        RectI::new(Vector2I::default(), vec2i(MASK_FRAMEBUFFER_WIDTH, height))
    }

    fn tile_transform(&self, core: &RendererCore<D>) -> Transform4F {
        let draw_viewport = core.draw_viewport().size().to_f32();
        let scale = Vector4F::new(2.0 / draw_viewport.x(), -2.0 / draw_viewport.y(), 1.0, 1.0);
        Transform4F::from_scale(scale).translate(Vector4F::new(-1.0, 1.0, 0.0, 1.0))
    }
}

#[derive(Clone)]
pub(crate) struct TileBatchInfoD3D9 {
    pub(crate) tile_count: u32,
    pub(crate) z_buffer_id: GeneralBufferID,
    tile_vertex_buffer_id: GeneralBufferID,
}

#[derive(Clone)]
struct FillBufferInfoD3D9 {
    fill_buffer_id: GeneralBufferID,
    fill_count: u32,
}

struct TileBufferD3D9 {
    tile_vertex_buffer_id: GeneralBufferID,
}

struct ClipBufferInfo {
    clip_buffer_id: GeneralBufferID,
    clip_count: u32,
}
