// pathfinder/renderer/src/gpu/renderer.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The GPU renderer that processes commands necessary to render a scene.

use crate::gpu::blend::{ToBlendState, ToCompositeCtrl};
use crate::gpu::d3d9::renderer::RendererD3D9;
use crate::gpu::d3d11::renderer::RendererD3D11;
use crate::gpu::debug::DebugUIPresenter;
use crate::gpu::options::{DestFramebuffer, RendererLevel, RendererMode, RendererOptions};
use crate::gpu::perf::{PendingTimer, RenderStats, RenderTime, TimeCategory, TimerQueryCache};
use crate::gpu::shaders::{BlitProgram, BlitVertexArray, ClearProgram, ClearVertexArray};
use crate::gpu::shaders::{ProgramsCore, ReprojectionProgram, ReprojectionVertexArray};
use crate::gpu::shaders::{StencilProgram, StencilVertexArray, TileProgramCommon, VertexArraysCore};
use crate::gpu_data::{ColorCombineMode, RenderCommand, TextureLocation, TextureMetadataEntry};
use crate::gpu_data::{TexturePageDescriptor, TexturePageId, TileBatchTexture};
use crate::options::BoundingQuad;
use crate::tiles::{TILE_HEIGHT, TILE_WIDTH};
use half::f16;
use pathfinder_color::{self as color, ColorF, ColorU};
use pathfinder_content::effects::{BlendMode, BlurDirection, Filter, PatternFilter};
use pathfinder_content::render_target::RenderTargetId;
use pathfinder_geometry::rect::{RectF, RectI};
use pathfinder_geometry::transform3d::Transform4F;
use pathfinder_geometry::util;
use pathfinder_geometry::vector::{Vector2F, Vector2I, Vector4F, vec2f, vec2i};
use pathfinder_gpu::allocator::{BufferTag, FramebufferID, FramebufferTag, GeneralBufferID};
use pathfinder_gpu::allocator::{GPUMemoryAllocator, IndexBufferID, TextureID, TextureTag};
use pathfinder_gpu::{BufferData, BufferTarget, ClearOps, DepthFunc, DepthState, Device, Primitive};
use pathfinder_gpu::{RenderOptions, RenderState, RenderTarget, StencilFunc, StencilState};
use pathfinder_gpu::{TextureBinding, TextureDataRef, TextureFormat, UniformBinding, UniformData};
use pathfinder_resources::ResourceLoader;
use pathfinder_simd::default::{F32x2, F32x4, I32x2};
use std::collections::VecDeque;
use std::f32;
use std::time::Duration;
use std::u32;

static QUAD_VERTEX_POSITIONS: [u16; 8] = [0, 0, 1, 0, 1, 1, 0, 1];
static QUAD_VERTEX_INDICES: [u32; 6] = [0, 1, 3, 1, 2, 3];

pub(crate) const MASK_TILES_ACROSS: u32 = 256;
pub(crate) const MASK_TILES_DOWN: u32 = 256;

// 1.0 / sqrt(2*pi)
const SQRT_2_PI_INV: f32 = 0.3989422804014327;

const TEXTURE_METADATA_ENTRIES_PER_ROW: i32 = 128;
const TEXTURE_METADATA_TEXTURE_WIDTH:   i32 = TEXTURE_METADATA_ENTRIES_PER_ROW * 10;
const TEXTURE_METADATA_TEXTURE_HEIGHT:  i32 = 65536 / TEXTURE_METADATA_ENTRIES_PER_ROW;

// FIXME(pcwalton): Shrink this again!
pub(crate) const MASK_FRAMEBUFFER_WIDTH:  i32 = TILE_WIDTH as i32      * MASK_TILES_ACROSS as i32;
pub(crate) const MASK_FRAMEBUFFER_HEIGHT: i32 = TILE_HEIGHT as i32 / 4 * MASK_TILES_DOWN as i32;

const COMBINER_CTRL_FILTER_RADIAL_GRADIENT: i32 =   0x1;
const COMBINER_CTRL_FILTER_TEXT: i32 =              0x2;
const COMBINER_CTRL_FILTER_BLUR: i32 =              0x3;
const COMBINER_CTRL_FILTER_COLOR_MATRIX: i32 =      0x4;

const COMBINER_CTRL_COLOR_FILTER_SHIFT: i32 =       4;
const COMBINER_CTRL_COLOR_COMBINE_SHIFT: i32 =      8;
const COMBINER_CTRL_COMPOSITE_SHIFT: i32 =         10;

/// The GPU renderer that processes commands necessary to render a scene.
pub struct Renderer<D> where D: Device {
    // Basic data
    pub(crate) core: RendererCore<D>,
    level_impl: RendererLevelImpl<D>,

    // Shaders
    blit_program: BlitProgram<D>,
    clear_program: ClearProgram<D>,
    stencil_program: StencilProgram<D>,
    reprojection_program: ReprojectionProgram<D>,

    // Frames
    frame: Frame<D>,

    // Debug
    current_cpu_build_time: Option<Duration>,
    pending_timers: VecDeque<PendingTimer<D>>,
    debug_ui_presenter: Option<DebugUIPresenter<D>>,
    last_stats: VecDeque<RenderStats>,
    last_rendering_time: Option<RenderTime>,
}

enum RendererLevelImpl<D> where D: Device {
    D3D9(RendererD3D9<D>),
    D3D11(RendererD3D11<D>),
}

pub(crate) struct RendererCore<D> where D: Device {
    // Basic data
    pub(crate) device: D,
    pub(crate) allocator: GPUMemoryAllocator<D>,
    pub(crate) mode: RendererMode,
    pub(crate) options: RendererOptions<D>,
    pub(crate) renderer_flags: RendererFlags,

    // Performance monitoring
    pub(crate) stats: RenderStats,
    pub(crate) current_timer: Option<PendingTimer<D>>,
    pub(crate) timer_query_cache: TimerQueryCache<D>,

    // Core shaders
    pub(crate) programs: ProgramsCore<D>,
    pub(crate) vertex_arrays: VertexArraysCore<D>,

    // Read-only static core resources
    pub(crate) quad_vertex_positions_buffer_id: GeneralBufferID,
    pub(crate) quad_vertex_indices_buffer_id: IndexBufferID,
    pub(crate) area_lut_texture_id: TextureID,
    pub(crate) gamma_lut_texture_id: TextureID,

    // Read-write static core resources
    intermediate_dest_framebuffer_id: FramebufferID,
    intermediate_dest_framebuffer_size: Vector2I,
    pub(crate) texture_metadata_texture_id: TextureID,

