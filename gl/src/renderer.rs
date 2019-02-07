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
use crate::device::{Buffer, BufferTarget, BufferUploadMode, Framebuffer, Program, Texture};
use crate::device::{TimerQuery, Uniform, VertexAttr};
use euclid::Size2D;
use gl::types::{GLfloat, GLint, GLuint};
use pathfinder_renderer::gpu_data::{Batch, BuiltScene, SolidTileScenePrimitive};
use pathfinder_renderer::paint::ObjectShader;
use pathfinder_renderer::tiles::{TILE_HEIGHT, TILE_WIDTH};
use std::collections::VecDeque;
use std::time::Duration;

static QUAD_VERTEX_POSITIONS: [u8; 8] = [0, 0, 1, 0, 1, 1, 0, 1];

const MASK_FRAMEBUFFER_WIDTH: u32 = TILE_WIDTH * 256;
const MASK_FRAMEBUFFER_HEIGHT: u32 = TILE_HEIGHT * 256;

// TODO(pcwalton): Replace with `mem::size_of` calls?
const FILL_INSTANCE_SIZE: GLint = 8;
const SOLID_TILE_INSTANCE_SIZE: GLint = 6;
const MASK_TILE_INSTANCE_SIZE: GLint = 8;

const FILL_COLORS_TEXTURE_WIDTH: u32 = 256;
const FILL_COLORS_TEXTURE_HEIGHT: u32 = 256;

pub struct Renderer {
    // Core shaders
    fill_program: FillProgram,
    solid_tile_program: SolidTileProgram,
    mask_tile_program: MaskTileProgram,
    area_lut_texture: Texture,
    #[allow(dead_code)]
    quad_vertex_positions_buffer: Buffer,
    fill_vertex_array: FillVertexArray,
    mask_tile_vertex_array: MaskTileVertexArray,
    solid_tile_vertex_array: SolidTileVertexArray,
    mask_framebuffer: Framebuffer,
    fill_colors_texture: Texture,

    // Postprocessing shader
    postprocess_program: PostprocessProgram,
    postprocess_vertex_array: PostprocessVertexArray,
    gamma_lut_texture: Texture,

    // Debug
    pending_timer_queries: VecDeque<TimerQuery>,
    free_timer_queries: Vec<TimerQuery>,
    pub debug_ui: DebugUI,

    // Extra info
    main_framebuffer_size: Size2D<u32>,
}

impl Renderer {
    pub fn new(main_framebuffer_size: &Size2D<u32>) -> Renderer {
        let fill_program = FillProgram::new();
        let solid_tile_program = SolidTileProgram::new();
        let mask_tile_program = MaskTileProgram::new();

        let postprocess_program = PostprocessProgram::new();

        let area_lut_texture = Texture::from_png("area-lut");
        let gamma_lut_texture = Texture::from_png("gamma-lut");

        let quad_vertex_positions_buffer = Buffer::new();
        quad_vertex_positions_buffer.upload(&QUAD_VERTEX_POSITIONS,
                                            BufferTarget::Vertex,
                                            BufferUploadMode::Static);

        let fill_vertex_array = FillVertexArray::new(&fill_program, &quad_vertex_positions_buffer);
        let mask_tile_vertex_array = MaskTileVertexArray::new(&mask_tile_program,
                                                              &quad_vertex_positions_buffer);
        let solid_tile_vertex_array = SolidTileVertexArray::new(&solid_tile_program,
                                                                &quad_vertex_positions_buffer);

        let postprocess_vertex_array = PostprocessVertexArray::new(&postprocess_program,
                                                                   &quad_vertex_positions_buffer);

        let mask_framebuffer = Framebuffer::new(&Size2D::new(MASK_FRAMEBUFFER_WIDTH,
                                                             MASK_FRAMEBUFFER_HEIGHT));

        let fill_colors_texture = Texture::new_rgba(&Size2D::new(FILL_COLORS_TEXTURE_WIDTH,
                                                                 FILL_COLORS_TEXTURE_HEIGHT));

        let debug_ui = DebugUI::new(main_framebuffer_size);

        Renderer {
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

            postprocess_program,
            postprocess_vertex_array,
            gamma_lut_texture,

            pending_timer_queries: VecDeque::new(),
            free_timer_queries: vec![],

            debug_ui,

            main_framebuffer_size: *main_framebuffer_size,
        }
    }

