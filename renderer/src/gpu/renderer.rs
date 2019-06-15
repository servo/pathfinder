// pathfinder/renderer/src/gpu/renderer.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::gpu::debug::DebugUIPresenter;
use crate::gpu_data::{AlphaTileBatchPrimitive, FillBatchPrimitive, PaintData};
use crate::gpu_data::{RenderCommand, SolidTileBatchPrimitive};
use crate::post::DefringingKernel;
use crate::tiles::{TILE_HEIGHT, TILE_WIDTH};
use pathfinder_geometry::basic::vector::{Vector2I, Vector4F};
use pathfinder_geometry::basic::rect::RectI;
use pathfinder_geometry::basic::transform3d::Transform3DF;
use pathfinder_geometry::color::ColorF;
use pathfinder_gpu::resources::ResourceLoader;
use pathfinder_gpu::{BlendState, BufferData, BufferTarget, BufferUploadMode, ClearParams};
use pathfinder_gpu::{DepthFunc, DepthState, Device, Primitive, RenderState, StencilFunc};
use pathfinder_gpu::{StencilState, TextureFormat, UniformData, VertexAttrClass};
use pathfinder_gpu::{VertexAttrDescriptor, VertexAttrType};
use pathfinder_simd::default::{F32x4, I32x4};
use std::cmp;
use std::collections::VecDeque;
use std::mem;
use std::ops::{Add, Div};
use std::time::Duration;
use std::u32;

static QUAD_VERTEX_POSITIONS: [u8; 8] = [0, 0, 1, 0, 1, 1, 0, 1];
static QUAD_VERTEX_INDICES: [u32; 6] = [0, 1, 3, 1, 2, 3];

// FIXME(pcwalton): Shrink this again!
const MASK_FRAMEBUFFER_WIDTH: i32 = TILE_WIDTH as i32 * 256;
const MASK_FRAMEBUFFER_HEIGHT: i32 = TILE_HEIGHT as i32 * 256;

// TODO(pcwalton): Replace with `mem::size_of` calls?
const FILL_INSTANCE_SIZE: usize = 8;
const SOLID_TILE_INSTANCE_SIZE: usize = 10;
const MASK_TILE_INSTANCE_SIZE: usize = 12;

const MAX_FILLS_PER_BATCH: usize = 0x4000;

pub struct Renderer<D>
where
    D: Device,
{
    // Device
    pub device: D,

    // Core data
    dest_framebuffer: DestFramebuffer<D>,
    fill_program: FillProgram<D>,
    solid_multicolor_tile_program: SolidTileMulticolorProgram<D>,
    alpha_multicolor_tile_program: AlphaTileMulticolorProgram<D>,
    solid_monochrome_tile_program: SolidTileMonochromeProgram<D>,
    alpha_monochrome_tile_program: AlphaTileMonochromeProgram<D>,
    solid_multicolor_tile_vertex_array: SolidTileVertexArray<D>,
    alpha_multicolor_tile_vertex_array: AlphaTileVertexArray<D>,
    solid_monochrome_tile_vertex_array: SolidTileVertexArray<D>,
    alpha_monochrome_tile_vertex_array: AlphaTileVertexArray<D>,
    area_lut_texture: D::Texture,
    quad_vertex_positions_buffer: D::Buffer,
    quad_vertex_indices_buffer: D::Buffer,
    fill_vertex_array: FillVertexArray<D>,
    mask_framebuffer: D::Framebuffer,
    paint_texture: Option<D::Texture>,

    // Postprocessing shader
    postprocess_source_framebuffer: Option<D::Framebuffer>,
    postprocess_program: PostprocessProgram<D>,
    postprocess_vertex_array: PostprocessVertexArray<D>,
    gamma_lut_texture: D::Texture,

    // Stencil shader
    stencil_program: StencilProgram<D>,
    stencil_vertex_array: StencilVertexArray<D>,

    // Reprojection shader
    reprojection_program: ReprojectionProgram<D>,
    reprojection_vertex_array: ReprojectionVertexArray<D>,

    // Rendering state
    mask_framebuffer_cleared: bool,
    buffered_fills: Vec<FillBatchPrimitive>,

    // Debug
    pub stats: RenderStats,
    current_timers: RenderTimers<D>,
    pending_timers: VecDeque<RenderTimers<D>>,
    free_timer_queries: Vec<D::TimerQuery>,
    pub debug_ui_presenter: DebugUIPresenter<D>,

    // Extra info
    render_mode: RenderMode,
    use_depth: bool,
}

