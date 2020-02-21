// pathfinder/renderer/src/gpu/renderer.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::gpu::debug::DebugUIPresenter;
use crate::gpu::options::{DestFramebuffer, RendererOptions};
use crate::gpu::shaders::{AlphaTileProgram, AlphaTileVertexArray, FillProgram, FillVertexArray};
use crate::gpu::shaders::{FilterBasicProgram, FilterBasicVertexArray, FilterTextProgram};
use crate::gpu::shaders::{FilterTextVertexArray, MAX_FILLS_PER_BATCH, MaskTileProgram};
use crate::gpu::shaders::{MaskTileVertexArray, ReprojectionProgram, ReprojectionVertexArray};
use crate::gpu::shaders::{SolidTileProgram, SolidTileVertexArray};
use crate::gpu::shaders::{StencilProgram, StencilVertexArray};
use crate::gpu_data::{AlphaTile, FillBatchPrimitive, MaskTile, PaintData};
use crate::gpu_data::{RenderCommand, SolidTileVertex};
use crate::tiles::{TILE_HEIGHT, TILE_WIDTH};
use pathfinder_color::{self as color, ColorF};
use pathfinder_content::effects::{BlendMode, CompositeOp, DefringingKernel, Effects, Filter};
use pathfinder_content::fill::FillRule;
use pathfinder_geometry::vector::{Vector2I, Vector4F};
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::transform3d::Transform4F;
use pathfinder_gpu::resources::ResourceLoader;
use pathfinder_gpu::{BlendFactor, BlendOp, BlendState, BufferData, BufferTarget, BufferUploadMode};
use pathfinder_gpu::{ClearOps, DepthFunc, DepthState, Device, Primitive, RenderOptions};
use pathfinder_gpu::{RenderState, RenderTarget, StencilFunc, StencilState, TextureDataRef};
use pathfinder_gpu::{TextureFormat, UniformData};
use pathfinder_simd::default::{F32x2, F32x4};
use std::cmp;
use std::collections::VecDeque;
use std::mem;
use std::ops::{Add, Div};
use std::time::Duration;
use std::u32;

static QUAD_VERTEX_POSITIONS: [u16; 8] = [0, 0, 1, 0, 1, 1, 0, 1];
static QUAD_VERTEX_INDICES: [u32; 6] = [0, 1, 3, 1, 2, 3];

pub(crate) const MASK_TILES_ACROSS: u32 = 256;
pub(crate) const MASK_TILES_DOWN: u32 = 256;

const TEXTURE_CACHE_SIZE: usize = 8;

// FIXME(pcwalton): Shrink this again!
const MASK_FRAMEBUFFER_WIDTH: i32 = TILE_WIDTH as i32 * MASK_TILES_ACROSS as i32;
const MASK_FRAMEBUFFER_HEIGHT: i32 = TILE_HEIGHT as i32 * MASK_TILES_DOWN as i32;

pub struct Renderer<D>
where
    D: Device,
{
    // Device
    pub device: D,

    // Core data
    dest_framebuffer: DestFramebuffer<D>,
    options: RendererOptions,
    fill_program: FillProgram<D>,
    mask_winding_tile_program: MaskTileProgram<D>,
    mask_evenodd_tile_program: MaskTileProgram<D>,
    solid_tile_program: SolidTileProgram<D>,
    alpha_tile_program: AlphaTileProgram<D>,
    mask_winding_tile_vertex_array: MaskTileVertexArray<D>,
    mask_evenodd_tile_vertex_array: MaskTileVertexArray<D>,
    solid_tile_vertex_array: SolidTileVertexArray<D>,
    alpha_tile_vertex_array: AlphaTileVertexArray<D>,
    area_lut_texture: D::Texture,
    quad_vertex_positions_buffer: D::Buffer,
    quad_vertex_indices_buffer: D::Buffer,
    quads_vertex_indices_buffer: D::Buffer,
    quads_vertex_indices_length: usize,
    fill_vertex_array: FillVertexArray<D>,
    fill_framebuffer: D::Framebuffer,
    mask_framebuffer: D::Framebuffer,
    paint_textures: Vec<D::Texture>,
    layer_framebuffer_stack: Vec<LayerFramebufferInfo<D>>,

    // This is a dummy texture consisting solely of a single `rgba(0, 0, 0, 255)` texel. It serves
    // as the paint texture when drawing alpha tiles with the Clear blend mode. If this weren't
    // used, then the transparent black paint would zero out the alpha mask.
    clear_paint_texture: D::Texture,

    // Filter shaders
    filter_basic_program: FilterBasicProgram<D>,
    filter_basic_vertex_array: FilterBasicVertexArray<D>,
    filter_text_program: FilterTextProgram<D>,
    filter_text_vertex_array: FilterTextVertexArray<D>,
    gamma_lut_texture: D::Texture,

    // Stencil shader
    stencil_program: StencilProgram<D>,
    stencil_vertex_array: StencilVertexArray<D>,

    // Reprojection shader
    reprojection_program: ReprojectionProgram<D>,
    reprojection_vertex_array: ReprojectionVertexArray<D>,

    // Rendering state
    framebuffer_flags: FramebufferFlags,
    buffered_fills: Vec<FillBatchPrimitive>,
    texture_cache: TextureCache<D>,

    // Debug
    pub stats: RenderStats,
    current_timers: RenderTimers<D>,
    pending_timers: VecDeque<RenderTimers<D>>,
    free_timer_queries: Vec<D::TimerQuery>,
    pub debug_ui_presenter: DebugUIPresenter<D>,

    // Extra info
    use_depth: bool,
}