    // Dynamic resources and associated metadata
    render_targets: Vec<RenderTargetInfo>,
    pub(crate) render_target_stack: Vec<RenderTargetId>,
    pub(crate) pattern_texture_pages: Vec<Option<PatternTexturePage>>,
    pub(crate) mask_storage: Option<MaskStorage>,
    pub(crate) alpha_tile_count: u32,
    pub(crate) framebuffer_flags: FramebufferFlags,
}

// TODO(pcwalton): Remove this.
struct Frame<D> where D: Device {
    blit_vertex_array: BlitVertexArray<D>,
    clear_vertex_array: ClearVertexArray<D>,
    stencil_vertex_array: StencilVertexArray<D>,
    reprojection_vertex_array: ReprojectionVertexArray<D>,
}

pub(crate) struct MaskStorage {
    pub(crate) framebuffer_id: FramebufferID,
    pub(crate) allocated_page_count: u32,
}

impl<D> Renderer<D> where D: Device {
    /// Creates a new renderer ready to render Pathfinder content.
    /// 
    /// Arguments:
    /// 
    /// * `device`: The GPU device to render with. This effectively specifies the system GPU API
    ///   Pathfinder will use (OpenGL, Metal, etc.)
    /// 
    /// * `resources`: Where Pathfinder should find shaders, lookup tables, and other data.
    ///   This is typically either an `EmbeddedResourceLoader` to use resources included in the
    ///   Pathfinder library or (less commonly) a `FilesystemResourceLoader` to use resources
    ///   stored in a directory on disk.
    /// 
    /// * `mode`: Renderer options that can't be changed after the renderer is created. Most
    ///   notably, this specifies the API level (D3D9 or D3D11).
    /// 
    /// * `options`: Renderer options that can be changed after the renderer is created. Most
    ///   importantly, this specifies where the output should go (to a window or off-screen).
    pub fn new(device: D,
               resources: &dyn ResourceLoader,
               mode: RendererMode,
               options: RendererOptions<D>)
               -> Renderer<D> {
        let mut allocator = GPUMemoryAllocator::new();

        device.begin_commands();

        let quad_vertex_positions_buffer_id =
            allocator.allocate_general_buffer::<u16>(&device,
                                                     QUAD_VERTEX_POSITIONS.len() as u64,
                                                     BufferTag("QuadVertexPositions"));
        device.upload_to_buffer(allocator.get_general_buffer(quad_vertex_positions_buffer_id),
                                0,
                                &QUAD_VERTEX_POSITIONS,
                                BufferTarget::Vertex);
        let quad_vertex_indices_buffer_id =
            allocator.allocate_index_buffer::<u32>(&device,
                                                   QUAD_VERTEX_INDICES.len() as u64,
                                                   BufferTag("QuadVertexIndices"));
        device.upload_to_buffer(allocator.get_index_buffer(quad_vertex_indices_buffer_id),
                                0,
                                &QUAD_VERTEX_INDICES,
                                BufferTarget::Index);

        let area_lut_texture_id = allocator.allocate_texture(&device,
                                                             Vector2I::splat(256),
                                                             TextureFormat::RGBA8,
                                                             TextureTag("AreaLUT"));
        let gamma_lut_texture_id = allocator.allocate_texture(&device,
                                                              vec2i(256, 8),
                                                              TextureFormat::R8,
                                                              TextureTag("GammaLUT"));
        device.upload_png_to_texture(resources,
                                     "area-lut",
                                     allocator.get_texture(area_lut_texture_id),
                                     TextureFormat::RGBA8);
        device.upload_png_to_texture(resources,
                                     "gamma-lut",
                                     allocator.get_texture(gamma_lut_texture_id),
                                     TextureFormat::R8);

        let window_size = options.dest.window_size(&device);
        let intermediate_dest_framebuffer_id =
            allocator.allocate_framebuffer(&device,
                                           window_size,
                                           TextureFormat::RGBA8,
                                           FramebufferTag("IntermediateDest"));

        let texture_metadata_texture_size = vec2i(TEXTURE_METADATA_TEXTURE_WIDTH,
                                                  TEXTURE_METADATA_TEXTURE_HEIGHT);
        let texture_metadata_texture_id =
            allocator.allocate_texture(&device,
                                       texture_metadata_texture_size,
                                       TextureFormat::RGBA16F,
                                       TextureTag("TextureMetadata"));

        let core_programs = ProgramsCore::new(&device, resources);
        let core_vertex_arrays =
             VertexArraysCore::new(&device,
                                   &core_programs,
                                   allocator.get_general_buffer(quad_vertex_positions_buffer_id),
                                   allocator.get_index_buffer(quad_vertex_indices_buffer_id));

        let mut core = RendererCore {
            device,
            allocator,
            mode,
            options,
            stats: RenderStats::default(),
            current_timer: None,
            timer_query_cache: TimerQueryCache::new(),
            renderer_flags: RendererFlags::empty(),

            programs: core_programs,
            vertex_arrays: core_vertex_arrays,

            quad_vertex_positions_buffer_id,
            quad_vertex_indices_buffer_id,
            area_lut_texture_id,
            gamma_lut_texture_id,

            intermediate_dest_framebuffer_id,
            intermediate_dest_framebuffer_size: window_size,

            texture_metadata_texture_id,
            render_targets: vec![],
            render_target_stack: vec![],
            pattern_texture_pages: vec![],
            mask_storage: None,
            alpha_tile_count: 0,
            framebuffer_flags: FramebufferFlags::empty(),
        };

        let level_impl = match core.mode.level {
            RendererLevel::D3D9 => {
                RendererLevelImpl::D3D9(RendererD3D9::new(&mut core, resources))
            }
            RendererLevel::D3D11 => {
                RendererLevelImpl::D3D11(RendererD3D11::new(&mut core, resources))
            }
        };

        let blit_program = BlitProgram::new(&core.device, resources);
        let clear_program = ClearProgram::new(&core.device, resources);
        let stencil_program = StencilProgram::new(&core.device, resources);
        let reprojection_program = ReprojectionProgram::new(&core.device, resources);

        let debug_ui_presenter = if core.options.show_debug_ui {
            Some(DebugUIPresenter::new(&core.device, resources, window_size, core.mode.level))
        } else {
            None
        };

        let frame = Frame::new(&core.device,
                               &mut core.allocator,
                               &blit_program,
                               &clear_program,
                               &reprojection_program,
                               &stencil_program,
                               quad_vertex_positions_buffer_id,
                               quad_vertex_indices_buffer_id);

        core.device.end_commands();

        Renderer {
            core,
            level_impl,

            blit_program,
            clear_program,

            frame,

            stencil_program,
            reprojection_program,

            current_cpu_build_time: None,
            pending_timers: VecDeque::new(),
            debug_ui_presenter,
            last_stats: VecDeque::new(),
            last_rendering_time: None,
        }
    }