impl<D> Renderer<D>
where
    D: Device,
{
    pub fn new(
        device: D,
        resources: &dyn ResourceLoader,
        dest_framebuffer: DestFramebuffer<D>,
    ) -> Renderer<D> {
        let fill_program = FillProgram::new(&device, resources);

        let solid_multicolor_tile_program = SolidTileMulticolorProgram::new(&device, resources);
        let alpha_multicolor_tile_program = AlphaTileMulticolorProgram::new(&device, resources);
        let solid_monochrome_tile_program = SolidTileMonochromeProgram::new(&device, resources);
        let alpha_monochrome_tile_program = AlphaTileMonochromeProgram::new(&device, resources);

        let postprocess_program = PostprocessProgram::new(&device, resources);
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

        let fill_vertex_array = FillVertexArray::new(
            &device,
            &fill_program,
            &quad_vertex_positions_buffer,
            &quad_vertex_indices_buffer,
        );
        let alpha_multicolor_tile_vertex_array = AlphaTileVertexArray::new(
            &device,
            &alpha_multicolor_tile_program.alpha_tile_program,
            &quad_vertex_positions_buffer,
            &quad_vertex_indices_buffer,
        );
        let solid_multicolor_tile_vertex_array = SolidTileVertexArray::new(
            &device,
            &solid_multicolor_tile_program.solid_tile_program,
            &quad_vertex_positions_buffer,
            &quad_vertex_indices_buffer,
        );
        let alpha_monochrome_tile_vertex_array = AlphaTileVertexArray::new(
            &device,
            &alpha_monochrome_tile_program.alpha_tile_program,
            &quad_vertex_positions_buffer,
            &quad_vertex_indices_buffer,
        );
        let solid_monochrome_tile_vertex_array = SolidTileVertexArray::new(
            &device,
            &solid_monochrome_tile_program.solid_tile_program,
            &quad_vertex_positions_buffer,
            &quad_vertex_indices_buffer,
        );
        let postprocess_vertex_array = PostprocessVertexArray::new(
            &device,
            &postprocess_program,
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

        let mask_framebuffer_size =
            Vector2I::new(MASK_FRAMEBUFFER_WIDTH, MASK_FRAMEBUFFER_HEIGHT);
        let mask_framebuffer_texture =
            device.create_texture(TextureFormat::R16F, mask_framebuffer_size);
        let mask_framebuffer = device.create_framebuffer(mask_framebuffer_texture);

        let window_size = dest_framebuffer.window_size(&device);
        let debug_ui_presenter = DebugUIPresenter::new(&device, resources, window_size);

        let renderer = Renderer {
            device,

            dest_framebuffer,
            fill_program,
            solid_monochrome_tile_program,
            alpha_monochrome_tile_program,
            solid_multicolor_tile_program,
            alpha_multicolor_tile_program,
            solid_monochrome_tile_vertex_array,
            alpha_monochrome_tile_vertex_array,
            solid_multicolor_tile_vertex_array,
            alpha_multicolor_tile_vertex_array,
            area_lut_texture,
            quad_vertex_positions_buffer,
            quad_vertex_indices_buffer,
            fill_vertex_array,
            mask_framebuffer,
            paint_texture: None,

            postprocess_source_framebuffer: None,
            postprocess_program,
            postprocess_vertex_array,
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

            mask_framebuffer_cleared: false,
            buffered_fills: vec![],

            render_mode: RenderMode::default(),
            use_depth: false,
        };

        // As a convenience, bind the destination framebuffer.
        renderer.bind_dest_framebuffer();

        renderer
    }

    pub fn begin_scene(&mut self) {
        self.init_postprocessing_framebuffer();

        self.mask_framebuffer_cleared = false;
        self.stats = RenderStats::default();
    }

    pub fn render_command(&mut self, command: &RenderCommand) {
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
                self.begin_composite_timer_query();
                self.draw_buffered_fills();
            }
            RenderCommand::SolidTile(ref solid_tiles) => {
                let count = solid_tiles.len();
                self.stats.solid_tile_count += count;
                self.upload_solid_tiles(solid_tiles);
                self.draw_solid_tiles(count as u32);
            }
            RenderCommand::AlphaTile(ref alpha_tiles) => {
                let count = alpha_tiles.len();
                self.stats.alpha_tile_count += count;
                self.upload_alpha_tiles(alpha_tiles);
                self.draw_alpha_tiles(count as u32);
            }
            RenderCommand::Finish { .. } => {}
        }
    }

    pub fn end_scene(&mut self) {
        if self.postprocessing_needed() {
            self.postprocess();
        }

        self.end_composite_timer_query();
        self.pending_timers.push_back(mem::replace(&mut self.current_timers, RenderTimers::new()));
    }

    pub fn draw_debug_ui(&self) {
        self.bind_dest_framebuffer();
        self.debug_ui_presenter.draw(&self.device);
    }

    pub fn shift_rendering_time(&mut self) -> Option<RenderTime> {
        let timers = self.pending_timers.front()?;

        // Accumulate stage-0 time.
        let mut total_stage_0_time = Duration::new(0, 0);
        for timer_query in &timers.stage_0 {
            if !self.device.timer_query_is_available(timer_query) {
                return None;
            }
            total_stage_0_time += self.device.get_timer_query(timer_query);
        }

        // Get stage-1 time.
        let stage_1_time = {
            let stage_1_timer_query = timers.stage_1.as_ref().unwrap();
            if !self.device.timer_query_is_available(&stage_1_timer_query) {
                return None;
            }
            self.device.get_timer_query(stage_1_timer_query)
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
    pub fn set_main_framebuffer_size(&mut self, new_framebuffer_size: Vector2I) {
        self.debug_ui_presenter.ui_presenter.set_framebuffer_size(new_framebuffer_size);
    }

    #[inline]
    pub fn set_render_mode(&mut self, mode: RenderMode) {
        self.render_mode = mode;
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
        match self.paint_texture {
            Some(ref paint_texture) if
                self.device.texture_size(paint_texture) == paint_data.size => {}
            _ => {
                let texture = self.device.create_texture(TextureFormat::RGBA8, paint_data.size);
                self.paint_texture = Some(texture)
            }
        }

        self.device.upload_to_texture(self.paint_texture.as_ref().unwrap(),
                                      paint_data.size,
                                      &paint_data.texels);
    }

    fn upload_solid_tiles(&mut self, solid_tiles: &[SolidTileBatchPrimitive]) {
        self.device.allocate_buffer(
            &self.solid_tile_vertex_array().vertex_buffer,
            BufferData::Memory(&solid_tiles),
            BufferTarget::Vertex,
            BufferUploadMode::Dynamic,
        );
    }

    fn upload_alpha_tiles(&mut self, alpha_tiles: &[AlphaTileBatchPrimitive]) {
        self.device.allocate_buffer(
            &self.alpha_tile_vertex_array().vertex_buffer,
            BufferData::Memory(&alpha_tiles),
            BufferTarget::Vertex,
            BufferUploadMode::Dynamic,
        );
    }

    fn clear_mask_framebuffer(&mut self) {
        self.device.bind_framebuffer(&self.mask_framebuffer);

        // TODO(pcwalton): Only clear the appropriate portion?
        self.device.clear(&ClearParams {
            color: Some(ColorF::transparent_black()),
            ..ClearParams::default()
        });
    }

    fn add_fills(&mut self, mut fills: &[FillBatchPrimitive]) {
        if fills.is_empty() {
            return;
        }

        let timer_query = self.allocate_timer_query();
        self.device.begin_timer_query(&timer_query);

        self.stats.fill_count += fills.len();

        while !fills.is_empty() {
            let count = cmp::min(fills.len(), MAX_FILLS_PER_BATCH - self.buffered_fills.len());
            self.buffered_fills.extend_from_slice(&fills[0..count]);
            fills = &fills[count..];
            if self.buffered_fills.len() == MAX_FILLS_PER_BATCH {
                self.draw_buffered_fills();
            }
        }

        self.device.end_timer_query(&timer_query);
        self.current_timers.stage_0.push(timer_query);
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

        if !self.mask_framebuffer_cleared {
            self.clear_mask_framebuffer();
            self.mask_framebuffer_cleared = true;
        }

        self.device.bind_framebuffer(&self.mask_framebuffer);

        self.device
            .bind_vertex_array(&self.fill_vertex_array.vertex_array);
        self.device.use_program(&self.fill_program.program);
        self.device.set_uniform(
            &self.fill_program.framebuffer_size_uniform,
            UniformData::Vec2(
                I32x4::new(MASK_FRAMEBUFFER_WIDTH, MASK_FRAMEBUFFER_HEIGHT, 0, 0).to_f32x4(),
            ),
        );
        self.device.set_uniform(
            &self.fill_program.tile_size_uniform,
            UniformData::Vec2(I32x4::new(TILE_WIDTH as i32, TILE_HEIGHT as i32, 0, 0).to_f32x4()),
        );
        self.device.bind_texture(&self.area_lut_texture, 0);
        self.device.set_uniform(
            &self.fill_program.area_lut_uniform,
            UniformData::TextureUnit(0),
        );
        let render_state = RenderState {
            blend: BlendState::RGBOneAlphaOne,
            ..RenderState::default()
        };
        debug_assert!(self.buffered_fills.len() <= u32::MAX as usize);
        self.device.draw_elements_instanced(
            Primitive::Triangles,
            6,
            self.buffered_fills.len() as u32,
            &render_state,
        );

        self.buffered_fills.clear()
    }

    fn draw_alpha_tiles(&mut self, count: u32) {
        self.bind_draw_framebuffer();

        let alpha_tile_vertex_array = self.alpha_tile_vertex_array();
        let alpha_tile_program = self.alpha_tile_program();

        self.device
            .bind_vertex_array(&alpha_tile_vertex_array.vertex_array);
        self.device.use_program(&alpha_tile_program.program);
        self.device.set_uniform(
            &alpha_tile_program.framebuffer_size_uniform,
            UniformData::Vec2(self.draw_viewport().size().to_f32().0),
        );
        self.device.set_uniform(
            &alpha_tile_program.tile_size_uniform,
            UniformData::Vec2(I32x4::new(TILE_WIDTH as i32, TILE_HEIGHT as i32, 0, 0).to_f32x4()),
        );
        self.device
            .bind_texture(self.device.framebuffer_texture(&self.mask_framebuffer), 0);
        self.device.set_uniform(
            &alpha_tile_program.stencil_texture_uniform,
            UniformData::TextureUnit(0),
        );
        self.device.set_uniform(
            &alpha_tile_program.stencil_texture_size_uniform,
            UniformData::Vec2(
                I32x4::new(MASK_FRAMEBUFFER_WIDTH, MASK_FRAMEBUFFER_HEIGHT, 0, 0).to_f32x4(),
            ),
        );

        match self.render_mode {
            RenderMode::Multicolor => {
                let paint_texture = self.paint_texture.as_ref().unwrap();
                self.device.bind_texture(paint_texture, 1);
                self.device.set_uniform(
                    &self.alpha_multicolor_tile_program.paint_texture_uniform,
                    UniformData::TextureUnit(1),
                );
                self.device.set_uniform(
                    &self.alpha_multicolor_tile_program.paint_texture_size_uniform,
                    UniformData::Vec2(self.device.texture_size(paint_texture).0.to_f32x4())
                );
            }
            RenderMode::Monochrome { .. } if self.postprocessing_needed() => {
                self.device.set_uniform(
                    &self.alpha_monochrome_tile_program.color_uniform,
                    UniformData::Vec4(F32x4::splat(1.0)),
                );
            }
            RenderMode::Monochrome { fg_color, .. } => {
                self.device.set_uniform(
                    &self.alpha_monochrome_tile_program.color_uniform,
                    UniformData::Vec4(fg_color.0),
                );
            }
        }

        // FIXME(pcwalton): Fill this in properly!
        self.device.set_uniform(
            &alpha_tile_program.view_box_origin_uniform,
            UniformData::Vec2(F32x4::default()),
        );
        let render_state = RenderState {
            blend: BlendState::RGBSrcAlphaAlphaOneMinusSrcAlpha,
            stencil: self.stencil_state(),
            ..RenderState::default()
        };
        self.device.bind_buffer(self.quad_vertex_indices_buffer(), BufferTarget::Index);
        self.device.draw_elements_instanced(Primitive::Triangles, 6, count, &render_state);
    }

    fn draw_solid_tiles(&mut self, count: u32) {
        self.bind_draw_framebuffer();

        let solid_tile_vertex_array = self.solid_tile_vertex_array();
        let solid_tile_program = self.solid_tile_program();

        self.device
            .bind_vertex_array(&solid_tile_vertex_array.vertex_array);
        self.device.use_program(&solid_tile_program.program);
        self.device.set_uniform(
            &solid_tile_program.framebuffer_size_uniform,
            UniformData::Vec2(self.draw_viewport().size().0.to_f32x4()),
        );
        self.device.set_uniform(
            &solid_tile_program.tile_size_uniform,
            UniformData::Vec2(I32x4::new(TILE_WIDTH as i32, TILE_HEIGHT as i32, 0, 0).to_f32x4()),
        );

        match self.render_mode {
            RenderMode::Multicolor => {
                let paint_texture = self.paint_texture.as_ref().unwrap();
                self.device.bind_texture(paint_texture, 0);
                self.device.set_uniform(
                    &self
                        .solid_multicolor_tile_program
                        .paint_texture_uniform,
                    UniformData::TextureUnit(0),
                );
                self.device.set_uniform(
                    &self
                        .solid_multicolor_tile_program
                        .paint_texture_size_uniform,
                    UniformData::Vec2(self.device.texture_size(paint_texture).0.to_f32x4())
                );
            }
            RenderMode::Monochrome { .. } if self.postprocessing_needed() => {
                self.device.set_uniform(
                    &self.solid_monochrome_tile_program.color_uniform,
                    UniformData::Vec4(F32x4::splat(1.0)),
                );
            }
            RenderMode::Monochrome { fg_color, .. } => {
                self.device.set_uniform(
                    &self.solid_monochrome_tile_program.color_uniform,
                    UniformData::Vec4(fg_color.0),
                );
            }
        }

        // FIXME(pcwalton): Fill this in properly!
        self.device.set_uniform(
            &solid_tile_program.view_box_origin_uniform,
            UniformData::Vec2(F32x4::default()),
        );
        let render_state = RenderState {
            stencil: self.stencil_state(),
            ..RenderState::default()
        };
        self.device.draw_elements_instanced(Primitive::Triangles, 6, count, &render_state);
    }

    fn postprocess(&mut self) {
        let (fg_color, bg_color, defringing_kernel, gamma_correction_enabled);
        match self.render_mode {
            RenderMode::Multicolor => return,
            RenderMode::Monochrome {
                fg_color: fg,
                bg_color: bg,
                defringing_kernel: kernel,
                gamma_correction,
            } => {
                fg_color = fg;
                bg_color = bg;
                defringing_kernel = kernel;
                gamma_correction_enabled = gamma_correction;
            }
        }

        self.bind_dest_framebuffer();

        self.device
            .bind_vertex_array(&self.postprocess_vertex_array.vertex_array);
        self.device.use_program(&self.postprocess_program.program);
        self.device.set_uniform(
            &self.postprocess_program.framebuffer_size_uniform,
            UniformData::Vec2(self.main_viewport().size().to_f32().0),
        );
        match defringing_kernel {
            Some(ref kernel) => {
                self.device.set_uniform(
                    &self.postprocess_program.kernel_uniform,
                    UniformData::Vec4(F32x4::from_slice(&kernel.0)),
                );
            }
            None => {
                self.device.set_uniform(
                    &self.postprocess_program.kernel_uniform,
                    UniformData::Vec4(F32x4::default()),
                );
            }
        }

        let postprocess_source_framebuffer = self.postprocess_source_framebuffer.as_ref().unwrap();
        let source_texture = self
            .device
            .framebuffer_texture(postprocess_source_framebuffer);
        let source_texture_size = self.device.texture_size(source_texture);
        self.device.bind_texture(&source_texture, 0);
        self.device.set_uniform(
            &self.postprocess_program.source_uniform,
            UniformData::TextureUnit(0),
        );
        self.device.set_uniform(
            &self.postprocess_program.source_size_uniform,
            UniformData::Vec2(source_texture_size.0.to_f32x4()),
        );
        self.device.bind_texture(&self.gamma_lut_texture, 1);
        self.device.set_uniform(
            &self.postprocess_program.gamma_lut_uniform,
            UniformData::TextureUnit(1),
        );
        self.device.set_uniform(
            &self.postprocess_program.fg_color_uniform,
            UniformData::Vec4(fg_color.0),
        );
        self.device.set_uniform(
            &self.postprocess_program.bg_color_uniform,
            UniformData::Vec4(bg_color.0),
        );
        self.device.set_uniform(
            &self.postprocess_program.gamma_correction_enabled_uniform,
            UniformData::Int(gamma_correction_enabled as i32),
        );
        self.device.draw_arrays(Primitive::Triangles, 4, &RenderState::default());
    }

    fn solid_tile_program(&self) -> &SolidTileProgram<D> {
        match self.render_mode {
            RenderMode::Monochrome { .. } => &self.solid_monochrome_tile_program.solid_tile_program,
            RenderMode::Multicolor => &self.solid_multicolor_tile_program.solid_tile_program,
        }
    }

    fn alpha_tile_program(&self) -> &AlphaTileProgram<D> {
        match self.render_mode {
            RenderMode::Monochrome { .. } => &self.alpha_monochrome_tile_program.alpha_tile_program,
            RenderMode::Multicolor => &self.alpha_multicolor_tile_program.alpha_tile_program,
        }
    }

    fn solid_tile_vertex_array(&self) -> &SolidTileVertexArray<D> {
        match self.render_mode {
            RenderMode::Monochrome { .. } => &self.solid_monochrome_tile_vertex_array,
            RenderMode::Multicolor => &self.solid_multicolor_tile_vertex_array,
        }
    }

    fn alpha_tile_vertex_array(&self) -> &AlphaTileVertexArray<D> {
        match self.render_mode {
            RenderMode::Monochrome { .. } => &self.alpha_monochrome_tile_vertex_array,
            RenderMode::Multicolor => &self.alpha_multicolor_tile_vertex_array,
        }
    }

    fn draw_stencil(&self, quad_positions: &[Vector4F]) {
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

        self.bind_draw_framebuffer();

        self.device.bind_vertex_array(&self.stencil_vertex_array.vertex_array);
        self.device.use_program(&self.stencil_program.program);
        self.device.draw_elements(
            Primitive::Triangles,
            indices.len() as u32,
            &RenderState {
                // FIXME(pcwalton): Should we really write to the depth buffer?
                depth: Some(DepthState {
                    func: DepthFunc::Less,
                    write: true,
                }),
                stencil: Some(StencilState {
                    func: StencilFunc::Always,
                    reference: 1,
                    mask: 1,
                    write: true,
                }),
                color_mask: false,
                ..RenderState::default()
            },
        )
    }

    pub fn reproject_texture(
        &self,
        texture: &D::Texture,
        old_transform: &Transform3DF,
        new_transform: &Transform3DF,
    ) {
        self.bind_draw_framebuffer();

        self.device.bind_vertex_array(&self.reprojection_vertex_array.vertex_array);
        self.device.use_program(&self.reprojection_program.program);
        self.device.set_uniform(
            &self.reprojection_program.old_transform_uniform,
            UniformData::from_transform_3d(old_transform),
        );
        self.device.set_uniform(
            &self.reprojection_program.new_transform_uniform,
            UniformData::from_transform_3d(new_transform),
        );
        self.device.bind_texture(texture, 0);
        self.device.set_uniform(
            &self.reprojection_program.texture_uniform,
            UniformData::TextureUnit(0),
        );
        self.device.draw_elements(
            Primitive::Triangles,
            6,
            &RenderState {
                blend: BlendState::RGBSrcAlphaAlphaOneMinusSrcAlpha,
                depth: Some(DepthState {
                    func: DepthFunc::Less,
                    write: false,
                }),
                ..RenderState::default()
            },
        );
    }

    pub fn bind_draw_framebuffer(&self) {
        if self.postprocessing_needed() {
            self.device
                .bind_framebuffer(self.postprocess_source_framebuffer.as_ref().unwrap());
        } else {
            self.bind_dest_framebuffer();
        }
    }

    pub fn bind_dest_framebuffer(&self) {
        match self.dest_framebuffer {
            DestFramebuffer::Default { viewport, .. } => {
                self.device.bind_default_framebuffer(viewport)
            }
            DestFramebuffer::Other(ref framebuffer) => self.device.bind_framebuffer(framebuffer),
        }
    }

    fn init_postprocessing_framebuffer(&mut self) {
        if !self.postprocessing_needed() {
            self.postprocess_source_framebuffer = None;
            return;
        }

        let source_framebuffer_size = self.draw_viewport().size();
        match self.postprocess_source_framebuffer {
            Some(ref framebuffer)
                if self
                    .device
                    .texture_size(self.device.framebuffer_texture(framebuffer))
                    == source_framebuffer_size => {}
            _ => {
                let texture = self
                    .device
                    .create_texture(TextureFormat::R8, source_framebuffer_size);
                self.postprocess_source_framebuffer = Some(self.device.create_framebuffer(texture))
            }
        };

        self.device
            .bind_framebuffer(self.postprocess_source_framebuffer.as_ref().unwrap());
        self.device.clear(&ClearParams {
            color: Some(ColorF::transparent_black()),
            ..ClearParams::default()
        });
    }

    fn postprocessing_needed(&self) -> bool {
        match self.render_mode {
            RenderMode::Monochrome {
                ref defringing_kernel,
                gamma_correction,
                ..
            } => defringing_kernel.is_some() || gamma_correction,
            _ => false,
        }
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

    fn draw_viewport(&self) -> RectI {
        let main_viewport = self.main_viewport();
        match self.render_mode {
            RenderMode::Monochrome {
                defringing_kernel: Some(..),
                ..
            } => {
                let scale = Vector2I::new(3, 1);
                RectI::new(Vector2I::default(), main_viewport.size().scale_xy(scale))
            }
            _ => main_viewport,
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
        let query = self.current_timers.stage_1.as_ref().expect("No stage 1 timer query yet?!");
        self.device.end_timer_query(&query);
    }
}

struct FillVertexArray<D>
where
    D: Device,
{
    vertex_array: D::VertexArray,
    vertex_buffer: D::Buffer,
}

impl<D> FillVertexArray<D>
where
    D: Device,
{
    fn new(
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

        device.bind_vertex_array(&vertex_array);
        device.use_program(&fill_program.program);
        device.bind_buffer(quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&tess_coord_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Float,
            attr_type: VertexAttrType::U8,
            stride: 0,
            offset: 0,
            divisor: 0,
        });
        device.bind_buffer(&vertex_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&from_px_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U8,
            stride: FILL_INSTANCE_SIZE,
            offset: 0,
            divisor: 1,
        });
        device.configure_vertex_attr(&to_px_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U8,
            stride: FILL_INSTANCE_SIZE,
            offset: 1,
            divisor: 1,
        });
        device.configure_vertex_attr(&from_subpx_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::FloatNorm,
            attr_type: VertexAttrType::U8,
            stride: FILL_INSTANCE_SIZE,
            offset: 2,
            divisor: 1,
        });
        device.configure_vertex_attr(&to_subpx_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::FloatNorm,
            attr_type: VertexAttrType::U8,
            stride: FILL_INSTANCE_SIZE,
            offset: 4,
            divisor: 1,
        });
        device.configure_vertex_attr(&tile_index_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U16,
            stride: FILL_INSTANCE_SIZE,
            offset: 6,
            divisor: 1,
        });
        device.bind_buffer(quad_vertex_indices_buffer, BufferTarget::Index);

        FillVertexArray { vertex_array, vertex_buffer }
    }
}

