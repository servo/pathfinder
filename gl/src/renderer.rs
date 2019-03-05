// pathfinder/gl/src/renderer.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::debug::DebugUI;
use pathfinder_geometry::basic::point::{Point2DI32, Point3DF32};
use pathfinder_gpu::{BlendState, BufferTarget, BufferUploadMode, DepthFunc, DepthState, Device};
use pathfinder_gpu::{Primitive, RenderState, Resources, StencilFunc, StencilState, TextureFormat};
use pathfinder_gpu::{UniformData, VertexAttrType};
use pathfinder_renderer::gpu_data::{Batch, BuiltScene, SolidTileScenePrimitive};
use pathfinder_renderer::paint::{ColorU, ObjectShader};
use pathfinder_renderer::post::DefringingKernel;
use pathfinder_renderer::tiles::{TILE_HEIGHT, TILE_WIDTH};
use pathfinder_simd::default::{F32x4, I32x4};
use std::collections::VecDeque;
use std::time::Duration;

static QUAD_VERTEX_POSITIONS: [u8; 8] = [0, 0, 1, 0, 1, 1, 0, 1];

const MASK_FRAMEBUFFER_WIDTH: i32 = TILE_WIDTH as i32 * 256;
const MASK_FRAMEBUFFER_HEIGHT: i32 = TILE_HEIGHT as i32 * 256;

// TODO(pcwalton): Replace with `mem::size_of` calls?
const FILL_INSTANCE_SIZE: usize = 8;
const SOLID_TILE_INSTANCE_SIZE: usize = 6;
const MASK_TILE_INSTANCE_SIZE: usize = 8;

const FILL_COLORS_TEXTURE_WIDTH: i32 = 256;
const FILL_COLORS_TEXTURE_HEIGHT: i32 = 256;

pub struct Renderer<D> where D: Device {
    // Device
    pub device: D,

    // Core data
    fill_program: FillProgram<D>,
    solid_tile_program: SolidTileProgram<D>,
    mask_tile_program: MaskTileProgram<D>,
    area_lut_texture: D::Texture,
    quad_vertex_positions_buffer: D::Buffer,
    fill_vertex_array: FillVertexArray<D>,
    mask_tile_vertex_array: MaskTileVertexArray<D>,
    solid_tile_vertex_array: SolidTileVertexArray<D>,
    mask_framebuffer: D::Framebuffer,
    fill_colors_texture: D::Texture,

    // Postprocessing shader
    postprocess_source_framebuffer: Option<D::Framebuffer>,
    postprocess_program: PostprocessProgram<D>,
    postprocess_vertex_array: PostprocessVertexArray<D>,
    gamma_lut_texture: D::Texture,

    // Stencil shader
    stencil_program: StencilProgram<D>,
    stencil_vertex_array: StencilVertexArray<D>,

    // Debug
    pending_timer_queries: VecDeque<D::TimerQuery>,
    free_timer_queries: Vec<D::TimerQuery>,
    pub debug_ui: DebugUI<D>,

    // Extra info
    main_framebuffer_size: Point2DI32,
    postprocess_options: PostprocessOptions,
    use_depth: bool,
}