    /// Destroys this renderer and returns the embedded GPU device.
    pub fn destroy(self) -> D {
        self.core.device
    }

    /// Performs work necessary to begin rendering a scene.
    /// 
    /// This must be called before `render_command()`.
    pub fn begin_scene(&mut self) {
        self.core.framebuffer_flags = FramebufferFlags::empty();

        self.core.device.begin_commands();
        self.core.current_timer = Some(PendingTimer::new());
        self.core.stats = RenderStats::default();

        self.core.alpha_tile_count = 0;
    }

    /// Issues a rendering command to the renderer.
    /// 
    /// These commands are generated from methods like `Scene::build()`.
    /// 
    /// `begin_scene()` must have been called first.
    pub fn render_command(&mut self, command: &RenderCommand) {
        debug!("render command: {:?}", command);
        match *command {
            RenderCommand::Start { bounding_quad, path_count, needs_readable_framebuffer } => {
                self.start_rendering(bounding_quad, path_count, needs_readable_framebuffer);
            }
            RenderCommand::AllocateTexturePage { page_id, ref descriptor } => {
                self.allocate_pattern_texture_page(page_id, descriptor)
            }
            RenderCommand::UploadTexelData { ref texels, location } => {
                self.upload_texel_data(texels, location)
            }
            RenderCommand::DeclareRenderTarget { id, location } => {
                self.declare_render_target(id, location)
            }
            RenderCommand::UploadTextureMetadata(ref metadata) => {
                self.upload_texture_metadata(metadata)
            }
            RenderCommand::AddFillsD3D9(ref fills) => {
                self.level_impl.require_d3d9().add_fills(&mut self.core, fills)
            }
            RenderCommand::FlushFillsD3D9 => {
                self.level_impl.require_d3d9().draw_buffered_fills(&mut self.core);
            }
            RenderCommand::UploadSceneD3D11 { ref draw_segments, ref clip_segments } => {
                self.level_impl
                    .require_d3d11()
                    .upload_scene(&mut self.core, draw_segments, clip_segments)
            }
            RenderCommand::PushRenderTarget(render_target_id) => {
                self.push_render_target(render_target_id)
            }
            RenderCommand::PopRenderTarget => self.pop_render_target(),
            RenderCommand::PrepareClipTilesD3D11(ref batch) => {
                self.level_impl.require_d3d11().prepare_tiles(&mut self.core, batch)
            }
            RenderCommand::DrawTilesD3D9(ref batch) => {
                self.level_impl.require_d3d9().upload_and_draw_tiles(&mut self.core, batch)
            }
            RenderCommand::DrawTilesD3D11(ref batch) => {
                self.level_impl.require_d3d11().prepare_and_draw_tiles(&mut self.core, batch)
            }
            RenderCommand::Finish { cpu_build_time } => {
                self.core.stats.cpu_build_time = cpu_build_time;
            }
        }
    }

    /// Finishes rendering a scene.
    /// 
    /// `begin_scene()` and all `render_command()` calls must have been issued before calling this
    /// method.
    /// 
    /// Note that, after calling this method, you might need to flush the output to the screen via
    /// `swap_buffers()`, `present()`, or a similar method that your windowing library offers.
    pub fn end_scene(&mut self) {
        self.clear_dest_framebuffer_if_necessary();
        self.blit_intermediate_dest_framebuffer_if_necessary();

        self.core.stats.gpu_bytes_allocated = self.core.allocator.bytes_allocated();
        self.core.stats.gpu_bytes_committed = self.core.allocator.bytes_committed();

        match self.level_impl {
            RendererLevelImpl::D3D9(_) => {}
            RendererLevelImpl::D3D11(ref mut d3d11_renderer) => {
                d3d11_renderer.end_frame(&mut self.core)
            }
        }

        if let Some(timer) = self.core.current_timer.take() {
            self.pending_timers.push_back(timer);
        }
        self.current_cpu_build_time = None;

        self.update_debug_ui();
        if self.core.options.show_debug_ui {
            self.draw_debug_ui();
        }

        self.core.allocator.purge_if_needed();

        self.core.device.end_commands();
    }

    fn start_rendering(&mut self,
                       bounding_quad: BoundingQuad,
                       path_count: usize,
                       needs_readable_framebuffer: bool) {
        match (&self.core.options.dest, self.core.mode.level) {
            (&DestFramebuffer::Other(_), _) => {
                self.core
                    .renderer_flags
                    .remove(RendererFlags::INTERMEDIATE_DEST_FRAMEBUFFER_NEEDED);
            }
            (&DestFramebuffer::Default { .. }, RendererLevel::D3D11) => {
                self.core
                    .renderer_flags
                    .insert(RendererFlags::INTERMEDIATE_DEST_FRAMEBUFFER_NEEDED);
            }
            _ => {
                self.core
                    .renderer_flags
                    .set(RendererFlags::INTERMEDIATE_DEST_FRAMEBUFFER_NEEDED,
                         needs_readable_framebuffer);
            }
        }

        if self.core.renderer_flags.contains(RendererFlags::USE_DEPTH) {
            self.draw_stencil(&bounding_quad);
        }

        self.core.stats.path_count = path_count;

        self.core.render_targets.clear();
    }

    fn update_debug_ui(&mut self) {
        self.last_stats.push_back(self.core.stats);
        self.shift_rendering_time();

        if !self.core.options.show_debug_ui || self.debug_ui_presenter.is_none() {
            return;
        }

        if let Some(last_rendering_time) = self.last_rendering_time {
            self.debug_ui_presenter
                .as_mut()
                .unwrap()
                .add_sample(self.last_stats.pop_front().unwrap(), last_rendering_time);
        }
    }

    fn draw_debug_ui(&mut self) {
        if let Some(ref mut debug_ui_presenter) = self.debug_ui_presenter {
            let window_size = self.core.options.dest.window_size(&self.core.device);
            debug_ui_presenter.set_framebuffer_size(window_size);
            debug_ui_presenter.draw(&self.core.device, &mut self.core.allocator);
        }
    }

    fn shift_rendering_time(&mut self) {
        if let Some(mut pending_timer) = self.pending_timers.pop_front() {
            for old_query in pending_timer.poll(&self.core.device) {
                self.core.timer_query_cache.free(old_query);
            }
            if let Some(render_time) = pending_timer.total_time() {
                self.last_rendering_time = Some(render_time);
                return;
            }
            self.pending_timers.push_front(pending_timer);
        }
        self.last_rendering_time = None;
    }

    /// Returns GPU timing information for the last frame, if present.
    pub fn last_rendering_time(&self) -> Option<RenderTime> {
        self.last_rendering_time
    }