    pub fn render_scene(&mut self, built_scene: &BuiltScene) {
        let timer_query = self.free_timer_queries.pop().unwrap_or_else(|| TimerQuery::new());
        timer_query.begin();

        self.upload_shaders(&built_scene.shaders);

        self.upload_solid_tiles(&built_scene.solid_tiles);
        self.draw_solid_tiles(&built_scene.solid_tiles);

        for batch in &built_scene.batches {
            self.upload_batch(batch);
            self.draw_batch_fills(batch);
            self.draw_batch_mask_tiles(batch);
        }

        timer_query.end();
        self.pending_timer_queries.push_back(timer_query);
    }

    pub fn shift_timer_query(&mut self) -> Option<Duration> {
        let query = self.pending_timer_queries.front()?;
        if !query.is_available() {
            return None
        }
        let query = self.pending_timer_queries.pop_front().unwrap();
        let result = Duration::from_nanos(query.get());
        self.free_timer_queries.push(query);
        Some(result)
    }

    pub fn set_main_framebuffer_size(&mut self, new_framebuffer_size: &Size2D<u32>) {
        self.main_framebuffer_size = *new_framebuffer_size;
        self.debug_ui.set_framebuffer_size(new_framebuffer_size);
    }

    fn upload_shaders(&mut self, shaders: &[ObjectShader]) {
        let size = Size2D::new(FILL_COLORS_TEXTURE_WIDTH, FILL_COLORS_TEXTURE_HEIGHT);
        let mut fill_colors = vec![0; size.width as usize * size.height as usize * 4];
        for (shader_index, shader) in shaders.iter().enumerate() {
            fill_colors[shader_index * 4 + 0] = shader.fill_color.r;
            fill_colors[shader_index * 4 + 1] = shader.fill_color.g;
            fill_colors[shader_index * 4 + 2] = shader.fill_color.b;
            fill_colors[shader_index * 4 + 3] = shader.fill_color.a;
        }
        self.fill_colors_texture.upload_rgba(&size, &fill_colors);
    }

    fn upload_solid_tiles(&mut self, solid_tiles: &[SolidTileScenePrimitive]) {
        self.solid_tile_vertex_array
            .vertex_buffer
            .upload(solid_tiles, BufferTarget::Vertex, BufferUploadMode::Dynamic);
    }

    fn upload_batch(&mut self, batch: &Batch) {
        self.fill_vertex_array
            .vertex_buffer
            .upload(&batch.fills, BufferTarget::Vertex, BufferUploadMode::Dynamic);
        self.mask_tile_vertex_array
            .vertex_buffer
            .upload(&batch.mask_tiles, BufferTarget::Vertex, BufferUploadMode::Dynamic);
    }