struct AlphaTileVertexArray<D>
where
    D: Device,
{
    vertex_array: D::VertexArray,
    vertex_buffer: D::Buffer,
}

impl<D> AlphaTileVertexArray<D>
where
    D: Device,
{
    fn new(
        device: &D,
        alpha_tile_program: &AlphaTileProgram<D>,
        quad_vertex_positions_buffer: &D::Buffer,
        quad_vertex_indices_buffer: &D::Buffer,
    ) -> AlphaTileVertexArray<D> {
        let (vertex_array, vertex_buffer) = (device.create_vertex_array(), device.create_buffer());

        let tess_coord_attr = device.get_vertex_attr(&alpha_tile_program.program, "TessCoord")
                                    .unwrap();
        let tile_origin_attr = device.get_vertex_attr(&alpha_tile_program.program, "TileOrigin")
                                     .unwrap();
        let backdrop_attr = device.get_vertex_attr(&alpha_tile_program.program, "Backdrop")
                                  .unwrap();
        let tile_index_attr = device.get_vertex_attr(&alpha_tile_program.program, "TileIndex")
                                    .unwrap();
        let color_tex_coord_attr = device.get_vertex_attr(&alpha_tile_program.program,
                                                          "ColorTexCoord");

        // NB: The object must be of type `I16`, not `U16`, to work around a macOS Radeon
        // driver bug.
        device.bind_vertex_array(&vertex_array);
        device.use_program(&alpha_tile_program.program);
        device.bind_buffer(quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&tess_coord_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Float,
            attr_type: VertexAttrType::U8,
            stride: 0,
            offset: 0,
            divisor: 0,
        });
        device.bind_buffer(&vertex_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&tile_origin_attr, &VertexAttrDescriptor {
            size: 3,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::U8,
            stride: MASK_TILE_INSTANCE_SIZE,
            offset: 0,
            divisor: 1,
        });
        device.configure_vertex_attr(&backdrop_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I8,
            stride: MASK_TILE_INSTANCE_SIZE,
            offset: 3,
            divisor: 1,
        });
        device.configure_vertex_attr(&tile_index_attr, &VertexAttrDescriptor {
            size: 1,
            class: VertexAttrClass::Int,
            attr_type: VertexAttrType::I16,
            stride: MASK_TILE_INSTANCE_SIZE,
            offset: 6,
            divisor: 1,
        });
        if let Some(color_tex_coord_attr) = color_tex_coord_attr {
            device.configure_vertex_attr(&color_tex_coord_attr, &VertexAttrDescriptor {
                size: 2,
                class: VertexAttrClass::FloatNorm,
                attr_type: VertexAttrType::U16,
                stride: MASK_TILE_INSTANCE_SIZE,
                offset: 8,
                divisor: 1,
            });
        }
        device.bind_buffer(quad_vertex_indices_buffer, BufferTarget::Index);

        AlphaTileVertexArray { vertex_array, vertex_buffer }
    }
}