    /// Returns a reference to the GPU device.
    /// 
    /// This can be useful to issue GPU commands manually via the low-level `pathfinder_gpu`
    /// abstraction. (Of course, you can also use your platform API such as OpenGL directly
    /// alongside Pathfinder.)
    #[inline]
    pub fn device(&self) -> &D {
        &self.core.device
    }

    /// Returns a mutable reference to the GPU device.
    /// 
    /// This can be useful to issue GPU commands manually via the low-level `pathfinder_gpu`
    /// abstraction. (Of course, you can also use your platform API such as OpenGL directly
    /// alongside Pathfinder.)
    #[inline]
    pub fn device_mut(&mut self) -> &mut D {
        &mut self.core.device
    }

    /// Returns the `RendererMode` this renderer was created with.
    #[inline]
    pub fn mode(&self) -> &RendererMode {
        &self.core.mode
    }

    /// Returns the current rendering options.
    #[inline]
    pub fn options(&self) -> &RendererOptions<D> {
        &self.core.options
    }

    /// Returns a mutable reference to the current rendering options, allowing them to be changed.
    /// 
    /// Among other things, you can use this function to change the destination of rendering
    /// output without having to recreate the renderer.
    /// 
    /// After changing the destination framebuffer size, you must call
    /// `dest_framebuffer_size_changed()`.
    pub fn options_mut(&mut self) -> &mut RendererOptions<D> {
        &mut self.core.options
    }

    /// Notifies Pathfinder that the size of the output framebuffer has changed.
    /// 
    /// You must call this function after changing the `dest_framebuffer` member of
    /// `RendererOptions` to a target with a different size.
    #[inline]
    pub fn dest_framebuffer_size_changed(&mut self) {
        let new_framebuffer_size = self.core.main_viewport().size();
        if let Some(ref mut debug_ui_presenter) = self.debug_ui_presenter {
            debug_ui_presenter.ui_presenter.set_framebuffer_size(new_framebuffer_size);
        }
    }

    /// Returns a mutable reference to the debug UI.
    /// 
    /// You can use this function to draw custom debug widgets on screen, as the demo does.
    #[inline]
    pub fn debug_ui_presenter_mut(&mut self) -> DebugUIPresenterInfo<D> {
        DebugUIPresenterInfo {
            device: &mut self.core.device,
            allocator: &mut self.core.allocator,
            debug_ui_presenter: self.debug_ui_presenter.as_mut().expect("Debug UI disabled!"),
        }
    }

    /// Turns off Pathfinder's use of the depth buffer.
    #[inline]
    #[deprecated]
    pub fn disable_depth(&mut self) {
        self.core.renderer_flags.remove(RendererFlags::USE_DEPTH);
    }

    /// Turns on Pathfinder's use of the depth buffer.
    #[inline]
    #[deprecated]
    pub fn enable_depth(&mut self) {
        self.core.renderer_flags.insert(RendererFlags::USE_DEPTH);
    }

    /// Returns various GPU-side statistics about rendering, averaged over the last few frames.
    #[inline]
    pub fn stats(&self) -> &RenderStats {
        &self.core.stats
    }

    /// Returns a GPU-side vertex buffer containing 2D vertices of a unit square.
    /// 
    /// This can be handy for custom rendering.
    #[inline]
    pub fn quad_vertex_positions_buffer(&self) -> &D::Buffer {
        self.core.allocator.get_general_buffer(self.core.quad_vertex_positions_buffer_id)
    }

    /// Returns a GPU-side 32-bit unsigned index buffer of triangles necessary to render a quad
    /// with the buffer returned by `quad_vertex_positions_buffer()`.
    /// 
    /// This can be handy for custom rendering.
    #[inline]
    pub fn quad_vertex_indices_buffer(&self) -> &D::Buffer {
        self.core.allocator.get_index_buffer(self.core.quad_vertex_indices_buffer_id)
    }

    fn allocate_pattern_texture_page(&mut self,
                                     page_id: TexturePageId,
                                     descriptor: &TexturePageDescriptor) {
        // Fill in IDs up to the requested page ID.
        let page_index = page_id.0 as usize;
        while self.core.pattern_texture_pages.len() < page_index + 1 {
            self.core.pattern_texture_pages.push(None);
        }

        // Clear out any existing texture.
        if let Some(old_texture_page) = self.core.pattern_texture_pages[page_index].take() {
            self.core.allocator.free_framebuffer(old_texture_page.framebuffer_id);
        }

        // Allocate texture.
        let texture_size = descriptor.size;
        let framebuffer_id = self.core
                                 .allocator
                                 .allocate_framebuffer(&self.core.device,
                                                       texture_size,
                                                       TextureFormat::RGBA8,
                                                       FramebufferTag("PatternPage"));
        self.core.pattern_texture_pages[page_index] = Some(PatternTexturePage {
            framebuffer_id,
            must_preserve_contents: false,
        });
    }

    fn upload_texel_data(&mut self, texels: &[ColorU], location: TextureLocation) {
        let texture_page = self.core
                               .pattern_texture_pages[location.page.0 as usize]
                               .as_mut()
                               .expect("Texture page not allocated yet!");
        let framebuffer_id = texture_page.framebuffer_id;
        let framebuffer = self.core.allocator.get_framebuffer(framebuffer_id);
        let texture = self.core.device.framebuffer_texture(framebuffer);
        let texels = color::color_slice_to_u8_slice(texels);
        self.core.device.upload_to_texture(texture, location.rect, TextureDataRef::U8(texels));
        texture_page.must_preserve_contents = true;
    }

    fn declare_render_target(&mut self,
                             render_target_id: RenderTargetId,
                             location: TextureLocation) {
        while self.core.render_targets.len() < render_target_id.render_target as usize + 1 {
            self.core.render_targets.push(RenderTargetInfo {
                location: TextureLocation { page: TexturePageId(!0), rect: RectI::default() },
            });
        }
        let mut render_target =
            &mut self.core.render_targets[render_target_id.render_target as usize];
        debug_assert_eq!(render_target.location.page, TexturePageId(!0));
        render_target.location = location;
    }