impl<D> Renderer<D> where D: Device {
    pub fn new(device: D, resources: &Resources, main_framebuffer_size: Point2DI32)
               -> Renderer<D> {
        let fill_program = FillProgram::new(&device, &resources);
        let solid_tile_program = SolidTileProgram::new(&device, &resources);
        let mask_tile_program = MaskTileProgram::new(&device, &resources);

        let postprocess_program = PostprocessProgram::new(&device, &resources);
        let stencil_program = StencilProgram::new(&device, &resources);

        let area_lut_texture = device.create_texture_from_png(&resources, "area-lut");
        let gamma_lut_texture = device.create_texture_from_png(&resources, "gamma-lut");

        let quad_vertex_positions_buffer = device.create_buffer();
        device.upload_to_buffer(&quad_vertex_positions_buffer,
                                &QUAD_VERTEX_POSITIONS,
                                BufferTarget::Vertex,
                                BufferUploadMode::Static);

        let fill_vertex_array = FillVertexArray::new(&device,
                                                     &fill_program,
                                                     &quad_vertex_positions_buffer);
        let mask_tile_vertex_array = MaskTileVertexArray::new(&device,
                                                              &mask_tile_program,
                                                              &quad_vertex_positions_buffer);
        let solid_tile_vertex_array = SolidTileVertexArray::new(&device,
                                                                &solid_tile_program,
                                                                &quad_vertex_positions_buffer);
        let postprocess_vertex_array = PostprocessVertexArray::new(&device,
                                                                   &postprocess_program,
                                                                   &quad_vertex_positions_buffer);
        let stencil_vertex_array = StencilVertexArray::new(&device, &stencil_program);

        let mask_framebuffer_size = Point2DI32::new(MASK_FRAMEBUFFER_WIDTH,
                                                    MASK_FRAMEBUFFER_HEIGHT);
        let mask_framebuffer_texture = device.create_texture(TextureFormat::R16F,
                                                             mask_framebuffer_size);
        let mask_framebuffer = device.create_framebuffer(mask_framebuffer_texture);

        let fill_colors_size = Point2DI32::new(FILL_COLORS_TEXTURE_WIDTH,
                                               FILL_COLORS_TEXTURE_HEIGHT);
        let fill_colors_texture = device.create_texture(TextureFormat::RGBA8, fill_colors_size);

        let debug_ui = DebugUI::new(&device, &resources, main_framebuffer_size);

        Renderer {
            device,
            fill_program,
            solid_tile_program,
            mask_tile_program,
            area_lut_texture,
            quad_vertex_positions_buffer,
            fill_vertex_array,
            mask_tile_vertex_array,
            solid_tile_vertex_array,
            mask_framebuffer,
            fill_colors_texture,

            postprocess_source_framebuffer: None,
            postprocess_program,
            postprocess_vertex_array,
            gamma_lut_texture,

            stencil_program,
            stencil_vertex_array,

            pending_timer_queries: VecDeque::new(),
            free_timer_queries: vec![],

            debug_ui,

            main_framebuffer_size,
            postprocess_options: PostprocessOptions::default(),
            use_depth: false,
        }
    }

    pub fn render_scene(&mut self, built_scene: &BuiltScene) {
        self.init_postprocessing_framebuffer();

        let timer_query = self.free_timer_queries
                              .pop()
                              .unwrap_or_else(|| self.device.create_timer_query());
        self.device.begin_timer_query(&timer_query);

        self.upload_shaders(&built_scene.shaders);

        if self.use_depth {
            self.draw_stencil(&built_scene.quad);
        }

        self.upload_solid_tiles(&built_scene.solid_tiles);
        self.draw_solid_tiles(&built_scene);

        for batch in &built_scene.batches {
            self.upload_batch(batch);
            self.draw_batch_fills(batch);
            self.draw_batch_mask_tiles(batch);
        }

        if self.postprocessing_needed() {
            self.postprocess();
        }

        self.device.end_timer_query(&timer_query);
        self.pending_timer_queries.push_back(timer_query);
    }

    pub fn shift_timer_query(&mut self) -> Option<Duration> {
        let query = self.pending_timer_queries.front()?;
        if !self.device.timer_query_is_available(&query) {
            return None
        }
        let query = self.pending_timer_queries.pop_front().unwrap();
        let result = self.device.get_timer_query(&query);
        self.free_timer_queries.push(query);
        Some(result)
    }

    #[inline]
    pub fn set_main_framebuffer_size(&mut self, new_framebuffer_size: Point2DI32) {
        self.main_framebuffer_size = new_framebuffer_size;
        self.debug_ui.set_framebuffer_size(new_framebuffer_size);
    }

    #[inline]
    pub fn disable_subpixel_aa(&mut self) {
        self.postprocess_options.defringing_kernel = None;
    }

    #[inline]
    pub fn enable_subpixel_aa(&mut self, defringing_kernel: &DefringingKernel) {
        self.postprocess_options.defringing_kernel = Some(*defringing_kernel);
    }

    #[inline]
    pub fn disable_gamma_correction(&mut self) {
        self.postprocess_options.gamma_correction_bg_color = None;
    }