    fn draw_batch_fills(&mut self, batch: &Batch) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.mask_framebuffer.gl_framebuffer);
            gl::Viewport(0, 0, MASK_FRAMEBUFFER_WIDTH as GLint, MASK_FRAMEBUFFER_HEIGHT as GLint);
            // TODO(pcwalton): Only clear the appropriate portion?
            gl::ClearColor(0.0, 0.0, 0.0, 0.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            gl::BindVertexArray(self.fill_vertex_array.gl_vertex_array);
            gl::UseProgram(self.fill_program.program.gl_program);
            gl::Uniform2f(self.fill_program.framebuffer_size_uniform.location,
                          MASK_FRAMEBUFFER_WIDTH as GLfloat,
                          MASK_FRAMEBUFFER_HEIGHT as GLfloat);
            gl::Uniform2f(self.fill_program.tile_size_uniform.location,
                          TILE_WIDTH as GLfloat,
                          TILE_HEIGHT as GLfloat);
            self.area_lut_texture.bind(0);
            gl::Uniform1i(self.fill_program.area_lut_uniform.location, 0);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::ONE, gl::ONE);
            gl::Enable(gl::BLEND);
            gl::DrawArraysInstanced(gl::TRIANGLE_FAN, 0, 4, batch.fills.len() as GLint);
            gl::Disable(gl::BLEND);
        }
    }

    fn draw_batch_mask_tiles(&mut self, batch: &Batch) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::Viewport(0,
                         0,
                         self.main_framebuffer_size.width as GLint,
                         self.main_framebuffer_size.height as GLint);

            gl::BindVertexArray(self.mask_tile_vertex_array.gl_vertex_array);
            gl::UseProgram(self.mask_tile_program.program.gl_program);
            gl::Uniform2f(self.mask_tile_program.framebuffer_size_uniform.location,
                          self.main_framebuffer_size.width as GLfloat,
                          self.main_framebuffer_size.height as GLfloat);
            gl::Uniform2f(self.mask_tile_program.tile_size_uniform.location,
                          TILE_WIDTH as GLfloat,
                          TILE_HEIGHT as GLfloat);
            self.mask_framebuffer.texture.bind(0);
            gl::Uniform1i(self.mask_tile_program.stencil_texture_uniform.location, 0);
            gl::Uniform2f(self.mask_tile_program.stencil_texture_size_uniform.location,
                          MASK_FRAMEBUFFER_WIDTH as GLfloat,
                          MASK_FRAMEBUFFER_HEIGHT as GLfloat);
            self.fill_colors_texture.bind(1);
            gl::Uniform1i(self.mask_tile_program.fill_colors_texture_uniform.location, 1);
            gl::Uniform2f(self.mask_tile_program.fill_colors_texture_size_uniform.location,
                          FILL_COLORS_TEXTURE_WIDTH as GLfloat,
                          FILL_COLORS_TEXTURE_HEIGHT as GLfloat);
            // FIXME(pcwalton): Fill this in properly!
            gl::Uniform2f(self.mask_tile_program.view_box_origin_uniform.location, 0.0, 0.0);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFuncSeparate(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA, gl::ONE, gl::ONE);
            gl::Enable(gl::BLEND);
            gl::DrawArraysInstanced(gl::TRIANGLE_FAN, 0, 4, batch.mask_tiles.len() as GLint);
            gl::Disable(gl::BLEND);
        }
    }

    fn draw_solid_tiles(&mut self, solid_tiles: &[SolidTileScenePrimitive]) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::Viewport(0,
                         0,
                         self.main_framebuffer_size.width as GLint,
                         self.main_framebuffer_size.height as GLint);

            gl::BindVertexArray(self.solid_tile_vertex_array.gl_vertex_array);
            gl::UseProgram(self.solid_tile_program.program.gl_program);
            gl::Uniform2f(self.solid_tile_program.framebuffer_size_uniform.location,
                          self.main_framebuffer_size.width as GLfloat,
                          self.main_framebuffer_size.height as GLfloat);
            gl::Uniform2f(self.solid_tile_program.tile_size_uniform.location,
                          TILE_WIDTH as GLfloat,
                          TILE_HEIGHT as GLfloat);
            self.fill_colors_texture.bind(0);
            gl::Uniform1i(self.solid_tile_program.fill_colors_texture_uniform.location, 0);
            gl::Uniform2f(self.solid_tile_program.fill_colors_texture_size_uniform.location,
                          FILL_COLORS_TEXTURE_WIDTH as GLfloat,
                          FILL_COLORS_TEXTURE_HEIGHT as GLfloat);
            // FIXME(pcwalton): Fill this in properly!
            gl::Uniform2f(self.solid_tile_program.view_box_origin_uniform.location, 0.0, 0.0);
            gl::Disable(gl::BLEND);
            gl::DrawArraysInstanced(gl::TRIANGLE_FAN, 0, 4, solid_tiles.len() as GLint);
        }
    }
}

struct FillVertexArray {
    gl_vertex_array: GLuint,
    vertex_buffer: Buffer,
}

impl FillVertexArray {
    fn new(fill_program: &FillProgram, quad_vertex_positions_buffer: &Buffer) -> FillVertexArray {
        let vertex_buffer = Buffer::new();
        let mut gl_vertex_array = 0;
        unsafe {
            let tess_coord_attr = VertexAttr::new(&fill_program.program, "TessCoord");
            let from_px_attr = VertexAttr::new(&fill_program.program, "FromPx");
            let to_px_attr = VertexAttr::new(&fill_program.program, "ToPx");
            let from_subpx_attr = VertexAttr::new(&fill_program.program, "FromSubpx");
            let to_subpx_attr = VertexAttr::new(&fill_program.program, "ToSubpx");
            let tile_index_attr = VertexAttr::new(&fill_program.program, "TileIndex");

            gl::GenVertexArrays(1, &mut gl_vertex_array);
            gl::BindVertexArray(gl_vertex_array);
            gl::UseProgram(fill_program.program.gl_program);
            gl::BindBuffer(gl::ARRAY_BUFFER, quad_vertex_positions_buffer.gl_buffer);
            tess_coord_attr.configure_float(2, gl::UNSIGNED_BYTE, false, 0, 0, 0);
            gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer.gl_buffer);
            from_px_attr.configure_int(1, gl::UNSIGNED_BYTE, FILL_INSTANCE_SIZE, 0, 1);
            to_px_attr.configure_int(1, gl::UNSIGNED_BYTE, FILL_INSTANCE_SIZE, 1, 1);
            from_subpx_attr.configure_float(2, gl::UNSIGNED_BYTE, true, FILL_INSTANCE_SIZE, 2, 1);
            to_subpx_attr.configure_float(2, gl::UNSIGNED_BYTE, true, FILL_INSTANCE_SIZE, 4, 1);
            tile_index_attr.configure_int(1, gl::UNSIGNED_SHORT, FILL_INSTANCE_SIZE, 6, 1);
        }