    fn upload_texture_metadata(&mut self, metadata: &[TextureMetadataEntry]) {
        let padded_texel_size =
            (util::alignup_i32(metadata.len() as i32, TEXTURE_METADATA_ENTRIES_PER_ROW) *
             TEXTURE_METADATA_TEXTURE_WIDTH * 4) as usize;
        let mut texels = Vec::with_capacity(padded_texel_size);
        for entry in metadata {
            let base_color = entry.base_color.to_f32();
            let filter_params = self.compute_filter_params(&entry.filter,
                                                           entry.blend_mode,
                                                           entry.color_0_combine_mode);
            texels.extend_from_slice(&[
                // 0
                f16::from_f32(entry.color_0_transform.m11()),
                f16::from_f32(entry.color_0_transform.m21()),
                f16::from_f32(entry.color_0_transform.m12()),
                f16::from_f32(entry.color_0_transform.m22()),
                // 1
                f16::from_f32(entry.color_0_transform.m13()),
                f16::from_f32(entry.color_0_transform.m23()),
                f16::default(),
                f16::default(),
                // 2
                f16::from_f32(base_color.r()),
                f16::from_f32(base_color.g()),
                f16::from_f32(base_color.b()),
                f16::from_f32(base_color.a()),
                // 3
                f16::from_f32(filter_params.p0.x()),
                f16::from_f32(filter_params.p0.y()),
                f16::from_f32(filter_params.p0.z()),
                f16::from_f32(filter_params.p0.w()),
                // 4
                f16::from_f32(filter_params.p1.x()),
                f16::from_f32(filter_params.p1.y()),
                f16::from_f32(filter_params.p1.z()),
                f16::from_f32(filter_params.p1.w()),
                // 5
                f16::from_f32(filter_params.p2.x()),
                f16::from_f32(filter_params.p2.y()),
                f16::from_f32(filter_params.p2.z()),
                f16::from_f32(filter_params.p2.w()),
                // 6
                f16::from_f32(filter_params.p3.x()),
                f16::from_f32(filter_params.p3.y()),
                f16::from_f32(filter_params.p3.z()),
                f16::from_f32(filter_params.p3.w()),
                // 7
                f16::from_f32(filter_params.p4.x()),
                f16::from_f32(filter_params.p4.y()),
                f16::from_f32(filter_params.p4.z()),
                f16::from_f32(filter_params.p4.w()),
                // 8
                f16::from_f32(filter_params.ctrl as f32),
                f16::default(),
                f16::default(),
                f16::default(),
                // 9
                f16::default(),
                f16::default(),
                f16::default(),
                f16::default(),
            ]);
        }
        while texels.len() < padded_texel_size {
            texels.push(f16::default())
        }

        let texture_id = self.core.texture_metadata_texture_id;
        let texture = self.core.allocator.get_texture(texture_id);
        let width = TEXTURE_METADATA_TEXTURE_WIDTH;
        let height = texels.len() as i32 / (4 * TEXTURE_METADATA_TEXTURE_WIDTH);
        let rect = RectI::new(Vector2I::zero(), Vector2I::new(width, height));
        self.core.device.upload_to_texture(texture, rect, TextureDataRef::F16(&texels));
    }