struct SolidTileVertexArray<D>
where
    D: Device,
{
    vertex_array: D::VertexArray,
    vertex_buffer: D::Buffer,
}

impl<D> SolidTileVertexArray<D>
where
    D: Device,
{
    fn new(
        device: &D,
        solid_tile_program: &SolidTileProgram<D>,
        quad_vertex_positions_buffer: &D::Buffer,
        quad_vertex_indices_buffer: &D::Buffer,
    ) -> SolidTileVertexArray<D> {
        let (vertex_array, vertex_buffer) = (device.create_vertex_array(), device.create_buffer());

        let tess_coord_attr = device.get_vertex_attr(&solid_tile_program.program, "TessCoord")
                                    .unwrap();
        let tile_origin_attr = device.get_vertex_attr(&solid_tile_program.program, "TileOrigin")
                                     .unwrap();
        let color_tex_coord_attr = device.get_vertex_attr(&solid_tile_program.program,
                                                          "ColorTexCoord");

        // NB: The object must be of type short, not unsigned short, to work around a macOS
        // Radeon driver bug.
        device.bind_vertex_array(&vertex_array);
        device.use_program(&solid_tile_program.program);
        device.bind_buffer(quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&tess_coord_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Float,
            attr_type: VertexAttrType::U8,
            stride: 0,
            offset: 0,
            divisor: 0,
        });
        device.bind_buffer(&vertex_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&tile_origin_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Float,
            attr_type: VertexAttrType::I16,
            stride: SOLID_TILE_INSTANCE_SIZE,
            offset: 0,
            divisor: 1,
        });
        if let Some(color_tex_coord_attr) = color_tex_coord_attr {
            device.configure_vertex_attr(&color_tex_coord_attr, &VertexAttrDescriptor {
                size: 2,
                class: VertexAttrClass::FloatNorm,
                attr_type: VertexAttrType::U16,
                stride: SOLID_TILE_INSTANCE_SIZE,
                offset: 4,
                divisor: 1,
            });
        }
        device.bind_buffer(quad_vertex_indices_buffer, BufferTarget::Index);

        SolidTileVertexArray { vertex_array, vertex_buffer }
    }
}