        FillVertexArray { gl_vertex_array, vertex_buffer }
    }
}

impl Drop for FillVertexArray {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &mut self.gl_vertex_array);
        }
    }
}

struct MaskTileVertexArray {
    gl_vertex_array: GLuint,
    vertex_buffer: Buffer,
}

impl MaskTileVertexArray {
    fn new(mask_tile_program: &MaskTileProgram, quad_vertex_positions_buffer: &Buffer)
           -> MaskTileVertexArray {
        let vertex_buffer = Buffer::new();
        let mut gl_vertex_array = 0;
        unsafe {
            let tess_coord_attr = VertexAttr::new(&mask_tile_program.program, "TessCoord");
            let tile_origin_attr = VertexAttr::new(&mask_tile_program.program, "TileOrigin");
            let backdrop_attr = VertexAttr::new(&mask_tile_program.program, "Backdrop");
            let object_attr = VertexAttr::new(&mask_tile_program.program, "Object");

            // NB: The object must be of type short, not unsigned short, to work around a macOS
            // Radeon driver bug.
            gl::GenVertexArrays(1, &mut gl_vertex_array);
            gl::BindVertexArray(gl_vertex_array);
            gl::UseProgram(mask_tile_program.program.gl_program);
            gl::BindBuffer(gl::ARRAY_BUFFER, quad_vertex_positions_buffer.gl_buffer);
            tess_coord_attr.configure_float(2, gl::UNSIGNED_BYTE, false, 0, 0, 0);
            gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer.gl_buffer);
            tile_origin_attr.configure_float(2, gl::SHORT, false, MASK_TILE_INSTANCE_SIZE, 0, 1);
            backdrop_attr.configure_int(1, gl::SHORT, MASK_TILE_INSTANCE_SIZE, 4, 1);
            object_attr.configure_int(2, gl::SHORT, MASK_TILE_INSTANCE_SIZE, 6, 1);
        }

        MaskTileVertexArray { gl_vertex_array, vertex_buffer }
    }
}

impl Drop for MaskTileVertexArray {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &mut self.gl_vertex_array);
        }
    }
}

struct SolidTileVertexArray {
    gl_vertex_array: GLuint,
    vertex_buffer: Buffer,
}

impl SolidTileVertexArray {
    fn new(solid_tile_program: &SolidTileProgram, quad_vertex_positions_buffer: &Buffer)
           -> SolidTileVertexArray {
        let vertex_buffer = Buffer::new();
        let mut gl_vertex_array = 0;
        unsafe {
            let tess_coord_attr = VertexAttr::new(&solid_tile_program.program, "TessCoord");
            let tile_origin_attr = VertexAttr::new(&solid_tile_program.program, "TileOrigin");
            let object_attr = VertexAttr::new(&solid_tile_program.program, "Object");

            // NB: The object must be of type short, not unsigned short, to work around a macOS
            // Radeon driver bug.
            gl::GenVertexArrays(1, &mut gl_vertex_array);
            gl::BindVertexArray(gl_vertex_array);
            gl::UseProgram(solid_tile_program.program.gl_program);
            gl::BindBuffer(gl::ARRAY_BUFFER, quad_vertex_positions_buffer.gl_buffer);
            tess_coord_attr.configure_float(2, gl::UNSIGNED_BYTE, false, 0, 0, 0);
            gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer.gl_buffer);
            tile_origin_attr.configure_float(2, gl::SHORT, false, SOLID_TILE_INSTANCE_SIZE, 0, 1);
            object_attr.configure_int(1, gl::SHORT, SOLID_TILE_INSTANCE_SIZE, 4, 1);
        }

        SolidTileVertexArray { gl_vertex_array, vertex_buffer }
    }
}

impl Drop for SolidTileVertexArray {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &mut self.gl_vertex_array);
        }
    }
}

