// pathfinder/gpu/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Minimal abstractions over GPU device capabilities.

use crate::resources::ResourceLoader;
use image::ImageFormat;
use pathfinder_geometry::basic::point::Point2DI32;
use pathfinder_geometry::basic::rect::RectI32;
use pathfinder_simd::default::F32x4;
use rustache::HashBuilder;
use std::time::Duration;

pub mod resources;

pub trait Device {
    type Buffer;
    type Framebuffer;
    type Program;
    type Shader;
    type Texture;
    type TimerQuery;
    type Uniform;
    type VertexArray;
    type VertexAttr;

    fn create_texture(&self, format: TextureFormat, size: Point2DI32) -> Self::Texture;
    fn create_texture_from_data(&self, size: Point2DI32, data: &[u8]) -> Self::Texture;
    fn create_shader_from_source(&self,
                                 name: &str,
                                 source: &[u8],
                                 kind: ShaderKind,
                                 template_input: HashBuilder)
                                 -> Self::Shader;
    fn create_vertex_array(&self) -> Self::VertexArray;
    fn create_program_from_shaders(&self,
                                   name: &str,
                                   vertex_shader: Self::Shader,
                                   fragment_shader: Self::Shader)
                                   -> Self::Program;
    fn get_vertex_attr(&self, program: &Self::Program, name: &str) -> Self::VertexAttr;
    fn get_uniform(&self, program: &Self::Program, name: &str) -> Self::Uniform;
    fn use_program(&self, program: &Self::Program);
    fn configure_float_vertex_attr(&self,
                                   attr: &Self::VertexAttr,
                                   size: usize,
                                   attr_type: VertexAttrType,
                                   normalized: bool,
                                   stride: usize,
                                   offset: usize,
                                   divisor: u32);
    fn configure_int_vertex_attr(&self,
                                 attr: &Self::VertexAttr,
                                 size: usize,
                                 attr_type: VertexAttrType,
                                 stride: usize,
                                 offset: usize,
                                 divisor: u32);
    fn set_uniform(&self, uniform: &Self::Uniform, data: UniformData);
    fn create_framebuffer(&self, texture: Self::Texture) -> Self::Framebuffer;
    fn create_buffer(&self) -> Self::Buffer;
    fn upload_to_buffer<T>(&self,
                           buffer: &Self::Buffer,
                           data: &[T],
                           target: BufferTarget,
                           mode: BufferUploadMode);
    fn framebuffer_texture<'f>(&self, framebuffer: &'f Self::Framebuffer) -> &'f Self::Texture;
    fn texture_size(&self, texture: &Self::Texture) -> Point2DI32;
    fn upload_to_texture(&self, texture: &Self::Texture, size: Point2DI32, data: &[u8]);
    fn read_pixels_from_default_framebuffer(&self, size: Point2DI32) -> Vec<u8>;
    // TODO(pcwalton): Switch to `ColorF`!
    fn clear(&self, color: Option<F32x4>, depth: Option<f32>, stencil: Option<u8>);
    fn draw_arrays(&self, primitive: Primitive, index_count: u32, render_state: &RenderState);
    fn draw_elements(&self, primitive: Primitive, index_count: u32, render_state: &RenderState);
    fn draw_arrays_instanced(&self,
                             primitive: Primitive,
                             index_count: u32,
                             instance_count: u32,
                             render_state: &RenderState);
    fn create_timer_query(&self) -> Self::TimerQuery;
    fn begin_timer_query(&self, query: &Self::TimerQuery);
    fn end_timer_query(&self, query: &Self::TimerQuery);
    fn timer_query_is_available(&self, query: &Self::TimerQuery) -> bool;
    fn get_timer_query(&self, query: &Self::TimerQuery) -> Duration;

    // TODO(pcwalton): Go bindless...
    fn bind_vertex_array(&self, vertex_array: &Self::VertexArray);
    fn bind_buffer(&self, buffer: &Self::Buffer, target: BufferTarget);
    fn bind_default_framebuffer(&self, viewport: RectI32);
    fn bind_framebuffer(&self, framebuffer: &Self::Framebuffer);
    fn bind_texture(&self, texture: &Self::Texture, unit: u32);

    fn create_texture_from_png(&self, resources: &dyn ResourceLoader, name: &str)
                               -> Self::Texture {
        let data = resources.slurp(&format!("textures/{}.png", name)).unwrap();
        let image = image::load_from_memory_with_format(&data, ImageFormat::PNG).unwrap().to_luma();
        let size = Point2DI32::new(image.width() as i32, image.height() as i32);
        self.create_texture_from_data(size, &image)
    }

    fn create_shader(&self, resources: &dyn ResourceLoader, name: &str, kind: ShaderKind)
                     -> Self::Shader {
        let suffix = match kind { ShaderKind::Vertex => 'v', ShaderKind::Fragment => 'f' };
        let source = resources.slurp(&format!("shaders/{}.{}s.glsl", name, suffix)).unwrap();

        let mut load_include_post_convolve = |_| load_shader_include(resources, "post_convolve");
        let mut load_include_post_gamma_correct =
            |_| load_shader_include(resources, "post_gamma_correct");
        let template_input =
            HashBuilder::new().insert_lambda("include_post_convolve",
                                             &mut load_include_post_convolve)
                              .insert_lambda("include_post_gamma_correct",
                                             &mut load_include_post_gamma_correct);

        self.create_shader_from_source(name, &source, kind, template_input)
    }

    fn create_program(&self, resources: &dyn ResourceLoader, name: &str) -> Self::Program {
        let vertex_shader = self.create_shader(resources, name, ShaderKind::Vertex);
        let fragment_shader = self.create_shader(resources, name, ShaderKind::Fragment);
        self.create_program_from_shaders(name, vertex_shader, fragment_shader)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TextureFormat {
    R16F,
    RGBA8,
}

#[derive(Clone, Copy, Debug)]
pub enum VertexAttrType {
    F32,
    I16,
    U16,
    U8,
}

#[derive(Clone, Copy, Debug)]
pub enum BufferTarget {
    Vertex,
    Index,
}

#[derive(Clone, Copy, Debug)]
pub enum BufferUploadMode {
    Static,
    Dynamic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShaderKind {
    Vertex,
    Fragment,
}

#[derive(Clone, Copy)]
pub enum UniformData {
    Mat4([F32x4; 4]),
    Vec2(F32x4),
    Vec4(F32x4),
    TextureUnit(u32),
}

#[derive(Clone, Copy)]
pub enum Primitive {
    Triangles,
    TriangleFan,
    Lines,
}

#[derive(Clone, Debug)]
pub struct RenderState {
    pub blend: BlendState,
    pub depth: Option<DepthState>,
    pub stencil: Option<StencilState>,
    pub color_mask: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum BlendState {
    Off,
    RGBOneAlphaOne,
    RGBOneAlphaOneMinusSrcAlpha,
    RGBSrcAlphaAlphaOneMinusSrcAlpha,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct DepthState {
    pub func: DepthFunc,
    pub write: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum DepthFunc {
    Less,
    Always,
}

#[derive(Clone, Copy, Debug)]
pub struct StencilState {
    pub func: StencilFunc,
    pub reference: u32,
    pub mask: u32,
    pub write: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum StencilFunc {
    Always,
    Equal,
    NotEqual,
}

impl Default for RenderState {
    #[inline]
    fn default() -> RenderState {
        RenderState { blend: BlendState::default(), depth: None, stencil: None, color_mask: true }
    }
}

impl Default for BlendState {
    #[inline]
    fn default() -> BlendState {
        BlendState::Off
    }
}

impl Default for StencilState {
    #[inline]
    fn default() -> StencilState {
        StencilState { func: StencilFunc::default(), reference: 0, mask: !0, write: false }
    }
}

impl Default for DepthFunc {
    #[inline]
    fn default() -> DepthFunc {
        DepthFunc::Less
    }
}

impl Default for StencilFunc {
    #[inline]
    fn default() -> StencilFunc {
        StencilFunc::Always
    }
}

fn load_shader_include(resources: &dyn ResourceLoader, include_name: &str) -> String {
    let resource = resources.slurp(&format!("shaders/{}.inc.glsl", include_name)).unwrap();
    String::from_utf8_lossy(&resource).to_string()
}