    #[inline]
    pub fn enable_gamma_correction(&mut self, bg_color: ColorU) {
        self.postprocess_options.gamma_correction_bg_color = Some(bg_color);
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

    fn upload_shaders(&mut self, shaders: &[ObjectShader]) {
        let size = Point2DI32::new(FILL_COLORS_TEXTURE_WIDTH, FILL_COLORS_TEXTURE_HEIGHT);
        let mut fill_colors = vec![0; size.x() as usize * size.y() as usize * 4];
        for (shader_index, shader) in shaders.iter().enumerate() {
            fill_colors[shader_index * 4 + 0] = shader.fill_color.r;
            fill_colors[shader_index * 4 + 1] = shader.fill_color.g;
            fill_colors[shader_index * 4 + 2] = shader.fill_color.b;
            fill_colors[shader_index * 4 + 3] = shader.fill_color.a;
        }
        self.device.upload_to_texture(&self.fill_colors_texture, size, &fill_colors);
    }

    fn upload_solid_tiles(&mut self, solid_tiles: &[SolidTileScenePrimitive]) {
        self.device.upload_to_buffer(&self.solid_tile_vertex_array.vertex_buffer,
                                     solid_tiles,
                                     BufferTarget::Vertex,
                                     BufferUploadMode::Dynamic);
    }

    fn upload_batch(&mut self, batch: &Batch) {
        self.device.upload_to_buffer(&self.fill_vertex_array.vertex_buffer,
                                     &batch.fills,
                                     BufferTarget::Vertex,
                                     BufferUploadMode::Dynamic);
        self.device.upload_to_buffer(&self.mask_tile_vertex_array.vertex_buffer,
                                     &batch.mask_tiles,
                                     BufferTarget::Vertex,
                                     BufferUploadMode::Dynamic);
    }

    fn draw_batch_fills(&mut self, batch: &Batch) {
        self.device.bind_framebuffer(&self.mask_framebuffer);
        // TODO(pcwalton): Only clear the appropriate portion?
        self.device.clear(Some(F32x4::splat(0.0)), None, None);

        self.device.bind_vertex_array(&self.fill_vertex_array.vertex_array);
        self.device.use_program(&self.fill_program.program);
        self.device.set_uniform(&self.fill_program.framebuffer_size_uniform,
                                UniformData::Vec2(I32x4::new(MASK_FRAMEBUFFER_WIDTH,
                                                             MASK_FRAMEBUFFER_HEIGHT,
                                                             0,
                                                             0).to_f32x4()));
        self.device.set_uniform(&self.fill_program.tile_size_uniform,
                                UniformData::Vec2(I32x4::new(TILE_WIDTH as i32,
                                                             TILE_HEIGHT as i32,
                                                             0,
                                                             0).to_f32x4()));
        self.device.bind_texture(&self.area_lut_texture, 0);
        self.device.set_uniform(&self.fill_program.area_lut_uniform,
                                UniformData::TextureUnit(0));
        let render_state = RenderState {
            blend: BlendState::RGBOneAlphaOne,
            ..RenderState::default()
        };
        self.device.draw_arrays_instanced(Primitive::TriangleFan,
                                          4,
                                          batch.fills.len() as u32,
                                          &render_state);
    }

    fn draw_batch_mask_tiles(&mut self, batch: &Batch) {
        self.bind_draw_framebuffer();

        self.device.bind_vertex_array(&self.mask_tile_vertex_array.vertex_array);
        self.device.use_program(&self.mask_tile_program.program);
        self.device.set_uniform(&self.mask_tile_program.framebuffer_size_uniform,
                                UniformData::Vec2(self.main_framebuffer_size.0.to_f32x4()));
        self.device.set_uniform(&self.mask_tile_program.tile_size_uniform,
                                UniformData::Vec2(I32x4::new(TILE_WIDTH as i32,
                                                             TILE_HEIGHT as i32,
                                                             0,
                                                             0).to_f32x4()));
        self.device.bind_texture(self.device.framebuffer_texture(&self.mask_framebuffer), 0);
        self.device.set_uniform(&self.mask_tile_program.stencil_texture_uniform,
                                UniformData::TextureUnit(0));
        self.device.set_uniform(&self.mask_tile_program.stencil_texture_size_uniform,
                                UniformData::Vec2(I32x4::new(MASK_FRAMEBUFFER_WIDTH,
                                                             MASK_FRAMEBUFFER_HEIGHT,
                                                             0,
                                                             0).to_f32x4()));
        self.device.bind_texture(&self.fill_colors_texture, 1);
        self.device.set_uniform(&self.mask_tile_program.fill_colors_texture_uniform,
                                UniformData::TextureUnit(1));
        self.device.set_uniform(&self.mask_tile_program.fill_colors_texture_size_uniform,
                                UniformData::Vec2(I32x4::new(FILL_COLORS_TEXTURE_WIDTH,
                                                             FILL_COLORS_TEXTURE_HEIGHT,
                                                             0,
                                                             0).to_f32x4()));
        // FIXME(pcwalton): Fill this in properly!
        self.device.set_uniform(&self.mask_tile_program.view_box_origin_uniform,
                                UniformData::Vec2(F32x4::default()));
        let render_state = RenderState {
            blend: BlendState::RGBSrcAlphaAlphaOneMinusSrcAlpha,
            stencil: self.stencil_state(),
            ..RenderState::default()
        };
        self.device.draw_arrays_instanced(Primitive::TriangleFan,
                                          4,
                                          batch.mask_tiles.len() as u32,
                                          &render_state);
    }

    fn draw_solid_tiles(&mut self, built_scene: &BuiltScene) {
        self.device.bind_vertex_array(&self.solid_tile_vertex_array.vertex_array);
        self.device.use_program(&self.solid_tile_program.program);
        self.device.set_uniform(&self.solid_tile_program.framebuffer_size_uniform,
                                UniformData::Vec2(self.main_framebuffer_size.0.to_f32x4()));
        self.device.set_uniform(&self.solid_tile_program.tile_size_uniform,
                                UniformData::Vec2(I32x4::new(TILE_WIDTH as i32,
                                                             TILE_HEIGHT as i32,
                                                             0,
                                                             0).to_f32x4()));
        self.device.bind_texture(&self.fill_colors_texture, 0);
        self.device.set_uniform(&self.solid_tile_program.fill_colors_texture_uniform,
                                UniformData::TextureUnit(0));
        self.device.set_uniform(&self.solid_tile_program.fill_colors_texture_size_uniform,
                                UniformData::Vec2(I32x4::new(FILL_COLORS_TEXTURE_WIDTH,
                                                             FILL_COLORS_TEXTURE_HEIGHT,
                                                             0,
                                                             0).to_f32x4()));
        // FIXME(pcwalton): Fill this in properly!
        self.device.set_uniform(&self.solid_tile_program.view_box_origin_uniform,
                                UniformData::Vec2(F32x4::default()));
        let render_state = RenderState {
            stencil: self.stencil_state(),
            ..RenderState::default()
        };
        let count = built_scene.solid_tiles.len() as u32;
        self.device.draw_arrays_instanced(Primitive::TriangleFan, 4, count, &render_state);
    }

    fn postprocess(&mut self) {
        self.device.bind_default_framebuffer(self.main_framebuffer_size);

        self.device.bind_vertex_array(&self.postprocess_vertex_array.vertex_array);
        self.device.use_program(&self.postprocess_program.program);
        self.device.set_uniform(&self.postprocess_program.framebuffer_size_uniform,
                                UniformData::Vec2(self.main_framebuffer_size.to_f32().0));
        match self.postprocess_options.defringing_kernel {
            Some(ref kernel) => {
                self.device.set_uniform(&self.postprocess_program.kernel_uniform,
                                        UniformData::Vec4(F32x4::from_slice(&kernel.0)));
            }
            None => {
                self.device.set_uniform(&self.postprocess_program.kernel_uniform,
                                        UniformData::Vec4(F32x4::default()));
            }
        }
        let source_texture =
            self.device.framebuffer_texture(self.postprocess_source_framebuffer.as_ref().unwrap());
        self.device.bind_texture(&source_texture, 0);
        self.device.set_uniform(&self.postprocess_program.source_uniform,
                                UniformData::TextureUnit(0));
        self.device.bind_texture(&self.gamma_lut_texture, 1);
        self.device.set_uniform(&self.postprocess_program.gamma_lut_uniform,
                                UniformData::TextureUnit(1));
        let gamma_correction_bg_color_uniform = &self.postprocess_program
                                                     .gamma_correction_bg_color_uniform;
        match self.postprocess_options.gamma_correction_bg_color {
            None => {
                self.device.set_uniform(gamma_correction_bg_color_uniform,
                                        UniformData::Vec4(F32x4::default()));
            }
            Some(color) => {
                self.device.set_uniform(gamma_correction_bg_color_uniform,
                                        UniformData::Vec4(color.to_f32().0));
            }
        }
        self.device.draw_arrays(Primitive::TriangleFan, 4, &RenderState {
            blend: BlendState::RGBSrcAlphaAlphaOneMinusSrcAlpha,
            ..RenderState::default()
        });
    }

    fn draw_stencil(&self, quad_positions: &[Point3DF32]) {
        self.device.upload_to_buffer(&self.stencil_vertex_array.vertex_buffer,
                                     quad_positions,
                                     BufferTarget::Vertex,
                                     BufferUploadMode::Dynamic);
        self.bind_draw_framebuffer();

        self.device.bind_vertex_array(&self.stencil_vertex_array.vertex_array);
        self.device.use_program(&self.stencil_program.program);
        self.device.draw_arrays(Primitive::TriangleFan, 4, &RenderState {
            // FIXME(pcwalton): Should we really write to the depth buffer?
            depth: Some(DepthState { func: DepthFunc::Less, write: true }),
            stencil: Some(StencilState {
                func: StencilFunc::Always,
                reference: 1,
                mask: 1,
                write: true,
            }),
            color_mask: false,
            ..RenderState::default()
        })
    }

    fn bind_draw_framebuffer(&self) {
        if self.postprocessing_needed() {
            self.device.bind_framebuffer(self.postprocess_source_framebuffer.as_ref().unwrap());
        } else {
            self.device.bind_default_framebuffer(self.main_framebuffer_size);
        }
    }

    fn init_postprocessing_framebuffer(&mut self) {
        if !self.postprocessing_needed() {
            self.postprocess_source_framebuffer = None;
            return;
        }

        match self.postprocess_source_framebuffer {
            Some(ref framebuffer) if
                    self.device.texture_size(self.device.framebuffer_texture(framebuffer)) ==
                    self.main_framebuffer_size => {}
            _ => {
                let texture = self.device.create_texture(TextureFormat::RGBA8,
                                                         self.main_framebuffer_size);
                self.postprocess_source_framebuffer = Some(self.device.create_framebuffer(texture))
            }
        };

        self.device.bind_framebuffer(self.postprocess_source_framebuffer.as_ref().unwrap());
        self.device.clear(Some(F32x4::default()), None, None);
    }

    fn postprocessing_needed(&self) -> bool {
        self.postprocess_options.defringing_kernel.is_some() ||
            self.postprocess_options.gamma_correction_bg_color.is_some()
    }

    fn stencil_state(&self) -> Option<StencilState> {
        if !self.use_depth {
            return None;
        }

        Some(StencilState { func: StencilFunc::Equal, reference: 1, mask: 1, write: false })
    }
}

#[derive(Clone, Copy, Default)]
struct PostprocessOptions {
    defringing_kernel: Option<DefringingKernel>,
    gamma_correction_bg_color: Option<ColorU>,
}

struct FillVertexArray<D> where D: Device {
    vertex_array: D::VertexArray,
    vertex_buffer: D::Buffer,
}

impl<D> FillVertexArray<D> where D: Device {
    fn new(device: &D, fill_program: &FillProgram<D>, quad_vertex_positions_buffer: &D::Buffer)
           -> FillVertexArray<D> {
        let vertex_array = device.create_vertex_array();
        let vertex_buffer = device.create_buffer();

        let tess_coord_attr = device.get_vertex_attr(&fill_program.program, "TessCoord");
        let from_px_attr = device.get_vertex_attr(&fill_program.program, "FromPx");
        let to_px_attr = device.get_vertex_attr(&fill_program.program, "ToPx");
        let from_subpx_attr = device.get_vertex_attr(&fill_program.program, "FromSubpx");
        let to_subpx_attr = device.get_vertex_attr(&fill_program.program, "ToSubpx");
        let tile_index_attr = device.get_vertex_attr(&fill_program.program, "TileIndex");

        device.bind_vertex_array(&vertex_array);
        device.use_program(&fill_program.program);
        device.bind_buffer(quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_float_vertex_attr(&tess_coord_attr,
                                           2,
                                           VertexAttrType::U8,
                                           false,
                                           0,
                                           0,
                                           0);
        device.bind_buffer(&vertex_buffer, BufferTarget::Vertex);
        device.configure_int_vertex_attr(&from_px_attr,
                                         1,
                                         VertexAttrType::U8,
                                         FILL_INSTANCE_SIZE,
                                         0,
                                         1);
        device.configure_int_vertex_attr(&to_px_attr,
                                         1,
                                         VertexAttrType::U8,
                                         FILL_INSTANCE_SIZE,
                                         1,
                                         1);
        device.configure_float_vertex_attr(&from_subpx_attr,
                                           2,
                                           VertexAttrType::U8,
                                           true,
                                           FILL_INSTANCE_SIZE,
                                           2,
                                           1);
        device.configure_float_vertex_attr(&to_subpx_attr,
                                           2,
                                           VertexAttrType::U8,
                                           true,
                                           FILL_INSTANCE_SIZE,
                                           4,
                                           1);
        device.configure_int_vertex_attr(&tile_index_attr,
                                         1,
                                         VertexAttrType::U16,
                                         FILL_INSTANCE_SIZE,
                                         6,
                                         1);

        FillVertexArray { vertex_array, vertex_buffer }
    }
}

struct MaskTileVertexArray<D> where D: Device {
    vertex_array: D::VertexArray,
    vertex_buffer: D::Buffer,
}

impl<D> MaskTileVertexArray<D> where D: Device {
    fn new(device: &D,
           mask_tile_program: &MaskTileProgram<D>,
           quad_vertex_positions_buffer: &D::Buffer)
           -> MaskTileVertexArray<D> {
        let (vertex_array, vertex_buffer) = (device.create_vertex_array(), device.create_buffer());

        let tess_coord_attr = device.get_vertex_attr(&mask_tile_program.program, "TessCoord");
        let tile_origin_attr = device.get_vertex_attr(&mask_tile_program.program, "TileOrigin");
        let backdrop_attr = device.get_vertex_attr(&mask_tile_program.program, "Backdrop");
        let object_attr = device.get_vertex_attr(&mask_tile_program.program, "Object");

        // NB: The object must be of type `I16`, not `U16`, to work around a macOS Radeon
        // driver bug.
        device.bind_vertex_array(&vertex_array);
        device.use_program(&mask_tile_program.program);
        device.bind_buffer(quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_float_vertex_attr(&tess_coord_attr,
                                            2,
                                            VertexAttrType::U8,
                                            false,
                                            0,
                                            0,
                                            0);
        device.bind_buffer(&vertex_buffer, BufferTarget::Vertex);
        device.configure_float_vertex_attr(&tile_origin_attr,
                                            2,
                                            VertexAttrType::I16,
                                            false,
                                            MASK_TILE_INSTANCE_SIZE,
                                            0,
                                            1);
        device.configure_int_vertex_attr(&backdrop_attr,
                                            1,
                                            VertexAttrType::I16,
                                            MASK_TILE_INSTANCE_SIZE,
                                            4,
                                            1);
        device.configure_int_vertex_attr(&object_attr,
                                            2,
                                            VertexAttrType::I16,
                                            MASK_TILE_INSTANCE_SIZE,
                                            6,
                                            1);

        MaskTileVertexArray { vertex_array, vertex_buffer }
    }
}

struct SolidTileVertexArray<D> where D: Device {
    vertex_array: D::VertexArray,
    vertex_buffer: D::Buffer,
}

impl<D> SolidTileVertexArray<D> where D: Device {
    fn new(device: &D,
           solid_tile_program: &SolidTileProgram<D>,
           quad_vertex_positions_buffer: &D::Buffer)
           -> SolidTileVertexArray<D> {
        let (vertex_array, vertex_buffer) = (device.create_vertex_array(), device.create_buffer());

        let tess_coord_attr = device.get_vertex_attr(&solid_tile_program.program, "TessCoord");
        let tile_origin_attr = device.get_vertex_attr(&solid_tile_program.program, "TileOrigin");
        let object_attr = device.get_vertex_attr(&solid_tile_program.program, "Object");

        // NB: The object must be of type short, not unsigned short, to work around a macOS
        // Radeon driver bug.
        device.bind_vertex_array(&vertex_array);
        device.use_program(&solid_tile_program.program);
        device.bind_buffer(quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_float_vertex_attr(&tess_coord_attr,
                                            2,
                                            VertexAttrType::U8,
                                            false,
                                            0,
                                            0,
                                            0);
        device.bind_buffer(&vertex_buffer, BufferTarget::Vertex);
        device.configure_float_vertex_attr(&tile_origin_attr,
                                            2,
                                            VertexAttrType::I16,
                                            false,
                                            SOLID_TILE_INSTANCE_SIZE,
                                            0,
                                            1);
        device.configure_int_vertex_attr(&object_attr,
                                            1,
                                            VertexAttrType::I16,
                                            SOLID_TILE_INSTANCE_SIZE,
                                            4,
                                            1);

        SolidTileVertexArray { vertex_array, vertex_buffer }
    }
}

struct FillProgram<D> where D: Device {
    program: D::Program,
    framebuffer_size_uniform: D::Uniform,
    tile_size_uniform: D::Uniform,
    area_lut_uniform: D::Uniform,
}

impl<D> FillProgram<D> where D: Device {
    fn new(device: &D, resources: &Resources) -> FillProgram<D> {
        let program = device.create_program(resources, "fill");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let tile_size_uniform = device.get_uniform(&program, "TileSize");
        let area_lut_uniform = device.get_uniform(&program, "AreaLUT");
        FillProgram { program, framebuffer_size_uniform, tile_size_uniform, area_lut_uniform }
    }
}

struct SolidTileProgram<D> where D: Device {
    program: D::Program,
    framebuffer_size_uniform: D::Uniform,
    tile_size_uniform: D::Uniform,
    fill_colors_texture_uniform: D::Uniform,
    fill_colors_texture_size_uniform: D::Uniform,
    view_box_origin_uniform: D::Uniform,
}

impl<D> SolidTileProgram<D> where D: Device {
    fn new(device: &D, resources: &Resources) -> SolidTileProgram<D> {
        let program = device.create_program(resources, "solid_tile");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let tile_size_uniform = device.get_uniform(&program, "TileSize");
        let fill_colors_texture_uniform = device.get_uniform(&program, "FillColorsTexture");
        let fill_colors_texture_size_uniform = device.get_uniform(&program,
                                                                  "FillColorsTextureSize");
        let view_box_origin_uniform = device.get_uniform(&program, "ViewBoxOrigin");
        SolidTileProgram {
            program,
            framebuffer_size_uniform,
            tile_size_uniform,
            fill_colors_texture_uniform,
            fill_colors_texture_size_uniform,
            view_box_origin_uniform,
        }
    }
}

struct MaskTileProgram<D> where D: Device {
    program: D::Program,
    framebuffer_size_uniform: D::Uniform,
    tile_size_uniform: D::Uniform,
    stencil_texture_uniform: D::Uniform,
    stencil_texture_size_uniform: D::Uniform,
    fill_colors_texture_uniform: D::Uniform,
    fill_colors_texture_size_uniform: D::Uniform,
    view_box_origin_uniform: D::Uniform,
}

impl<D> MaskTileProgram<D> where D: Device {
    fn new(device: &D, resources: &Resources) -> MaskTileProgram<D> {
        let program = device.create_program(resources, "mask_tile");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let tile_size_uniform = device.get_uniform(&program, "TileSize");
        let stencil_texture_uniform = device.get_uniform(&program, "StencilTexture");
        let stencil_texture_size_uniform = device.get_uniform(&program, "StencilTextureSize");
        let fill_colors_texture_uniform = device.get_uniform(&program, "FillColorsTexture");
        let fill_colors_texture_size_uniform = device.get_uniform(&program,
                                                                  "FillColorsTextureSize");
        let view_box_origin_uniform = device.get_uniform(&program, "ViewBoxOrigin");
        MaskTileProgram {
            program,
            framebuffer_size_uniform,
            tile_size_uniform,
            stencil_texture_uniform,
            stencil_texture_size_uniform,
            fill_colors_texture_uniform,
            fill_colors_texture_size_uniform,
            view_box_origin_uniform,
        }
    }
}

struct PostprocessProgram<D> where D: Device {
    program: D::Program,
    source_uniform: D::Uniform,
    framebuffer_size_uniform: D::Uniform,
    kernel_uniform: D::Uniform,
    gamma_lut_uniform: D::Uniform,
    gamma_correction_bg_color_uniform: D::Uniform,
}

impl<D> PostprocessProgram<D> where D: Device {
    fn new(device: &D, resources: &Resources) -> PostprocessProgram<D> {
        let program = device.create_program(resources, "post");
        let source_uniform = device.get_uniform(&program, "Source");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let kernel_uniform = device.get_uniform(&program, "Kernel");
        let gamma_lut_uniform = device.get_uniform(&program, "GammaLUT");
        let gamma_correction_bg_color_uniform = device.get_uniform(&program,
                                                                   "GammaCorrectionBGColor");
        PostprocessProgram {
            program,
            source_uniform,
            framebuffer_size_uniform,
            kernel_uniform,
            gamma_lut_uniform,
            gamma_correction_bg_color_uniform,
        }
    }
}

struct PostprocessVertexArray<D> where D: Device {
    vertex_array: D::VertexArray,
}

impl<D> PostprocessVertexArray<D> where D: Device {
    fn new(device: &D,
           postprocess_program: &PostprocessProgram<D>,
           quad_vertex_positions_buffer: &D::Buffer)
           -> PostprocessVertexArray<D> {
        let vertex_array = device.create_vertex_array();
        let position_attr = device.get_vertex_attr(&postprocess_program.program, "Position");

        device.bind_vertex_array(&vertex_array);
        device.use_program(&postprocess_program.program);
        device.bind_buffer(quad_vertex_positions_buffer, BufferTarget::Vertex);
        device.configure_float_vertex_attr(&position_attr,
                                            2,
                                            VertexAttrType::U8,
                                            false,
                                            0,
                                            0,
                                            0);

        PostprocessVertexArray { vertex_array }
    }
}

struct StencilProgram<D> where D: Device {
    program: D::Program,
}

impl<D> StencilProgram<D> where D: Device {
    fn new(device: &D, resources: &Resources) -> StencilProgram<D> {
        let program = device.create_program(resources, "stencil");
        StencilProgram { program }
    }
}

struct StencilVertexArray<D> where D: Device {
    vertex_array: D::VertexArray,
    vertex_buffer: D::Buffer,
}

impl<D> StencilVertexArray<D> where D: Device {
    fn new(device: &D, stencil_program: &StencilProgram<D>) -> StencilVertexArray<D> {
        let (vertex_array, vertex_buffer) = (device.create_vertex_array(), device.create_buffer());

        let position_attr = device.get_vertex_attr(&stencil_program.program, "Position");

        device.bind_vertex_array(&vertex_array);
        device.use_program(&stencil_program.program);
        device.bind_buffer(&vertex_buffer, BufferTarget::Vertex);
        device.configure_float_vertex_attr(&position_attr,
                                           3,
                                           VertexAttrType::F32,
                                           false,
                                           4 * 4,
                                           0,
                                           0);

        StencilVertexArray { vertex_array, vertex_buffer }
    }
}