struct FillProgram {
    program: Program,
    framebuffer_size_uniform: Uniform,
    tile_size_uniform: Uniform,
    area_lut_uniform: Uniform,
}

impl FillProgram {
    fn new() -> FillProgram {
        let program = Program::new("fill");
        let framebuffer_size_uniform = Uniform::new(&program, "FramebufferSize");
        let tile_size_uniform = Uniform::new(&program, "TileSize");
        let area_lut_uniform = Uniform::new(&program, "AreaLUT");
        FillProgram { program, framebuffer_size_uniform, tile_size_uniform, area_lut_uniform }
    }
}

struct SolidTileProgram {
    program: Program,
    framebuffer_size_uniform: Uniform,
    tile_size_uniform: Uniform,
    fill_colors_texture_uniform: Uniform,
    fill_colors_texture_size_uniform: Uniform,
    view_box_origin_uniform: Uniform,
}

impl SolidTileProgram {
    fn new() -> SolidTileProgram {
        let program = Program::new("solid_tile");
        let framebuffer_size_uniform = Uniform::new(&program, "FramebufferSize");
        let tile_size_uniform = Uniform::new(&program, "TileSize");
        let fill_colors_texture_uniform = Uniform::new(&program, "FillColorsTexture");
        let fill_colors_texture_size_uniform = Uniform::new(&program, "FillColorsTextureSize");
        let view_box_origin_uniform = Uniform::new(&program, "ViewBoxOrigin");
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

struct MaskTileProgram {
    program: Program,
    framebuffer_size_uniform: Uniform,
    tile_size_uniform: Uniform,
    stencil_texture_uniform: Uniform,
    stencil_texture_size_uniform: Uniform,
    fill_colors_texture_uniform: Uniform,
    fill_colors_texture_size_uniform: Uniform,
    view_box_origin_uniform: Uniform,
}

impl MaskTileProgram {
    fn new() -> MaskTileProgram {
        let program = Program::new("mask_tile");
        let framebuffer_size_uniform = Uniform::new(&program, "FramebufferSize");
        let tile_size_uniform = Uniform::new(&program, "TileSize");
        let stencil_texture_uniform = Uniform::new(&program, "StencilTexture");
        let stencil_texture_size_uniform = Uniform::new(&program, "StencilTextureSize");
        let fill_colors_texture_uniform = Uniform::new(&program, "FillColorsTexture");
        let fill_colors_texture_size_uniform = Uniform::new(&program, "FillColorsTextureSize");
        let view_box_origin_uniform = Uniform::new(&program, "ViewBoxOrigin");
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

struct PostprocessProgram {
    program: Program,
    source_uniform: Uniform,
    framebuffer_size_uniform: Uniform,
    kernel_uniform: Uniform,
    gamma_lut_uniform: Uniform,
    bg_color_uniform: Uniform,
}

impl PostprocessProgram {
    fn new() -> PostprocessProgram {
        let program = Program::new("post");
        let source_uniform = Uniform::new(&program, "Source");
        let framebuffer_size_uniform = Uniform::new(&program, "FramebufferSize");
        let kernel_uniform = Uniform::new(&program, "Kernel");
        let gamma_lut_uniform = Uniform::new(&program, "GammaLUT");
        let bg_color_uniform = Uniform::new(&program, "BGColor");
        PostprocessProgram {
            program,
            source_uniform,
            framebuffer_size_uniform,
            kernel_uniform,
            gamma_lut_uniform,
            bg_color_uniform,
        }
    }
}

struct PostprocessVertexArray {
    gl_vertex_array: GLuint,
}

impl PostprocessVertexArray {
    fn new(postprocess_program: &PostprocessProgram, quad_vertex_positions_buffer: &Buffer)
           -> PostprocessVertexArray {
        let mut gl_vertex_array = 0;
        unsafe {
            let position_attr = VertexAttr::new(&postprocess_program.program, "Position");

            gl::GenVertexArrays(1, &mut gl_vertex_array);
            gl::BindVertexArray(gl_vertex_array);
            gl::UseProgram(postprocess_program.program.gl_program);
            gl::BindBuffer(gl::ARRAY_BUFFER, quad_vertex_positions_buffer.gl_buffer);
            position_attr.configure_float(2, gl::UNSIGNED_BYTE, false, 0, 0, 0);
        }

        PostprocessVertexArray { gl_vertex_array }
    }
}

impl Drop for PostprocessVertexArray {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &mut self.gl_vertex_array);
        }
    }
}