    fn draw_stencil(&mut self, quad_positions: &[Vector4F]) {
        self.core.device.allocate_buffer(&self.frame.stencil_vertex_array.vertex_buffer,
                                         BufferData::Memory(quad_positions),
                                         BufferTarget::Vertex);

        // Create indices for a triangle fan. (This is OK because the clipped quad should always be
        // convex.)
        let mut indices: Vec<u32> = vec![];
        for index in 1..(quad_positions.len() as u32 - 1) {
            indices.extend_from_slice(&[0, index as u32, index + 1]);
        }
        self.core.device.allocate_buffer(&self.frame.stencil_vertex_array.index_buffer,
                                    BufferData::Memory(&indices),
                                    BufferTarget::Index);

        self.core.device.draw_elements(indices.len() as u32, &RenderState {
            target: &self.core.draw_render_target(),
            program: &self.stencil_program.program,
            vertex_array: &self.frame.stencil_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &[],
            images: &[],
            storage_buffers: &[],
            uniforms: &[],
            viewport: self.core.draw_viewport(),
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

        self.core.stats.drawcall_count += 1;
    }


    /// Draws a texture that was originally drawn with `old_transform` with `new_transform` by
    /// transforming in screen space.
    #[deprecated]
    pub fn reproject_texture(&mut self,
                             texture: &D::Texture,
                             old_transform: &Transform4F,
                             new_transform: &Transform4F) {
        let clear_color = self.core.clear_color_for_draw_operation();

        self.core.device.draw_elements(6, &RenderState {
            target: &self.core.draw_render_target(),
            program: &self.reprojection_program.program,
            vertex_array: &self.frame.reprojection_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &[(&self.reprojection_program.texture, texture)],
            images: &[],
            storage_buffers: &[],
            uniforms: &[
                (&self.reprojection_program.old_transform_uniform,
                 UniformData::from_transform_3d(old_transform)),
                (&self.reprojection_program.new_transform_uniform,
                 UniformData::from_transform_3d(new_transform)),
            ],
            viewport: self.core.draw_viewport(),
            options: RenderOptions {
                blend: BlendMode::SrcOver.to_blend_state(),
                depth: Some(DepthState { func: DepthFunc::Less, write: false, }),
                clear_ops: ClearOps { color: clear_color, ..ClearOps::default() },
                ..RenderOptions::default()
            },
        });

        self.core.stats.drawcall_count += 1;

        self.core.preserve_draw_framebuffer();
    }

    fn push_render_target(&mut self, render_target_id: RenderTargetId) {
        self.core.render_target_stack.push(render_target_id);
    }

    fn pop_render_target(&mut self) {
        self.core.render_target_stack.pop().expect("Render target stack underflow!");
    }

    fn clear_dest_framebuffer_if_necessary(&mut self) {
        let background_color = match self.core.options.background_color {
            None => return,
            Some(background_color) => background_color,
        };

        if self.core.framebuffer_flags.contains(FramebufferFlags::DEST_FRAMEBUFFER_IS_DIRTY) {
            return;
        }

        let main_viewport = self.core.main_viewport();
        let uniforms = [
            (&self.clear_program.rect_uniform, UniformData::Vec4(main_viewport.to_f32().0)),
            (&self.clear_program.framebuffer_size_uniform,
             UniformData::Vec2(main_viewport.size().to_f32().0)),
            (&self.clear_program.color_uniform, UniformData::Vec4(background_color.0)),
        ];

        self.core.device.draw_elements(6, &RenderState {
            target: &RenderTarget::Default,
            program: &self.clear_program.program,
            vertex_array: &self.frame.clear_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &[],
            images: &[],
            storage_buffers: &[],
            uniforms: &uniforms[..],
            viewport: main_viewport,
            options: RenderOptions::default(),
        });

        self.core.stats.drawcall_count += 1;
    }

    fn blit_intermediate_dest_framebuffer_if_necessary(&mut self) {
        if !self.core
                .renderer_flags
                .contains(RendererFlags::INTERMEDIATE_DEST_FRAMEBUFFER_NEEDED) {
            return;
        }

        let main_viewport = self.core.main_viewport();

        if self.core.intermediate_dest_framebuffer_size != main_viewport.size() {
            self.core.allocator.free_framebuffer(self.core.intermediate_dest_framebuffer_id);
            self.core.intermediate_dest_framebuffer_id =
                self.core.allocator.allocate_framebuffer(&self.core.device,
                                                         main_viewport.size(),
                                                         TextureFormat::RGBA8,
                                                         FramebufferTag("IntermediateDest"));
            self.core.intermediate_dest_framebuffer_size = main_viewport.size();
        }

        let intermediate_dest_framebuffer =
            self.core.allocator.get_framebuffer(self.core.intermediate_dest_framebuffer_id);

        let textures = [
            (&self.blit_program.src_texture,
             self.core.device.framebuffer_texture(intermediate_dest_framebuffer))
        ];

        self.core.device.draw_elements(6, &RenderState {
            target: &RenderTarget::Default,
            program: &self.blit_program.program,
            vertex_array: &self.frame.blit_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &textures[..],
            images: &[],
            storage_buffers: &[],
            uniforms: &[
                (&self.blit_program.framebuffer_size_uniform,
                 UniformData::Vec2(main_viewport.size().to_f32().0)),
                (&self.blit_program.dest_rect_uniform,
                 UniformData::Vec4(RectF::new(Vector2F::zero(), main_viewport.size().to_f32()).0)),
            ],
            viewport: main_viewport,
            options: RenderOptions {
                clear_ops: ClearOps {
                    color: Some(ColorF::new(0.0, 0.0, 0.0, 1.0)),
                    ..ClearOps::default()
                },
                ..RenderOptions::default()
            },
        });

        self.core.stats.drawcall_count += 1;
    }

    /// Returns the output viewport in the destination framebuffer, as specified in the render
    /// options.
    #[inline]
    pub fn draw_viewport(&self) -> RectI {
        self.core.draw_viewport()
    }

    /// Returns the destination framebuffer, wrapped in a render target.
    #[inline]
    pub fn draw_render_target(&self) -> RenderTarget<D> {
        self.core.draw_render_target()
    }

    fn compute_filter_params(&self,
                             filter: &Filter,
                             blend_mode: BlendMode,
                             color_0_combine_mode: ColorCombineMode)
                             -> FilterParams {
        let mut ctrl = 0;
        ctrl |= blend_mode.to_composite_ctrl() << COMBINER_CTRL_COMPOSITE_SHIFT;
        ctrl |= color_0_combine_mode.to_composite_ctrl() << COMBINER_CTRL_COLOR_COMBINE_SHIFT;

        match *filter {
            Filter::RadialGradient { line, radii, uv_origin } => {
                FilterParams {
                    p0: line.from().0.concat_xy_xy(line.vector().0),
                    p1: radii.concat_xy_xy(uv_origin.0),
                    p2: F32x4::default(),
                    p3: F32x4::default(),
                    p4: F32x4::default(),
                    ctrl: ctrl | (COMBINER_CTRL_FILTER_RADIAL_GRADIENT <<
                                  COMBINER_CTRL_COLOR_FILTER_SHIFT)
                }
            }
            Filter::PatternFilter(PatternFilter::Blur { sigma, direction }) => {
                let sigma_inv = 1.0 / sigma;
                let gauss_coeff_x = SQRT_2_PI_INV * sigma_inv;
                let gauss_coeff_y = f32::exp(-0.5 * sigma_inv * sigma_inv);
                let gauss_coeff_z = gauss_coeff_y * gauss_coeff_y;

                let src_offset = match direction {
                    BlurDirection::X => vec2f(1.0, 0.0),
                    BlurDirection::Y => vec2f(0.0, 1.0),
                };

                let support = f32::ceil(1.5 * sigma) * 2.0;

                FilterParams {
                    p0: src_offset.0.concat_xy_xy(F32x2::new(support, 0.0)),
                    p1: F32x4::new(gauss_coeff_x, gauss_coeff_y, gauss_coeff_z, 0.0),
                    p2: F32x4::default(),
                    p3: F32x4::default(),
                    p4: F32x4::default(),
                    ctrl: ctrl | (COMBINER_CTRL_FILTER_BLUR << COMBINER_CTRL_COLOR_FILTER_SHIFT),
                }
            }
            Filter::PatternFilter(PatternFilter::Text { 
                fg_color,
                bg_color,
                defringing_kernel,
                gamma_correction,
            }) => {
                let mut p2 = fg_color.0;
                p2.set_w(gamma_correction as i32 as f32);

                FilterParams {
                    p0: match defringing_kernel {
                        Some(ref kernel) => F32x4::from_slice(&kernel.0),
                        None => F32x4::default(),
                    },
                    p1: bg_color.0,
                    p2,
                    p3: F32x4::default(),
                    p4: F32x4::default(),
                    ctrl: ctrl | (COMBINER_CTRL_FILTER_TEXT << COMBINER_CTRL_COLOR_FILTER_SHIFT),
                }
            }
            Filter::PatternFilter(PatternFilter::ColorMatrix(matrix)) => {
                let [p0, p1, p2, p3, p4] = matrix.0;
                FilterParams {
                    p0, p1, p2, p3, p4,
                    ctrl: ctrl | (COMBINER_CTRL_FILTER_COLOR_MATRIX << COMBINER_CTRL_COLOR_FILTER_SHIFT),
                }
            }
            Filter::None => {
                FilterParams {
                    p0: F32x4::default(),
                    p1: F32x4::default(),
                    p2: F32x4::default(),
                    p3: F32x4::default(),
                    p4: F32x4::default(),
                    ctrl,
                }
            }
        }
    }
}

impl<D> RendererCore<D> where D: Device {
    pub(crate) fn mask_texture_format(&self) -> TextureFormat {
        match self.mode.level {
            RendererLevel::D3D9 => TextureFormat::RGBA16F,
            RendererLevel::D3D11 => TextureFormat::RGBA8,
        }
    }

    pub(crate) fn reallocate_alpha_tile_pages_if_necessary(&mut self, copy_existing: bool) {
        let alpha_tile_pages_needed = ((self.alpha_tile_count + 0xffff) >> 16) as u32;
        if let Some(ref mask_storage) = self.mask_storage {
            if alpha_tile_pages_needed <= mask_storage.allocated_page_count {
                return;
            }
        }

        let new_size = vec2i(MASK_FRAMEBUFFER_WIDTH,
                             MASK_FRAMEBUFFER_HEIGHT * alpha_tile_pages_needed as i32);
        let format = self.mask_texture_format();
        let mask_framebuffer_id =
            self.allocator.allocate_framebuffer(&self.device,
                                                new_size,
                                                format,
                                                FramebufferTag("TileAlphaMask"));
        let mask_framebuffer = self.allocator.get_framebuffer(mask_framebuffer_id);
        let old_mask_storage = self.mask_storage.take();
        self.mask_storage = Some(MaskStorage {
            framebuffer_id: mask_framebuffer_id,
            allocated_page_count: alpha_tile_pages_needed,
        });

        // Copy over existing content if needed.
        let old_mask_framebuffer_id = match old_mask_storage {
            Some(old_storage) if copy_existing => old_storage.framebuffer_id,
            Some(_) | None => return,
        };
        let old_mask_framebuffer = self.allocator.get_framebuffer(old_mask_framebuffer_id);
        let old_mask_texture = self.device.framebuffer_texture(old_mask_framebuffer);
        let old_size = self.device.texture_size(old_mask_texture);

        let timer_query = self.timer_query_cache.start_timing_draw_call(&self.device,
                                                                        &self.options);

        self.device.draw_elements(6, &RenderState {
            target: &RenderTarget::Framebuffer(mask_framebuffer),
            program: &self.programs.blit_program.program,
            vertex_array: &self.vertex_arrays.blit_vertex_array.vertex_array,
            primitive: Primitive::Triangles,
            textures: &[(&self.programs.blit_program.src_texture, old_mask_texture)],
            images: &[],
            storage_buffers: &[],
            uniforms: &[
                (&self.programs.blit_program.framebuffer_size_uniform,
                 UniformData::Vec2(new_size.to_f32().0)),
                (&self.programs.blit_program.dest_rect_uniform,
                 UniformData::Vec4(RectF::new(Vector2F::zero(), old_size.to_f32()).0)),
            ],
            viewport: RectI::new(Vector2I::default(), new_size),
            options: RenderOptions {
                clear_ops: ClearOps {
                    color: Some(ColorF::new(0.0, 0.0, 0.0, 0.0)),
                    ..ClearOps::default()
                },
                ..RenderOptions::default()
            },
        });

        self.stats.drawcall_count += 1;
        self.finish_timing_draw_call(&timer_query);
        self.current_timer.as_mut().unwrap().push_query(TimeCategory::Other, timer_query);
    }

    pub(crate) fn set_uniforms_for_drawing_tiles<'a>(
            &'a self,
            tile_program: &'a TileProgramCommon<D>,
            textures: &mut Vec<TextureBinding<'a, D::TextureParameter, D::Texture>>,
            uniforms: &mut Vec<UniformBinding<'a, D::Uniform>>,
            color_texture_0: Option<TileBatchTexture>) {
        let draw_viewport = self.draw_viewport();

        let gamma_lut_texture = self.allocator.get_texture(self.gamma_lut_texture_id);
        textures.push((&tile_program.gamma_lut_texture, gamma_lut_texture));

        let texture_metadata_texture =
            self.allocator.get_texture(self.texture_metadata_texture_id);
        textures.push((&tile_program.texture_metadata_texture, texture_metadata_texture));

        uniforms.push((&tile_program.tile_size_uniform,
                       UniformData::Vec2(F32x2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32))));
        uniforms.push((&tile_program.framebuffer_size_uniform,
                       UniformData::Vec2(draw_viewport.size().to_f32().0)));
        uniforms.push((&tile_program.texture_metadata_size_uniform,
                       UniformData::IVec2(I32x2::new(TEXTURE_METADATA_TEXTURE_WIDTH,
                                                     TEXTURE_METADATA_TEXTURE_HEIGHT))));

        if let Some(ref mask_storage) = self.mask_storage {
            let mask_framebuffer_id = mask_storage.framebuffer_id;
            let mask_framebuffer = self.allocator.get_framebuffer(mask_framebuffer_id);
            let mask_texture = self.device.framebuffer_texture(mask_framebuffer);
            uniforms.push((&tile_program.mask_texture_size_0_uniform,
                           UniformData::Vec2(self.device.texture_size(mask_texture).to_f32().0)));
            textures.push((&tile_program.mask_texture_0, mask_texture));
        }

        match color_texture_0 {
            Some(color_texture) => {
                let color_texture_page = self.texture_page(color_texture.page);
                let color_texture_size = self.device.texture_size(color_texture_page).to_f32();
                self.device.set_texture_sampling_mode(color_texture_page,
                                                      color_texture.sampling_flags);
                textures.push((&tile_program.color_texture_0, color_texture_page));
                uniforms.push((&tile_program.color_texture_size_0_uniform,
                               UniformData::Vec2(color_texture_size.0)));
            }
            None => {
                // Attach any old texture, just to satisfy Metal.
                textures.push((&tile_program.color_texture_0, texture_metadata_texture));
                uniforms.push((&tile_program.color_texture_size_0_uniform,
                               UniformData::Vec2(F32x2::default())));
            }
        }
    }