impl<D> Renderer<D>
where
    D: Device,
{
    pub fn new(device: D,
               resources: &dyn ResourceLoader,
               dest_framebuffer: DestFramebuffer<D>,
               options: RendererOptions)
               -> Renderer<D> {
        let fill_program = FillProgram::new(&device, resources);
        let mask_winding_tile_program = MaskTileProgram::new(FillRule::Winding,
                                                             &device,
                                                             resources);
        let mask_evenodd_tile_program = MaskTileProgram::new(FillRule::EvenOdd,
                                                             &device,
                                                             resources);
        let solid_tile_program = SolidTileProgram::new(&device, resources);
        let alpha_tile_program = AlphaTileProgram::new(&device, resources);
        let filter_basic_program = FilterBasicProgram::new(&device, resources);
        let filter_text_program = FilterTextProgram::new(&device, resources);
        let stencil_program = StencilProgram::new(&device, resources);
        let reprojection_program = ReprojectionProgram::new(&device, resources);

        let area_lut_texture = device.create_texture_from_png(resources, "area-lut");
        let gamma_lut_texture = device.create_texture_from_png(resources, "gamma-lut");

        let quad_vertex_positions_buffer = device.create_buffer();
        device.allocate_buffer(
            &quad_vertex_positions_buffer,
            BufferData::Memory(&QUAD_VERTEX_POSITIONS),
            BufferTarget::Vertex,
            BufferUploadMode::Static,
        );
        let quad_vertex_indices_buffer = device.create_buffer();
        device.allocate_buffer(
            &quad_vertex_indices_buffer,
            BufferData::Memory(&QUAD_VERTEX_INDICES),
            BufferTarget::Index,
            BufferUploadMode::Static,
        );
        let quads_vertex_indices_buffer = device.create_buffer();

        let fill_vertex_array = FillVertexArray::new(
            &device,
            &fill_program,
            &quad_vertex_positions_buffer,
            &quad_vertex_indices_buffer,
        );
        let mask_winding_tile_vertex_array = MaskTileVertexArray::new(
            &device,
            &mask_winding_tile_program,
            &quads_vertex_indices_buffer,
        );
        let mask_evenodd_tile_vertex_array = MaskTileVertexArray::new(
            &device,
            &mask_evenodd_tile_program,
            &quads_vertex_indices_buffer,
        );
        let alpha_tile_vertex_array = AlphaTileVertexArray::new(
            &device,
            &alpha_tile_program,
            &quads_vertex_indices_buffer,
        );
        let solid_tile_vertex_array = SolidTileVertexArray::new(
            &device,
            &solid_tile_program,
            &quads_vertex_indices_buffer,
        );
        let filter_basic_vertex_array = FilterBasicVertexArray::new(
            &device,
            &filter_basic_program,
            &quad_vertex_positions_buffer,
            &quad_vertex_indices_buffer,
        );
        let filter_text_vertex_array = FilterTextVertexArray::new(
            &device,
            &filter_text_program,
            &quad_vertex_positions_buffer,
            &quad_vertex_indices_buffer,
        );
        let stencil_vertex_array = StencilVertexArray::new(&device, &stencil_program);
        let reprojection_vertex_array = ReprojectionVertexArray::new(
            &device,
            &reprojection_program,
            &quad_vertex_positions_buffer,
            &quad_vertex_indices_buffer,
        );

        let fill_framebuffer_size =
            Vector2I::new(MASK_FRAMEBUFFER_WIDTH, MASK_FRAMEBUFFER_HEIGHT);
        let fill_framebuffer_texture =
            device.create_texture(TextureFormat::R16F, fill_framebuffer_size);
        let fill_framebuffer = device.create_framebuffer(fill_framebuffer_texture);

        let mask_framebuffer_size =
            Vector2I::new(MASK_FRAMEBUFFER_WIDTH, MASK_FRAMEBUFFER_HEIGHT);
        let mask_framebuffer_texture =
            device.create_texture(TextureFormat::R8, mask_framebuffer_size);
        let mask_framebuffer = device.create_framebuffer(mask_framebuffer_texture);

        let clear_paint_texture =
            device.create_texture_from_data(TextureFormat::RGBA8,
                                            Vector2I::splat(1),
                                            TextureDataRef::U8(&[0, 0, 0, 255]));

        let window_size = dest_framebuffer.window_size(&device);
        let debug_ui_presenter = DebugUIPresenter::new(&device, resources, window_size);

        Renderer {
            device,

            dest_framebuffer,
            options,
            fill_program,
            mask_winding_tile_program,
            mask_evenodd_tile_program,
            solid_tile_program,
            alpha_tile_program,
            mask_winding_tile_vertex_array,
            mask_evenodd_tile_vertex_array,
            solid_tile_vertex_array,
            alpha_tile_vertex_array,
            area_lut_texture,
            quad_vertex_positions_buffer,
            quad_vertex_indices_buffer,
            quads_vertex_indices_buffer,
            quads_vertex_indices_length: 0,
            fill_vertex_array,
            fill_framebuffer,
            mask_framebuffer,
            paint_textures: vec![],
            layer_framebuffer_stack: vec![],
            clear_paint_texture,

            filter_basic_program,
            filter_basic_vertex_array,
            filter_text_program,
            filter_text_vertex_array,
            gamma_lut_texture,

            stencil_program,
            stencil_vertex_array,

            reprojection_program,
            reprojection_vertex_array,

            stats: RenderStats::default(),
            current_timers: RenderTimers::new(),
            pending_timers: VecDeque::new(),
            free_timer_queries: vec![],
            debug_ui_presenter,

            framebuffer_flags: FramebufferFlags::empty(),
            buffered_fills: vec![],
            texture_cache: TextureCache::new(),

            use_depth: false,
        }
    }

    pub fn begin_scene(&mut self) {
        self.framebuffer_flags = FramebufferFlags::empty();
        self.device.begin_commands();
        self.stats = RenderStats::default();
    }

    pub fn render_command(&mut self, command: &RenderCommand) {
        //println!("{:?}", command);
        match *command {
            RenderCommand::Start { bounding_quad, path_count } => {
                if self.use_depth {
                    self.draw_stencil(&bounding_quad);
                }
                self.stats.path_count = path_count;
            }
            RenderCommand::AddPaintData(ref paint_data) => self.upload_paint_data(paint_data),
            RenderCommand::AddFills(ref fills) => self.add_fills(fills),
            RenderCommand::FlushFills => {
                self.draw_buffered_fills();
                self.begin_composite_timer_query();
            }
            RenderCommand::RenderMaskTiles { tiles: ref mask_tiles, fill_rule } => {
                let count = mask_tiles.len();
                self.upload_mask_tiles(mask_tiles, fill_rule);
                self.draw_mask_tiles(count as u32, fill_rule);
            }
            RenderCommand::PushLayer { effects } => self.push_layer(effects),
            RenderCommand::PopLayer => self.pop_layer(),
            RenderCommand::DrawSolidTiles(ref batch) => {
                let count = batch.vertices.len() / 4;
                self.stats.solid_tile_count += count;
                self.upload_solid_tiles(&batch.vertices);
                self.draw_solid_tiles(count as u32, batch.paint_page);
            }
            RenderCommand::DrawAlphaTiles { tiles: ref alpha_tiles, paint_page, blend_mode } => {
                let count = alpha_tiles.len();
                self.stats.alpha_tile_count += count;
                self.upload_alpha_tiles(alpha_tiles);
                self.draw_alpha_tiles(count as u32, paint_page, blend_mode);
            }
            RenderCommand::Finish { .. } => {}
        }
    }

    pub fn end_scene(&mut self) {
        self.end_composite_timer_query();
        self.pending_timers.push_back(mem::replace(&mut self.current_timers, RenderTimers::new()));

        self.device.end_commands();
    }

    pub fn draw_debug_ui(&self) {
        self.debug_ui_presenter.draw(&self.device);
    }

    pub fn shift_rendering_time(&mut self) -> Option<RenderTime> {
        let timers = self.pending_timers.front()?;

        // Accumulate stage-0 time.
        let mut total_stage_0_time = Duration::new(0, 0);
        for timer_query in &timers.stage_0 {
            match self.device.try_recv_timer_query(timer_query) {
                None => return None,
                Some(stage_0_time) => total_stage_0_time += stage_0_time,
            }
        }

        // Get stage-1 time.
        let stage_1_time = {
            let stage_1_timer_query = timers.stage_1.as_ref().unwrap();
            match self.device.try_recv_timer_query(stage_1_timer_query) {
                None => return None,
                Some(query) => query,
            }
        };

        // Recycle all timer queries.
        let timers = self.pending_timers.pop_front().unwrap();
        self.free_timer_queries.extend(timers.stage_0.into_iter());
        self.free_timer_queries.push(timers.stage_1.unwrap());

        Some(RenderTime { stage_0: total_stage_0_time, stage_1: stage_1_time })
    }

    #[inline]
    pub fn dest_framebuffer(&self) -> &DestFramebuffer<D> {
        &self.dest_framebuffer
    }

    #[inline]
    pub fn replace_dest_framebuffer(
        &mut self,
        new_dest_framebuffer: DestFramebuffer<D>,
    ) -> DestFramebuffer<D> {
        mem::replace(&mut self.dest_framebuffer, new_dest_framebuffer)
    }

    #[inline]
    pub fn set_options(&mut self, new_options: RendererOptions) {
        self.options = new_options
    }

    #[inline]
    pub fn set_main_framebuffer_size(&mut self, new_framebuffer_size: Vector2I) {
        self.debug_ui_presenter.ui_presenter.set_framebuffer_size(new_framebuffer_size);
    }

    #[inline]
    pub fn disable_depth(&mut self) {
        self.use_depth = false;
    }

    #[inline]
    pub fn enable_depth(&mut self) {
        self.use_depth = true;
    }

    #[inline]
    pub fn quad_vertex_positions_buffer(&self) -> &D::Buffer {
        &self.quad_vertex_positions_buffer
    }

    #[inline]
    pub fn quad_vertex_indices_buffer(&self) -> &D::Buffer {
        &self.quad_vertex_indices_buffer
    }

    fn upload_paint_data(&mut self, paint_data: &PaintData) {
        for paint_texture in self.paint_textures.drain(..) {
            self.texture_cache.release_texture(paint_texture);
        }

        for paint_page_data in &paint_data.pages {
            let paint_size = paint_page_data.size;
            let paint_texels = &paint_page_data.texels;

            let paint_texture = self.texture_cache.create_texture(&mut self.device,
                                                                  TextureFormat::RGBA8,
                                                                  paint_size);

            let texels = color::color_slice_to_u8_slice(paint_texels);
            self.device.upload_to_texture(&paint_texture,
                                          RectI::new(Vector2I::default(), paint_size),
                                          TextureDataRef::U8(texels));

            self.paint_textures.push(paint_texture);
        }
    }

    fn upload_mask_tiles(&mut self, mask_tiles: &[MaskTile], fill_rule: FillRule) {
        let vertex_array = match fill_rule {
            FillRule::Winding => &self.mask_winding_tile_vertex_array,
            FillRule::EvenOdd => &self.mask_evenodd_tile_vertex_array,
        };

        self.device.allocate_buffer(
            &vertex_array.vertex_buffer,
            BufferData::Memory(&mask_tiles),
            BufferTarget::Vertex,
            BufferUploadMode::Dynamic,
        );
        self.ensure_index_buffer(mask_tiles.len());
    }

    fn upload_solid_tiles(&mut self, solid_tile_vertices: &[SolidTileVertex]) {
        self.device.allocate_buffer(
            &self.solid_tile_vertex_array.vertex_buffer,
            BufferData::Memory(&solid_tile_vertices),
            BufferTarget::Vertex,
            BufferUploadMode::Dynamic,
        );
        self.ensure_index_buffer(solid_tile_vertices.len() / 4);
    }

    fn upload_alpha_tiles(&mut self, alpha_tiles: &[AlphaTile]) {
        self.device.allocate_buffer(
            &self.alpha_tile_vertex_array.vertex_buffer,
            BufferData::Memory(&alpha_tiles),
            BufferTarget::Vertex,
            BufferUploadMode::Dynamic,
        );
        self.ensure_index_buffer(alpha_tiles.len());
    }

    fn ensure_index_buffer(&mut self, mut length: usize) {
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

        self.device.allocate_buffer(
            &self.quads_vertex_indices_buffer,
            BufferData::Memory(&indices),
            BufferTarget::Index,
            BufferUploadMode::Static,
        );

        self.quads_vertex_indices_length = length;
    }

    fn add_fills(&mut self, mut fills: &[FillBatchPrimitive]) {
        if fills.is_empty() {
            return;
        }

        self.stats.fill_count += fills.len();

        while !fills.is_empty() {
            let count = cmp::min(fills.len(), MAX_FILLS_PER_BATCH - self.buffered_fills.len());
            self.buffered_fills.extend_from_slice(&fills[0..count]);
            fills = &fills[count..];
            if self.buffered_fills.len() == MAX_FILLS_PER_BATCH {
                self.draw_buffered_fills();
            }
        }
    }

    fn draw_buffered_fills(&mut self) {
        if self.buffered_fills.is_empty() {
            return;
        }

        self.device.allocate_buffer(
            &self.fill_vertex_array.vertex_buffer,
            BufferData::Memory(&self.buffered_fills),
            BufferTarget::Vertex,
            BufferUploadMode::Dynamic,
        );

        let mut clear_color = None;
        if !self.framebuffer_flags.contains(
                FramebufferFlags::MUST_PRESERVE_FILL_FRAMEBUFFER_CONTENTS) {
            clear_color = Some(ColorF::default());
        };

        let timer_query = self.allocate_timer_query();
        self.device.begin_timer_query(&timer_query);

        debug_assert!(self.buffered_fills.len() <= u32::MAX as usize);
        self.device.draw_elements_instanced(6, self.buffered_fills.len() as u32, &RenderState {
            target: &RenderTarget::Framebuffer(&self.fill_framebuffer),
            program: &self.fill_program.program,
            vertex_array: &self.fill_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &[&self.area_lut_texture],
            uniforms: &[
                (&self.fill_program.framebuffer_size_uniform,
                 UniformData::Vec2(F32x2::new(MASK_FRAMEBUFFER_WIDTH as f32,
                                              MASK_FRAMEBUFFER_HEIGHT as f32))),
                (&self.fill_program.tile_size_uniform,
                 UniformData::Vec2(F32x2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32))),
                (&self.fill_program.area_lut_uniform, UniformData::TextureUnit(0)),
            ],
            viewport: self.mask_viewport(),
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

        self.device.end_timer_query(&timer_query);
        self.current_timers.stage_0.push(timer_query);

        self.framebuffer_flags.insert(FramebufferFlags::MUST_PRESERVE_FILL_FRAMEBUFFER_CONTENTS);
        self.buffered_fills.clear();
    }

    fn tile_transform(&self) -> Transform4F {
        let draw_viewport = self.draw_viewport().size().to_f32();
        let scale = Vector4F::new(2.0 / draw_viewport.x(), -2.0 / draw_viewport.y(), 1.0, 1.0);
        Transform4F::from_scale(scale).translate(Vector4F::new(-1.0, 1.0, 0.0, 1.0))
    }

    fn draw_mask_tiles(&mut self, tile_count: u32, fill_rule: FillRule) {
        let clear_color =
            if self.framebuffer_flags
                   .contains(FramebufferFlags::MUST_PRESERVE_MASK_FRAMEBUFFER_CONTENTS) {
                None
            } else {
                Some(ColorF::new(1.0, 1.0, 1.0, 1.0))
            };

        let (mask_tile_program, mask_tile_vertex_array) = match fill_rule {
            FillRule::Winding => {
                (&self.mask_winding_tile_program, &self.mask_winding_tile_vertex_array)
            }
            FillRule::EvenOdd => {
                (&self.mask_evenodd_tile_program, &self.mask_evenodd_tile_vertex_array)
            }
        };

        self.device.draw_elements(tile_count * 6, &RenderState {
            target: &RenderTarget::Framebuffer(&self.mask_framebuffer),
            program: &mask_tile_program.program,
            vertex_array: &mask_tile_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &[self.device.framebuffer_texture(&self.fill_framebuffer)],
            uniforms: &[
                (&self.mask_winding_tile_program.fill_texture_uniform,
                 UniformData::TextureUnit(0)),
            ],
            viewport: self.mask_viewport(),
            options: RenderOptions {
                blend: Some(BlendState {
                    src_rgb_factor: BlendFactor::One,
                    src_alpha_factor: BlendFactor::One,
                    dest_rgb_factor: BlendFactor::One,
                    dest_alpha_factor: BlendFactor::One,
                    op: BlendOp::Min,
                    ..BlendState::default()
                }),
                clear_ops: ClearOps { color: clear_color, ..ClearOps::default() },
                ..RenderOptions::default()
            },
        });

        self.framebuffer_flags.insert(FramebufferFlags::MUST_PRESERVE_MASK_FRAMEBUFFER_CONTENTS);
    }

    fn draw_alpha_tiles(&mut self, tile_count: u32, paint_page: u32, blend_mode: BlendMode) {
        let clear_color = self.clear_color_for_draw_operation();

        let mut textures = vec![self.device.framebuffer_texture(&self.mask_framebuffer)];
        let mut uniforms = vec![
            (&self.alpha_tile_program.transform_uniform,
             UniformData::Mat4(self.tile_transform().to_columns())),
            (&self.alpha_tile_program.tile_size_uniform,
             UniformData::Vec2(F32x2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32))),
            (&self.alpha_tile_program.stencil_texture_uniform, UniformData::TextureUnit(0)),
            (&self.alpha_tile_program.stencil_texture_size_uniform,
             UniformData::Vec2(F32x2::new(MASK_FRAMEBUFFER_WIDTH as f32,
                                          MASK_FRAMEBUFFER_HEIGHT as f32))),
        ];

        let paint_texture = match blend_mode {
            BlendMode::Clear => {
                // Use a special dummy paint texture containing `rgba(0, 0, 0, 255)` so that the
                // transparent black paint color doesn't zero out the mask.
                &self.clear_paint_texture
            }
            _ => &self.paint_textures[paint_page as usize],
        };

        textures.push(paint_texture);
        uniforms.push((&self.alpha_tile_program.paint_texture_uniform,
                        UniformData::TextureUnit(1)));
        uniforms.push((&self.alpha_tile_program.paint_texture_size_uniform,
                        UniformData::Vec2(self.device
                                              .texture_size(paint_texture)
                                              .0
                                              .to_f32x2())));

        self.device.draw_elements(tile_count * 6, &RenderState {
            target: &self.draw_render_target(),
            program: &self.alpha_tile_program.program,
            vertex_array: &self.alpha_tile_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &textures,
            uniforms: &uniforms,
            viewport: self.draw_viewport(),
            options: RenderOptions {
                blend: Some(blend_mode.to_blend_state()),
                stencil: self.stencil_state(),
                clear_ops: ClearOps { color: clear_color, ..ClearOps::default() },
                ..RenderOptions::default()
            },
        });

        self.preserve_draw_framebuffer();
    }

    fn draw_solid_tiles(&mut self, tile_count: u32, paint_page: u32) {
        let clear_color = self.clear_color_for_draw_operation();

        let mut textures = vec![];
        let mut uniforms = vec![
            (&self.solid_tile_program.transform_uniform,
             UniformData::Mat4(self.tile_transform().to_columns())),
            (&self.solid_tile_program.tile_size_uniform,
             UniformData::Vec2(F32x2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32))),
        ];

        let paint_texture = &self.paint_textures[paint_page as usize];
        textures.push(paint_texture);
        uniforms.push((&self.solid_tile_program.paint_texture_uniform,
                        UniformData::TextureUnit(0)));
        uniforms.push((&self.solid_tile_program.paint_texture_size_uniform,
                        UniformData::Vec2(self.device.texture_size(paint_texture).0.to_f32x2())));

        self.device.draw_elements(6 * tile_count, &RenderState {
            target: &self.draw_render_target(),
            program: &self.solid_tile_program.program,
            vertex_array: &self.solid_tile_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &textures,
            uniforms: &uniforms,
            viewport: self.draw_viewport(),
            options: RenderOptions {
                stencil: self.stencil_state(),
                clear_ops: ClearOps { color: clear_color, ..ClearOps::default() },
                ..RenderOptions::default()
            },
        });

        self.preserve_draw_framebuffer();
    }

    fn draw_stencil(&mut self, quad_positions: &[Vector4F]) {
        self.device.allocate_buffer(
            &self.stencil_vertex_array.vertex_buffer,
            BufferData::Memory(quad_positions),
            BufferTarget::Vertex,
            BufferUploadMode::Dynamic,
        );

        // Create indices for a triangle fan. (This is OK because the clipped quad should always be
        // convex.)
        let mut indices: Vec<u32> = vec![];
        for index in 1..(quad_positions.len() as u32 - 1) {
            indices.extend_from_slice(&[0, index as u32, index + 1]);
        }
        self.device.allocate_buffer(
            &self.stencil_vertex_array.index_buffer,
            BufferData::Memory(&indices),
            BufferTarget::Index,
            BufferUploadMode::Dynamic,
        );

        self.device.draw_elements(indices.len() as u32, &RenderState {
            target: &self.draw_render_target(),
            program: &self.stencil_program.program,
            vertex_array: &self.stencil_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &[],
            uniforms: &[],
            viewport: self.draw_viewport(),
            options: RenderOptions {
                // FIXME(pcwalton): Should we really write to the depth buffer?
                depth: Some(DepthState { func: DepthFunc::Less, write: true }),
                stencil: Some(StencilState {
                    func: StencilFunc::Always,
                    reference: 1,
                    mask: 1,
                    write: true,
                }),
                color_mask: false,
                clear_ops: ClearOps { stencil: Some(0), ..ClearOps::default() },
                ..RenderOptions::default()
            },
        });
    }

    pub fn reproject_texture(
        &mut self,
        texture: &D::Texture,
        old_transform: &Transform4F,
        new_transform: &Transform4F,
    ) {
        let clear_color = self.clear_color_for_draw_operation();

        self.device.draw_elements(6, &RenderState {
            target: &self.draw_render_target(),
            program: &self.reprojection_program.program,
            vertex_array: &self.reprojection_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &[texture],
            uniforms: &[
                (&self.reprojection_program.old_transform_uniform,
                 UniformData::from_transform_3d(old_transform)),
                (&self.reprojection_program.new_transform_uniform,
                 UniformData::from_transform_3d(new_transform)),
                (&self.reprojection_program.texture_uniform, UniformData::TextureUnit(0)),
            ],
            viewport: self.draw_viewport(),
            options: RenderOptions {
                blend: Some(BlendMode::SrcOver.to_blend_state()),
                depth: Some(DepthState { func: DepthFunc::Less, write: false, }),
                clear_ops: ClearOps { color: clear_color, ..ClearOps::default() },
                ..RenderOptions::default()
            },
        });

        self.preserve_draw_framebuffer();
    }

    pub fn draw_render_target(&self) -> RenderTarget<D> {
        match self.layer_framebuffer_stack.last() {
            Some(ref layer_framebuffer_info) => {
                RenderTarget::Framebuffer(&layer_framebuffer_info.framebuffer)
            }
            None => {
                match self.dest_framebuffer {
                    DestFramebuffer::Default { .. } => RenderTarget::Default,
                    DestFramebuffer::Other(ref framebuffer) => {
                        RenderTarget::Framebuffer(framebuffer)
                    }
                }
            }
        }
    }

    fn push_layer(&mut self, effects: Effects) {
        let main_framebuffer_size = self.main_viewport().size();
        let framebuffer_size = match effects.filter {
            Filter::Text { defringing_kernel: Some(_), .. } => {
                main_framebuffer_size.scale_xy(Vector2I::new(3, 1))
            }
            _ => main_framebuffer_size,
        };

        let texture = self.texture_cache.create_texture(&mut self.device,
                                                        TextureFormat::RGBA8,
                                                        framebuffer_size);
        let framebuffer = self.device.create_framebuffer(texture);

        self.layer_framebuffer_stack.push(LayerFramebufferInfo {
            framebuffer,
            effects,
            must_preserve_contents: false,
        });
    }

    fn pop_layer(&mut self) {
        let layer_framebuffer_info = self.layer_framebuffer_stack   
                                         .pop()
                                         .expect("Where's the layer?");

        match layer_framebuffer_info.effects.filter {
            Filter::Composite(composite_op) => {
                self.composite_layer(&layer_framebuffer_info, composite_op)
            }
            Filter::Text { fg_color, bg_color, defringing_kernel, gamma_correction } => {
                self.draw_text_layer(&layer_framebuffer_info,
                                     fg_color,
                                     bg_color,
                                     defringing_kernel,
                                     gamma_correction)
            }
        }

        self.preserve_draw_framebuffer();

        let texture = self.device.destroy_framebuffer(layer_framebuffer_info.framebuffer);
        self.texture_cache.release_texture(texture);
    }

    fn composite_layer(&self,
                       layer_framebuffer_info: &LayerFramebufferInfo<D>,
                       composite_op: CompositeOp) {
        let clear_color = self.clear_color_for_draw_operation();
        let source_framebuffer = &layer_framebuffer_info.framebuffer;
        let source_texture = self.device.framebuffer_texture(source_framebuffer);
        let source_texture_size = self.device.texture_size(source_texture);
        let main_viewport = self.main_viewport();

        let uniforms = vec![
            (&self.filter_basic_program.framebuffer_size_uniform,
             UniformData::Vec2(main_viewport.size().to_f32().0)),
            (&self.filter_basic_program.source_uniform, UniformData::TextureUnit(0)),
            (&self.filter_basic_program.source_size_uniform,
             UniformData::Vec2(source_texture_size.0.to_f32x2())),
        ];

        let blend_state = match composite_op {
            CompositeOp::SrcOver => BlendMode::SrcOver.to_blend_state(),
        };

        self.device.draw_elements(6, &RenderState {
            target: &self.draw_render_target(),
            program: &self.filter_basic_program.program,
            vertex_array: &self.filter_basic_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &[&source_texture],
            uniforms: &uniforms,
            viewport: main_viewport,
            options: RenderOptions {
                clear_ops: ClearOps { color: clear_color, ..ClearOps::default() },
                blend: Some(blend_state),
                ..RenderOptions::default()
            },
        });
    }

    fn draw_text_layer(&self,
                       layer_framebuffer_info: &LayerFramebufferInfo<D>,
                       fg_color: ColorF,
                       bg_color: ColorF,
                       defringing_kernel: Option<DefringingKernel>,
                       gamma_correction: bool) {
        let clear_color = self.clear_color_for_draw_operation();
        let source_framebuffer = &layer_framebuffer_info.framebuffer;
        let source_texture = self.device.framebuffer_texture(source_framebuffer);
        let source_texture_size = self.device.texture_size(source_texture);
        let main_viewport = self.main_viewport();

        let mut uniforms = vec![
            (&self.filter_text_program.framebuffer_size_uniform,
             UniformData::Vec2(main_viewport.size().to_f32().0)),
            (&self.filter_text_program.source_uniform, UniformData::TextureUnit(0)),
            (&self.filter_text_program.source_size_uniform,
             UniformData::Vec2(source_texture_size.0.to_f32x2())),
            (&self.filter_text_program.gamma_lut_uniform, UniformData::TextureUnit(1)),
            (&self.filter_text_program.fg_color_uniform, UniformData::Vec4(fg_color.0)),
            (&self.filter_text_program.bg_color_uniform, UniformData::Vec4(bg_color.0)),
            (&self.filter_text_program.gamma_correction_enabled_uniform,
             UniformData::Int(gamma_correction as i32)),
        ];

        match defringing_kernel {
            Some(ref kernel) => {
                uniforms.push((&self.filter_text_program.kernel_uniform,
                               UniformData::Vec4(F32x4::from_slice(&kernel.0))));
            }
            None => {
                uniforms.push((&self.filter_text_program.kernel_uniform,
                               UniformData::Vec4(F32x4::default())));
            }
        }

        self.device.draw_elements(6, &RenderState {
            target: &self.draw_render_target(),
            program: &self.filter_text_program.program,
            vertex_array: &self.filter_text_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &[&source_texture, &self.gamma_lut_texture],
            uniforms: &uniforms,
            viewport: main_viewport,
            options: RenderOptions {
                clear_ops: ClearOps { color: clear_color, ..ClearOps::default() },
                ..RenderOptions::default()
            },
        });
    }

    fn stencil_state(&self) -> Option<StencilState> {
        if !self.use_depth {
            return None;
        }

        Some(StencilState {
            func: StencilFunc::Equal,
            reference: 1,
            mask: 1,
            write: false,
        })
    }

    fn clear_color_for_draw_operation(&self) -> Option<ColorF> {
        let must_preserve_contents = match self.layer_framebuffer_stack.last() {
            Some(ref layer_framebuffer_info) => layer_framebuffer_info.must_preserve_contents,
            None => {
                self.framebuffer_flags
                    .contains(FramebufferFlags::MUST_PRESERVE_DEST_FRAMEBUFFER_CONTENTS)
            }
        };

        if must_preserve_contents {
            None
        } else if self.layer_framebuffer_stack.is_empty() {
            self.options.background_color
        } else {
            Some(ColorF::default())
        }
    }

    fn preserve_draw_framebuffer(&mut self) {
        match self.layer_framebuffer_stack.last_mut() {
            Some(ref mut layer_framebuffer_info) => {
                layer_framebuffer_info.must_preserve_contents = true;
            }
            None => {
                self.framebuffer_flags
                    .insert(FramebufferFlags::MUST_PRESERVE_DEST_FRAMEBUFFER_CONTENTS);
            }
        }
    }

    pub fn draw_viewport(&self) -> RectI {
        match self.layer_framebuffer_stack.last() {
            Some(ref layer_framebuffer_info) => {
                let texture = self.device.framebuffer_texture(&layer_framebuffer_info.framebuffer);
                RectI::new(Vector2I::default(), self.device.texture_size(texture))
            }
            None => self.main_viewport(),
        }
    }

    fn main_viewport(&self) -> RectI {
        match self.dest_framebuffer {
            DestFramebuffer::Default { viewport, .. } => viewport,
            DestFramebuffer::Other(ref framebuffer) => {
                let size = self
                    .device
                    .texture_size(self.device.framebuffer_texture(framebuffer));
                RectI::new(Vector2I::default(), size)
            }
        }
    }

    fn mask_viewport(&self) -> RectI {
        RectI::new(Vector2I::default(),
                   Vector2I::new(MASK_FRAMEBUFFER_WIDTH, MASK_FRAMEBUFFER_HEIGHT))
    }

    fn allocate_timer_query(&mut self) -> D::TimerQuery {
        match self.free_timer_queries.pop() {
            Some(query) => query,
            None => self.device.create_timer_query(),
        }
    }

    fn begin_composite_timer_query(&mut self) {
        let timer_query = self.allocate_timer_query();
        self.device.begin_timer_query(&timer_query);
        self.current_timers.stage_1 = Some(timer_query);
    }

    fn end_composite_timer_query(&mut self) {
        if let Some(ref query) = self.current_timers.stage_1 {
            self.device.end_timer_query(query);
        }
    }
}