struct FillProgram<D>
where
    D: Device,
{
    program: D::Program,
    framebuffer_size_uniform: D::Uniform,
    tile_size_uniform: D::Uniform,
    area_lut_uniform: D::Uniform,
}

impl<D> FillProgram<D>
where
    D: Device,
{
    fn new(device: &D, resources: &dyn ResourceLoader) -> FillProgram<D> {
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

struct SolidTileProgram<D>
where
    D: Device,
{
    program: D::Program,
    framebuffer_size_uniform: D::Uniform,
    tile_size_uniform: D::Uniform,
    view_box_origin_uniform: D::Uniform,
}

impl<D> SolidTileProgram<D>
where
    D: Device,
{
    fn new(device: &D, program_name: &str, resources: &dyn ResourceLoader) -> SolidTileProgram<D> {
        let program = device.create_program_from_shader_names(
            resources,
            program_name,
            program_name,
            "tile_solid",
        );
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let tile_size_uniform = device.get_uniform(&program, "TileSize");
        let view_box_origin_uniform = device.get_uniform(&program, "ViewBoxOrigin");
        SolidTileProgram {
            program,
            framebuffer_size_uniform,
            tile_size_uniform,
            view_box_origin_uniform,
        }
    }
}

struct SolidTileMulticolorProgram<D>
where
    D: Device,
{
    solid_tile_program: SolidTileProgram<D>,
    paint_texture_uniform: D::Uniform,
    paint_texture_size_uniform: D::Uniform,
}

impl<D> SolidTileMulticolorProgram<D>
where
    D: Device,
{
    fn new(device: &D, resources: &dyn ResourceLoader) -> SolidTileMulticolorProgram<D> {
        let solid_tile_program = SolidTileProgram::new(device, "tile_solid_multicolor", resources);
        let paint_texture_uniform =
            device.get_uniform(&solid_tile_program.program, "PaintTexture");
        let paint_texture_size_uniform =
            device.get_uniform(&solid_tile_program.program, "PaintTextureSize");
        SolidTileMulticolorProgram {
            solid_tile_program,
            paint_texture_uniform,
            paint_texture_size_uniform,
        }
    }
}

struct SolidTileMonochromeProgram<D>
where
    D: Device,
{
    solid_tile_program: SolidTileProgram<D>,
    color_uniform: D::Uniform,
}

impl<D> SolidTileMonochromeProgram<D>
where
    D: Device,
{
    fn new(device: &D, resources: &dyn ResourceLoader) -> SolidTileMonochromeProgram<D> {
        let solid_tile_program = SolidTileProgram::new(device, "tile_solid_monochrome", resources);
        let color_uniform = device.get_uniform(&solid_tile_program.program, "Color");
        SolidTileMonochromeProgram {
            solid_tile_program,
            color_uniform,
        }
    }
}

struct AlphaTileProgram<D>
where
    D: Device,
{
    program: D::Program,
    framebuffer_size_uniform: D::Uniform,
    tile_size_uniform: D::Uniform,
    stencil_texture_uniform: D::Uniform,
    stencil_texture_size_uniform: D::Uniform,
    view_box_origin_uniform: D::Uniform,
}

impl<D> AlphaTileProgram<D>
where
    D: Device,
{
    fn new(device: &D, program_name: &str, resources: &dyn ResourceLoader) -> AlphaTileProgram<D> {
        let program = device.create_program_from_shader_names(
            resources,
            program_name,
            program_name,
            "tile_alpha",
        );
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let tile_size_uniform = device.get_uniform(&program, "TileSize");
        let stencil_texture_uniform = device.get_uniform(&program, "StencilTexture");
        let stencil_texture_size_uniform = device.get_uniform(&program, "StencilTextureSize");
        let view_box_origin_uniform = device.get_uniform(&program, "ViewBoxOrigin");
        AlphaTileProgram {
            program,
            framebuffer_size_uniform,
            tile_size_uniform,
            stencil_texture_uniform,
            stencil_texture_size_uniform,
            view_box_origin_uniform,
        }
    }
}

struct AlphaTileMulticolorProgram<D>
where
    D: Device,
{
    alpha_tile_program: AlphaTileProgram<D>,
    paint_texture_uniform: D::Uniform,
    paint_texture_size_uniform: D::Uniform,
}

impl<D> AlphaTileMulticolorProgram<D>
where
    D: Device,
{
    fn new(device: &D, resources: &dyn ResourceLoader) -> AlphaTileMulticolorProgram<D> {
        let alpha_tile_program = AlphaTileProgram::new(device, "tile_alpha_multicolor", resources);
        let paint_texture_uniform =
            device.get_uniform(&alpha_tile_program.program, "PaintTexture");
        let paint_texture_size_uniform =
            device.get_uniform(&alpha_tile_program.program, "PaintTextureSize");
        AlphaTileMulticolorProgram {
            alpha_tile_program,
            paint_texture_uniform,
            paint_texture_size_uniform,
        }
    }
}

struct AlphaTileMonochromeProgram<D>
where
    D: Device,
{
    alpha_tile_program: AlphaTileProgram<D>,
    color_uniform: D::Uniform,
}

impl<D> AlphaTileMonochromeProgram<D>
where
    D: Device,
{
    fn new(device: &D, resources: &dyn ResourceLoader) -> AlphaTileMonochromeProgram<D> {
        let alpha_tile_program = AlphaTileProgram::new(device, "tile_alpha_monochrome", resources);
        let color_uniform = device.get_uniform(&alpha_tile_program.program, "Color");
        AlphaTileMonochromeProgram {
            alpha_tile_program,
            color_uniform,
        }
    }
}

struct PostprocessProgram<D>
where
    D: Device,
{
    program: D::Program,
    source_uniform: D::Uniform,
    source_size_uniform: D::Uniform,
    framebuffer_size_uniform: D::Uniform,
    kernel_uniform: D::Uniform,
    gamma_lut_uniform: D::Uniform,
    gamma_correction_enabled_uniform: D::Uniform,
    fg_color_uniform: D::Uniform,
    bg_color_uniform: D::Uniform,
}

impl<D> PostprocessProgram<D>
where
    D: Device,
{
    fn new(device: &D, resources: &dyn ResourceLoader) -> PostprocessProgram<D> {
        let program = device.create_program(resources, "post");
        let source_uniform = device.get_uniform(&program, "Source");
        let source_size_uniform = device.get_uniform(&program, "SourceSize");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let kernel_uniform = device.get_uniform(&program, "Kernel");
        let gamma_lut_uniform = device.get_uniform(&program, "GammaLUT");
        let gamma_correction_enabled_uniform =
            device.get_uniform(&program, "GammaCorrectionEnabled");
        let fg_color_uniform = device.get_uniform(&program, "FGColor");
        let bg_color_uniform = device.get_uniform(&program, "BGColor");
        PostprocessProgram {
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

struct PostprocessVertexArray<D>
where
    D: Device,
{
    vertex_array: D::VertexArray,
}

impl<D> PostprocessVertexArray<D>
where
    D: Device,
{
    fn new(
        device: &D,
        postprocess_program: &PostprocessProgram<D>,
        quad_vertex_positions_buffer: &D::Buffer,
        quad_vertex_indices_buffer: &D::Buffer,
    ) -> PostprocessVertexArray<D> {
        let vertex_array = device.create_vertex_array();
        let position_attr = device.get_vertex_attr(&postprocess_program.program, "Position")
                                  .unwrap();

        device.bind_vertex_array(&vertex_array);
        device.use_program(&postprocess_program.program);
        device.bind_buffer(quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&position_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Float,
            attr_type: VertexAttrType::U8,
            stride: 0,
            offset: 0,
            divisor: 0,
        });
        device.bind_buffer(quad_vertex_indices_buffer, BufferTarget::Index);

        PostprocessVertexArray { vertex_array }
    }
}

struct StencilProgram<D>
where
    D: Device,
{
    program: D::Program,
}

impl<D> StencilProgram<D>
where
    D: Device,
{
    fn new(device: &D, resources: &dyn ResourceLoader) -> StencilProgram<D> {
        let program = device.create_program(resources, "stencil");
        StencilProgram { program }
    }
}

struct StencilVertexArray<D>
where
    D: Device,
{
    vertex_array: D::VertexArray,
    vertex_buffer: D::Buffer,
    index_buffer: D::Buffer,
}

impl<D> StencilVertexArray<D>
where
    D: Device,
{
    fn new(device: &D, stencil_program: &StencilProgram<D>) -> StencilVertexArray<D> {
        let vertex_array = device.create_vertex_array();
        let (vertex_buffer, index_buffer) = (device.create_buffer(), device.create_buffer());

        let position_attr = device.get_vertex_attr(&stencil_program.program, "Position").unwrap();
        device.bind_vertex_array(&vertex_array);
        device.use_program(&stencil_program.program);
        device.bind_buffer(&vertex_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&position_attr, &VertexAttrDescriptor {
            size: 3,
            class: VertexAttrClass::Float,
            attr_type: VertexAttrType::F32,
            stride: 4 * 4,
            offset: 0,
            divisor: 0,
        });
        device.bind_buffer(&index_buffer, BufferTarget::Index);

        StencilVertexArray { vertex_array, vertex_buffer, index_buffer }
    }
}

struct ReprojectionProgram<D>
where
    D: Device,
{
    program: D::Program,
    old_transform_uniform: D::Uniform,
    new_transform_uniform: D::Uniform,
    texture_uniform: D::Uniform,
}

impl<D> ReprojectionProgram<D>
where
    D: Device,
{
    fn new(device: &D, resources: &dyn ResourceLoader) -> ReprojectionProgram<D> {
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

struct ReprojectionVertexArray<D>
where
    D: Device,
{
    vertex_array: D::VertexArray,
}

impl<D> ReprojectionVertexArray<D>
where
    D: Device,
{
    fn new(
        device: &D,
        reprojection_program: &ReprojectionProgram<D>,
        quad_vertex_positions_buffer: &D::Buffer,
        quad_vertex_indices_buffer: &D::Buffer,
    ) -> ReprojectionVertexArray<D> {
        let vertex_array = device.create_vertex_array();

        let position_attr = device.get_vertex_attr(&reprojection_program.program, "Position")
                                  .unwrap();
        device.bind_vertex_array(&vertex_array);
        device.use_program(&reprojection_program.program);
        device.bind_buffer(quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_vertex_attr(&position_attr, &VertexAttrDescriptor {
            size: 2,
            class: VertexAttrClass::Float,
            attr_type: VertexAttrType::U8,
            stride: 0,
            offset: 0,
            divisor: 0,
        });
        device.bind_buffer(quad_vertex_indices_buffer, BufferTarget::Index);

        ReprojectionVertexArray { vertex_array }
    }
}

#[derive(Clone)]
pub enum DestFramebuffer<D>
where
    D: Device,
{
    Default {
        viewport: RectI,
        window_size: Vector2I,
    },
    Other(D::Framebuffer),
}

impl<D> DestFramebuffer<D>
where
    D: Device,
{
    #[inline]
    pub fn full_window(window_size: Vector2I) -> DestFramebuffer<D> {
        let viewport = RectI::new(Vector2I::default(), window_size);
        DestFramebuffer::Default { viewport, window_size }
    }

    fn window_size(&self, device: &D) -> Vector2I {
        match *self {
            DestFramebuffer::Default { window_size, .. } => window_size,
            DestFramebuffer::Other(ref framebuffer) => {
                device.texture_size(device.framebuffer_texture(framebuffer))
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum RenderMode {
    Multicolor,
    Monochrome {
        fg_color: ColorF,
        bg_color: ColorF,
        defringing_kernel: Option<DefringingKernel>,
        gamma_correction: bool,
    },
}

impl Default for RenderMode {
    #[inline]
    fn default() -> RenderMode {
        RenderMode::Multicolor
    }
}

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