    // Pattern textures

    fn texture_page(&self, id: TexturePageId) -> &D::Texture {
        self.device.framebuffer_texture(&self.texture_page_framebuffer(id))
    }

    fn texture_page_framebuffer(&self, id: TexturePageId) -> &D::Framebuffer {
        let framebuffer_id = self.pattern_texture_pages[id.0 as usize]
                                 .as_ref()
                                 .expect("Texture page not allocated!")
                                 .framebuffer_id;
        self.allocator.get_framebuffer(framebuffer_id)
    }

    pub(crate) fn clear_color_for_draw_operation(&self) -> Option<ColorF> {
        let must_preserve_contents = match self.render_target_stack.last() {
            Some(&render_target_id) => {
                let texture_page = self.render_target_location(render_target_id).page;
                self.pattern_texture_pages[texture_page.0 as usize]
                    .as_ref()
                    .expect("Draw target texture page not allocated!")
                    .must_preserve_contents
            }
            None => {
                self.framebuffer_flags.contains(FramebufferFlags::DEST_FRAMEBUFFER_IS_DIRTY)
            }
        };

        if must_preserve_contents {
            None
        } else if self.render_target_stack.is_empty() {
            self.options.background_color
        } else {
            Some(ColorF::default())
        }
    }

    // Sizing

    pub(crate) fn tile_size(&self) -> Vector2I {
        let temp = self.draw_viewport().size() +
            vec2i(TILE_WIDTH as i32 - 1, TILE_HEIGHT as i32 - 1);
        vec2i(temp.x() / TILE_WIDTH as i32, temp.y() / TILE_HEIGHT as i32)
    }

    pub(crate) fn framebuffer_tile_size(&self) -> Vector2I {
        pixel_size_to_tile_size(self.options.dest.window_size(&self.device))
    }

    // Viewport calculation

    fn main_viewport(&self) -> RectI {
        match self.options.dest {
            DestFramebuffer::Default { viewport, .. } => viewport,
            DestFramebuffer::Other(ref framebuffer) => {
                let texture = self.device.framebuffer_texture(framebuffer);
                let size = self.device.texture_size(texture);
                RectI::new(Vector2I::default(), size)
            }
        }
    }