// Render stats

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderStats {
    pub path_count: usize,
    pub fill_count: usize,
    pub alpha_tile_count: usize,
    pub solid_tile_count: usize,
}

impl Add<RenderStats> for RenderStats {
    type Output = RenderStats;
    fn add(self, other: RenderStats) -> RenderStats {
        RenderStats {
            path_count: self.path_count + other.path_count,
            solid_tile_count: self.solid_tile_count + other.solid_tile_count,
            alpha_tile_count: self.alpha_tile_count + other.alpha_tile_count,
            fill_count: self.fill_count + other.fill_count,
        }
    }
}

impl Div<usize> for RenderStats {
    type Output = RenderStats;
    fn div(self, divisor: usize) -> RenderStats {
        RenderStats {
            path_count: self.path_count / divisor,
            solid_tile_count: self.solid_tile_count / divisor,
            alpha_tile_count: self.alpha_tile_count / divisor,
            fill_count: self.fill_count / divisor,
        }
    }
}

struct RenderTimers<D> where D: Device {
    stage_0: Vec<D::TimerQuery>,
    stage_1: Option<D::TimerQuery>,
}

impl<D> RenderTimers<D> where D: Device {
    fn new() -> RenderTimers<D> {
        RenderTimers { stage_0: vec![], stage_1: None }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RenderTime {
    pub stage_0: Duration,
    pub stage_1: Duration,
}

impl Default for RenderTime {
    #[inline]
    fn default() -> RenderTime {
        RenderTime { stage_0: Duration::new(0, 0), stage_1: Duration::new(0, 0) }
    }
}

impl Add<RenderTime> for RenderTime {
    type Output = RenderTime;

    #[inline]
    fn add(self, other: RenderTime) -> RenderTime {
        RenderTime {
            stage_0: self.stage_0 + other.stage_0,
            stage_1: self.stage_1 + other.stage_1,
        }
    }
}

bitflags! {
    struct FramebufferFlags: u8 {
        const MUST_PRESERVE_FILL_FRAMEBUFFER_CONTENTS = 0x01;
        const MUST_PRESERVE_MASK_FRAMEBUFFER_CONTENTS = 0x02;
        const MUST_PRESERVE_DEST_FRAMEBUFFER_CONTENTS = 0x04;
    }
}

struct TextureCache<D> where D: Device {
    textures: Vec<D::Texture>,
}

impl<D> TextureCache<D> where D: Device {
    fn new() -> TextureCache<D> {
        TextureCache { textures: vec![] }
    }

    fn create_texture(&mut self, device: &mut D, format: TextureFormat, size: Vector2I)
                      -> D::Texture {
        for index in 0..self.textures.len() {
            if device.texture_size(&self.textures[index]) != size ||
                    device.texture_format(&self.textures[index]) != format {
                continue;
            }
            return self.textures.remove(index);
        }

        device.create_texture(format, size)
    }

    fn release_texture(&mut self, texture: D::Texture) {
        if self.textures.len() == TEXTURE_CACHE_SIZE {
            self.textures.pop();
        }
        self.textures.insert(0, texture);
    }
}

struct LayerFramebufferInfo<D> where D: Device {
    framebuffer: D::Framebuffer,
    effects: Effects,
    must_preserve_contents: bool,
}

trait BlendModeExt {
    fn to_blend_state(self) -> BlendState;
}

impl BlendModeExt for BlendMode {
    fn to_blend_state(self) -> BlendState {
        match self {
            BlendMode::Clear => {
                BlendState {
                    src_rgb_factor: BlendFactor::Zero,
                    dest_rgb_factor: BlendFactor::OneMinusSrcAlpha,
                    src_alpha_factor: BlendFactor::Zero,
                    dest_alpha_factor: BlendFactor::OneMinusSrcAlpha,
                    ..BlendState::default()
                }
            }
            BlendMode::SrcOver => {
                BlendState {
                    src_rgb_factor: BlendFactor::One,
                    dest_rgb_factor: BlendFactor::OneMinusSrcAlpha,
                    src_alpha_factor: BlendFactor::One,
                    dest_alpha_factor: BlendFactor::OneMinusSrcAlpha,
                    ..BlendState::default()
                }
            }
            BlendMode::DestOver => {
                BlendState {
                    src_rgb_factor: BlendFactor::OneMinusDestAlpha,
                    dest_rgb_factor: BlendFactor::DestAlpha,
                    src_alpha_factor: BlendFactor::OneMinusDestAlpha,
                    dest_alpha_factor: BlendFactor::One,
                    ..BlendState::default()
                }
            }
            BlendMode::DestOut => {
                BlendState {
                    src_rgb_factor: BlendFactor::Zero,
                    dest_rgb_factor: BlendFactor::OneMinusSrcAlpha,
                    src_alpha_factor: BlendFactor::Zero,
                    dest_alpha_factor: BlendFactor::OneMinusSrcAlpha,
                    ..BlendState::default()
                }
            }
            BlendMode::SrcAtop => {
                BlendState {
                    src_rgb_factor: BlendFactor::DestAlpha,
                    dest_rgb_factor: BlendFactor::OneMinusSrcAlpha,
                    src_alpha_factor: BlendFactor::DestAlpha,
                    dest_alpha_factor: BlendFactor::OneMinusSrcAlpha,
                    ..BlendState::default()
                }
            }
            BlendMode::Xor => {
                BlendState {
                    src_rgb_factor: BlendFactor::OneMinusDestAlpha,
                    dest_rgb_factor: BlendFactor::OneMinusSrcAlpha,
                    src_alpha_factor: BlendFactor::OneMinusDestAlpha,
                    dest_alpha_factor: BlendFactor::OneMinusSrcAlpha,
                    ..BlendState::default()
                }
            }
            BlendMode::Lighter => {
                BlendState {
                    src_rgb_factor: BlendFactor::One,
                    dest_rgb_factor: BlendFactor::One,
                    src_alpha_factor: BlendFactor::One,
                    dest_alpha_factor: BlendFactor::One,
                    ..BlendState::default()
                }
            }
            BlendMode::Lighten => {
                BlendState {
                    src_rgb_factor: BlendFactor::One,
                    dest_rgb_factor: BlendFactor::OneMinusSrcAlpha,
                    src_alpha_factor: BlendFactor::One,
                    dest_alpha_factor: BlendFactor::One,
                    op: BlendOp::Max,
                }
            }
            BlendMode::Darken => {
                BlendState {
                    src_rgb_factor: BlendFactor::One,
                    dest_rgb_factor: BlendFactor::OneMinusSrcAlpha,
                    src_alpha_factor: BlendFactor::One,
                    dest_alpha_factor: BlendFactor::One,
                    op: BlendOp::Min,
                }
            }
        }
    }
}
