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
use pathfinder_content::color::ColorF;
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::transform3d::Transform3DF;
use pathfinder_geometry::vector::Vector2I;
use pathfinder_simd::default::{F32x2, F32x4};
use std::time::Duration;

pub mod resources;

pub trait Encoder<D: Device> {
    fn draw_arrays(&mut self, index_count: u32, render_state: &RenderState<D>);
    fn draw_elements(&mut self, index_count: u32, render_state: &RenderState<D>);
    fn draw_elements_instanced(&mut self,
                               index_count: u32,
                               instance_count: u32,
                               render_state: &RenderState<D>);
    fn begin_timer_query(&mut self, query: &D::TimerQuery);
    fn end_timer_query(&mut self, query: &D::TimerQuery);
}

pub trait Device: Sized {
    type Buffer;
    type Framebuffer;
    type Pipeline;
    type Shader;
    type Texture;
    type TimerQuery;
    type Uniform;
    type Encoder: Encoder<Self>;

    fn create_texture(&self, format: TextureFormat, size: Vector2I) -> Self::Texture;
    fn create_texture_from_data(&self, size: Vector2I, data: &[u8]) -> Self::Texture;
    fn create_texture_from_png(
        &self, resources: &dyn ResourceLoader, name: &str
    ) -> (Self::Texture, Vector2I) {
        let data = resources.slurp(&format!("textures/{}.png", name)).unwrap();
        let image = image::load_from_memory_with_format(&data, ImageFormat::PNG)
            .unwrap()
            .to_luma();
        let size = Vector2I::new(image.width() as i32, image.height() as i32);
        (self.create_texture_from_data(size, &image), size)
    }

    fn create_shader(&self, resources: &dyn ResourceLoader, name: &str, kind: ShaderKind)
                     -> Self::Shader;
    fn create_shader_from_source(&self, name: &str, source: &[u8], kind: ShaderKind)
                                 -> Self::Shader;
    fn create_pipeline_from_shaders(
        &self,
        resources: &dyn ResourceLoader,
        name: &str,
        vertex_shader: Self::Shader,
        fragment_shader: Self::Shader,
        options: RenderOptions,
        vertex_buffers: &[VertexBufferDescriptor],
    ) -> Self::Pipeline;
    fn create_pipeline_from_shader_names(
        &self,
        resources: &dyn ResourceLoader,
        program_name: &str,
        vertex_shader_name: &str,
        fragment_shader_name: &str,
        options: RenderOptions,
        vertex_buffers: &[VertexBufferDescriptor],
    ) -> Self::Pipeline {
        let vertex_shader = self.create_shader(resources, vertex_shader_name, ShaderKind::Vertex);
        let fragment_shader =
            self.create_shader(resources, fragment_shader_name, ShaderKind::Fragment);
        self.create_pipeline_from_shaders(
            resources, program_name, vertex_shader, fragment_shader, options, vertex_buffers
        )
    }
    fn create_pipeline(
        &self,
        resources: &dyn ResourceLoader,
        name: &str,
        options: RenderOptions,
        vertex_buffers: &[VertexBufferDescriptor],
    ) -> Self::Pipeline {
        self.create_pipeline_from_shader_names(resources, name, name, name, options, vertex_buffers)
    }

    fn create_framebuffer(&self, texture: Self::Texture) -> Self::Framebuffer;
    fn create_buffer<T>(&self, data: BufferData<T>) -> Self::Buffer;
    fn upload_to_texture(&self, texture: &Self::Texture, size: Vector2I, data: &[u8]);
    fn read_pixels(&self, target: &RenderTarget<Self>, viewport: RectI) -> TextureData;
    fn begin_commands(&self) -> Self::Encoder;
    fn end_commands(&self, encoder: Self::Encoder);
    fn create_timer_query(&self) -> Self::TimerQuery;
    fn get_timer_query(&self, query: &Self::TimerQuery) -> Option<Duration>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextureFormat {
    R8,
    R16F,
    RGBA8,
}

#[derive(Clone, Copy, Debug)]
pub enum VertexAttrType {
    F32,
    I16,
    I8,
    U16,
    U8,
}

#[derive(Clone, Copy, Debug)]
pub enum BufferData<'a, T> {
    Uninitialized(usize),
    Memory(&'a [T]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShaderKind {
    Vertex,
    Fragment,
}

#[derive(Clone, Copy)]
pub enum UniformData {
    Int(i32),
    Mat4([F32x4; 4]),
    Vec2(F32x2),
    Vec4(F32x4),
    TextureUnit(u32),
}

#[derive(Clone, Copy)]
pub enum Primitive {
    Triangles,
    Lines,
}

#[derive(Clone)]
pub struct RenderState<'a, D> where D: Device {
    pub target: &'a RenderTarget<'a, D>,
    pub pipeline: &'a D::Pipeline,
    pub index_buffer: Option<&'a D::Buffer>,
    pub vertex_buffers: &'a [&'a D::Buffer],
    pub primitive: Primitive,
    pub uniforms: &'a [(&'a D::Uniform, UniformData)],
    pub textures: &'a [&'a D::Texture],
    pub viewport: RectI,
}

#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub blend: BlendState,
    pub depth: Option<DepthState>,
    pub stencil: Option<StencilState>,
    pub clear_ops: ClearOps,
    pub color_mask: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ClearOps {
    pub color: Option<ColorF>,
    pub depth: Option<f32>,
    pub stencil: Option<u8>,
}

#[derive(Clone, Copy, Debug)]
pub enum RenderTarget<'a, D> where D: Device {
    Default,
    Framebuffer(&'a D::Framebuffer),
}

#[derive(Clone, Copy, Debug, PartialEq)]
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
}

impl Default for RenderOptions {
    #[inline]
    fn default() -> RenderOptions {
        RenderOptions {
            blend: BlendState::default(),
            depth: None,
            stencil: None,
            clear_ops: ClearOps::default(),
            color_mask: true,
        }
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
        StencilState {
            func: StencilFunc::default(),
            reference: 0,
            mask: !0,
            write: false,
        }
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

#[derive(Clone, Debug)]
pub enum TextureData {
    U8(Vec<u8>),
    U16(Vec<u16>),
}

impl UniformData {
    #[inline]
    pub fn from_transform_3d(transform: &Transform3DF) -> UniformData {
        UniformData::Mat4([transform.c0, transform.c1, transform.c2, transform.c3])
    }
}

#[derive(Clone, Debug)]
pub struct VertexBufferDescriptor {
    pub stride: usize,
    pub divisor: u32,
    pub attributes: Vec<VertexAttrDescriptor>,
}

#[derive(Clone, Copy, Debug)]
pub struct VertexAttrDescriptor {
    pub size: usize,
    pub class: VertexAttrClass,
    pub attr_type: VertexAttrType,
    pub offset: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VertexAttrClass {
    Float,
    FloatNorm,
    Int,
}

impl TextureFormat {
    #[inline]
    pub fn channels(self) -> usize {
        match self {
            TextureFormat::R8 | TextureFormat::R16F => 1,
            TextureFormat::RGBA8 => 4,
        }
    }
}

impl ClearOps {
    #[inline]
    pub fn has_ops(&self) -> bool {
        self.color.is_some() || self.depth.is_some() || self.stencil.is_some()
    }
}