    pub(crate) fn draw_viewport(&self) -> RectI {
        match self.render_target_stack.last() {
            Some(&render_target_id) => self.render_target_location(render_target_id).rect,
            None => self.main_viewport(),
        }
    }

    pub(crate) fn draw_render_target(&self) -> RenderTarget<D> {
        match self.render_target_stack.last() {
            Some(&render_target_id) => {
                let texture_page_id = self.render_target_location(render_target_id).page;
                let framebuffer = self.texture_page_framebuffer(texture_page_id);
                RenderTarget::Framebuffer(framebuffer)
            }
            None => {
                if self.renderer_flags
                       .contains(RendererFlags::INTERMEDIATE_DEST_FRAMEBUFFER_NEEDED) {
                    let intermediate_dest_framebuffer =
                        self.allocator.get_framebuffer(self.intermediate_dest_framebuffer_id);
                    RenderTarget::Framebuffer(intermediate_dest_framebuffer)
                } else {
                    match self.options.dest {
                        DestFramebuffer::Default { .. } => RenderTarget::Default,
                        DestFramebuffer::Other(ref framebuffer) => {
                            RenderTarget::Framebuffer(framebuffer)
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn preserve_draw_framebuffer(&mut self) {
        match self.render_target_stack.last() {
            Some(&render_target_id) => {
                let texture_page = self.render_target_location(render_target_id).page;
                self.pattern_texture_pages[texture_page.0 as usize]
                    .as_mut()
                    .expect("Draw target texture page not allocated!")
                    .must_preserve_contents = true;
            }
            None => {
                self.framebuffer_flags.insert(FramebufferFlags::DEST_FRAMEBUFFER_IS_DIRTY);
            }
        }
    }

    fn render_target_location(&self, render_target_id: RenderTargetId) -> TextureLocation {
        self.render_targets[render_target_id.render_target as usize].location
    }

    pub(crate) fn finish_timing_draw_call(&self, timer_query: &Option<D::TimerQuery>) {
        if let Some(ref timer_query) = *timer_query {
            self.device.end_timer_query(timer_query)
        }
    }
}

impl<D> Frame<D> where D: Device {
    // FIXME(pcwalton): This signature shouldn't be so big. Make a struct.
    fn new(device: &D,
           allocator: &mut GPUMemoryAllocator<D>,
           blit_program: &BlitProgram<D>,
           clear_program: &ClearProgram<D>,
           reprojection_program: &ReprojectionProgram<D>,
           stencil_program: &StencilProgram<D>,
           quad_vertex_positions_buffer_id: GeneralBufferID,
           quad_vertex_indices_buffer_id: IndexBufferID)
           -> Frame<D> {
        let quad_vertex_positions_buffer =
            allocator.get_general_buffer(quad_vertex_positions_buffer_id);
        let quad_vertex_indices_buffer =
            allocator.get_index_buffer(quad_vertex_indices_buffer_id);

        let blit_vertex_array = BlitVertexArray::new(device,
                                                     &blit_program,
                                                     &quad_vertex_positions_buffer,
                                                     &quad_vertex_indices_buffer);
        let clear_vertex_array = ClearVertexArray::new(device,
                                                       &clear_program,
                                                       &quad_vertex_positions_buffer,
                                                       &quad_vertex_indices_buffer);
        let reprojection_vertex_array = ReprojectionVertexArray::new(device,
                                                                     &reprojection_program,
                                                                     &quad_vertex_positions_buffer,
                                                                     &quad_vertex_indices_buffer);
        let stencil_vertex_array = StencilVertexArray::new(device, &stencil_program);

        Frame {
            blit_vertex_array,
            clear_vertex_array,
            reprojection_vertex_array,
            stencil_vertex_array,
        }
    }
}

impl<D> RendererLevelImpl<D> where D: Device {
    #[inline]
    fn require_d3d9(&mut self) -> &mut RendererD3D9<D> {
        match *self {
            RendererLevelImpl::D3D9(ref mut d3d9_renderer) => d3d9_renderer,
            RendererLevelImpl::D3D11(_) => {
                panic!("Tried to enter the D3D9 path with a D3D11 renderer!")
            }
        }
    }

    #[inline]
    fn require_d3d11(&mut self) -> &mut RendererD3D11<D> {
        match *self {
            RendererLevelImpl::D3D11(ref mut d3d11_renderer) => d3d11_renderer,
            RendererLevelImpl::D3D9(_) => {
                panic!("Tried to enter the D3D11 path with a D3D9 renderer!")
            }
        }
    }
}

// Render stats

bitflags! {
    pub(crate) struct FramebufferFlags: u8 {
        const MASK_FRAMEBUFFER_IS_DIRTY = 0x01;
        const DEST_FRAMEBUFFER_IS_DIRTY = 0x02;
    }
}

struct RenderTargetInfo {
    location: TextureLocation,
}

bitflags! {
    pub(crate) struct RendererFlags: u8 {
        // Whether we need a depth buffer.
        const USE_DEPTH = 0x01;
        // Whether an intermediate destination framebuffer is needed.
        //
        // This will be true if any exotic blend modes are used at the top level (not inside a
        // render target), *and* the output framebuffer is the default framebuffer.
        const INTERMEDIATE_DEST_FRAMEBUFFER_NEEDED = 0x02;
    }
}

fn pixel_size_to_tile_size(pixel_size: Vector2I) -> Vector2I {
    // Round up.
    let tile_size = vec2i(TILE_WIDTH as i32 - 1, TILE_HEIGHT as i32 - 1);
    let size = pixel_size + tile_size;
    vec2i(size.x() / TILE_WIDTH as i32, size.y() / TILE_HEIGHT as i32)
}

struct FilterParams {
    p0: F32x4,
    p1: F32x4,
    p2: F32x4,
    p3: F32x4,
    p4: F32x4,
    ctrl: i32,
}

pub(crate) struct PatternTexturePage {
    pub(crate) framebuffer_id: FramebufferID,
    pub(crate) must_preserve_contents: bool,
}

/// A mutable reference to the debug UI presenter.
/// 
/// You can use this structure to draw custom debug widgets on screen, as the demo does.
pub struct DebugUIPresenterInfo<'a, D> where D: Device {
    /// The GPU device.
    pub device: &'a mut D,
    /// The GPU memory allocator.
    pub allocator: &'a mut GPUMemoryAllocator<D>,
    /// The debug UI presenter, useful for drawing custom debug widgets on screen.
    pub debug_ui_presenter: &'a mut DebugUIPresenter<D>,
}
