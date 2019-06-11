// pathfinder/gl/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A Metal implementation of the device abstraction, for macOS and iOS.

#[macro_use]
extern crate objc;

use metal::{BufferRef, CommandBufferRef, CommandQueueRef, CompileOptions};
use metal::{CoreAnimationDrawableRef, CoreAnimationLayerRef, DepthStencilDescriptor, DeviceRef, FunctionRef, LibraryRef};
use metal::{MTLClearColor, MTLLoadAction, MTLOrigin, MTLPixelFormat, MTLRegion};
use metal::{MTLResourceOptions, MTLSamplerAddressMode, MTLSamplerMinMagFilter, MTLSize};
use metal::{MTLStorageMode, MTLStoreAction, MTLTextureType};
use metal::{MTLTextureUsage, MTLVertexFormat, MTLVertexStepFunction, RenderPassDescriptor};
use metal::{SamplerDescriptor, SamplerStateRef, TextureDescriptor, TextureRef, VertexAttributeRef};
use metal::{VertexDescriptor, VertexDescriptorRef};
use pathfinder_geometry::basic::vector::Vector2I;
use pathfinder_gpu::resources::ResourceLoader;
use pathfinder_gpu::{BufferData, BufferTarget, BufferUploadMode, ClearParams, Device};
use pathfinder_gpu::{RenderTarget, ShaderKind, TextureFormat, UniformData, UniformType};
use pathfinder_gpu::{VertexAttrClass, VertexAttrDescriptor, VertexAttrType};
use std::cell::RefCell;
use std::mem;
use std::rc::Rc;
use std::time::Duration;

const FIRST_VERTEX_BUFFER_INDEX: u32 = 16;

pub struct MetalDevice {
    device: DeviceRef,
    layer: CoreAnimationLayerRef,
    drawable: CoreAnimationDrawableRef,
    command_queue: CommandQueueRef,
    command_buffer: RefCell<Option<CommandBufferRef>>,
    sampler: SamplerStateRef,
}

pub struct MetalProgram {
    vertex: MetalShader,
    fragment: MetalShader,
}

#[derive(Clone)]
struct MetalBuffer {
    buffer: Rc<RefCell<Option<BufferRef>>>,
}

impl MetalDevice {
    #[inline]
    pub fn new(layer: CoreAnimationLayerRef) -> MetalDevice {
        let device = unsafe { DeviceRef::from_ptr(msg_send![layer.as_ptr(), device]) };

        let drawable = layer.next_drawable().unwrap();
        let command_queue = device.new_command_buffer();

        let sampler_descriptor = SamplerDescriptor::new();
        sampler_descriptor.set_normalized_coordinates(true);
        sampler_descriptor.set_min_filter(MTLSamplerMinMagFilter::Linear);
        sampler_descriptor.set_mag_filter(MTLSamplerMinMagFilter::Linear);
        sampler_descriptor.set_address_mode_s(MTLSamplerAddressMode::ClampToEdge);
        sampler_descriptor.set_address_mode_t(MTLSamplerAddressMode::ClampToEdge);
        let sampler_state = device.new_sampler_state(sampler_descriptor);

        MetalDevice {
            device,
            layer,
            drawable,
            command_queue,
            command_buffer: RefCell::new(None),
            sampler_state,
        }
    }
}

pub struct MetalFramebuffer(TextureRef);

pub struct MetalShader {
    library: LibraryRef,
    function: FunctionRef,
}

// TODO(pcwalton): Use `MTLEvent`s.
pub struct MetalTimerQuery;

#[derive(Clone)]
pub struct MetalUniform {
    vertex_index: Option<u64>,
    fragment_index: Option<u64>,
}

pub struct MetalVertexArray {
    descriptor: VertexDescriptorRef,
    vertex_buffers: RefCell<Vec<MetalBuffer>>,
    index_buffer: RefCell<Option<MetalBuffer>>,
}

impl Device for MetalDevice {
    type Buffer = MetalBuffer;
    type Framebuffer = MetalFramebuffer;
    type Program = MetalProgram;
    type Shader = MetalShader;
    type Texture = TextureRef;
    type TimerQuery = MetalTimerQuery;
    type Uniform = MetalUniform;
    type VertexArray = VertexDescriptorRef;
    type VertexAttr = VertexAttributeRef;

    // TODO: Add texture usage hint.
    fn create_texture(&self, format: TextureFormat, size: Vector2I) -> TextureRef {
        let descriptor = TextureDescriptor::new();
        descriptor.set_texture_type(MTLTextureType::D2);
        match format {
            TextureFormat::R8 => descriptor.set_pixel_format(MTLPixelFormat::R8Unorm),
            TextureFormat::R16F => descriptor.set_pixel_format(MTLPixelFormat::R16Float),
            TextureFormat::RGBA8 => descriptor.set_pixel_format(MTLPixelFormat::RGBA8Unorm),
        }
        descriptor.set_width(size.x());
        descriptor.set_height(size.y());
        descriptor.set_storage_mode(MTLStorageMode::Managed);
        descriptor.set_texture_usage(MTLTextureUsage::ShaderRead | MTLTextureUsage::RenderTarget);
        self.device.new_texture(&descriptor)
    }

    fn create_texture_from_data(&self, size: Vector2I, data: &[u8]) -> TextureRef {
        assert!(data.len() >= size.x() as usize * size.y() as usize);
        let texture = self.create_texture(TextureFormat::R8, size);
        self.upload_to_texture(&texture, size, data);
        texture
    }

    fn create_shader_from_source(&self, name: &str, source: &[u8], _: ShaderKind) -> MetalShader {
        let library = self.device.new_library_with_source(source, &CompileOptions::new()).unwrap();
        let function = library.get_function("main0", None).unwrap();
        MetalShader { library, function }
    }

    fn create_vertex_array(&self) -> VertexDescriptorRef {
        VertexDescriptor::new()
    }

    fn bind_buffer(&self,
                   vertex_array: &VertexDescriptorRef,
                   buffer: &MetalBuffer,
                   target: BufferTarget) {
        match target {
            BufferTarget::Vertex => {
                vertex_array.vertex_buffers.borrow_mut().unwrap().push((*buffer).clone())
            }
            BufferTarget::Index => {
                *vertex_array.index_buffer.borrow_mut().unwrap() = Some((*buffer).clone())
            }
        }
    }

    fn create_program_from_shaders(&self,
                                   _: &dyn ResourceLoader,
                                   _: &str,
                                   vertex_shader: MetalShader,
                                   fragment_shader: MetalShader)
                                   -> MetalProgram {
        MetalProgram { vertex_shader, fragment_shader, uniforms: vec![] }
    }

    fn get_vertex_attr(&self, program: &MetalProgram, name: &str) -> VertexAttributeRef {
        // TODO(pcwalton): Cache the function?
        let attributes = program.vertex.function.vertex_attributes();
        for attribute in attributes {
            let this_name = attribute.name();
            if this_name[0] == b'a' && this_name[1..] == name {
                return attribute
            }
        }
        panic!("No vertex attribute named `{}` found!", name);
    }

    fn get_uniform(&self, program: &Self::Program, name: &str, uniform_type: UniformType)
                   -> MetalUniform {
        let name = format!("u{}", name);
        MetalUniform {
            vertex_index: self.get_uniform_index(&program.vertex, &name),
            fragment_index: self.get_uniform_index(&program.fragment, &name),
        }
    }

    fn configure_vertex_attr(&self,
                             vertex_array: &VertexDescriptorRef,
                             attr: &VertexAttributeRef,
                             descriptor: &VertexAttrDescriptor) {
        let attribute_index = attr.attribute_index();

        let layout = vertex_array.layouts().object_at(attribute_index);
        if descriptor.divisor == 0 {
            layout.set_step_function(MTLVertexStepFunction::PerVertex);
            layout.set_step_rate(1);
        } else {
            layout.set_step_function(MTLVertexStepFunction::PerInstance);
            layout.set_step_rate(descriptor.divisor);
        }
        layout.set_stride(descriptor.stride);

        let attr_info = vertex_array.attributes().object_at(attribute_index);
        match (descriptor.class, descriptor.attr_type, descriptor.size) {
            (VertexAttrClass::Int, VertexAttrType::I8, 2) => MTLVertexFormat::Char2,
            (VertexAttrClass::Int, VertexAttrType::I8, 3) => MTLVertexFormat::Char3,
            (VertexAttrClass::Int, VertexAttrType::I8, 4) => MTLVertexFormat::Char4,
            (VertexAttrClass::FloatNorm, VertexAttrType::U8, 2) => {
                MTLVertexFormat::UChar2Normalized
            }
            (VertexAttrClass::FloatNorm, VertexAttrType::U8, 3) => {
                MTLVertexFormat::UChar3Normalized
            }
            (VertexAttrClass::FloatNorm, VertexAttrType::U8, 4) => {
                MTLVertexFormat::UChar4Normalized
            }
            (VertexAttrClass::FloatNorm, VertexAttrType::I8, 2) => {
                MTLVertexFormat::Char2Normalized
            }
            (VertexAttrClass::FloatNorm, VertexAttrType::I8, 3) => {
                MTLVertexFormat::Char3Normalized
            }
            (VertexAttrClass::FloatNorm, VertexAttrType::I8, 4) => {
                MTLVertexFormat::Char4Normalized
            }
            (VertexAttrClass::Int, VertexAttrType::I16, 2) => MTLVertexFormat::Short2,
            (VertexAttrClass::Int, VertexAttrType::I16, 3) => MTLVertexFormat::Short3,
            (VertexAttrClass::Int, VertexAttrType::I16, 4) => MTLVertexFormat::Short4,
            (VertexAttrClass::FloatNorm, VertexAttrType::U16, 2) => {
                MTLVertexFormat::UShort2Normalized
            }
            (VertexAttrClass::FloatNorm, VertexAttrType::U16, 3) => {
                MTLVertexFormat::UShort3Normalized
            }
            (VertexAttrClass::FloatNorm, VertexAttrType::U16, 4) => {
                MTLVertexFormat::UShort4Normalized
            }
            (VertexAttrClass::FloatNorm, VertexAttrType::I16, 2) => {
                MTLVertexFormat::Short2Normalized
            }
            (VertexAttrClass::FloatNorm, VertexAttrType::I16, 3) => {
                MTLVertexFormat::Short3Normalized
            }
            (VertexAttrClass::FloatNorm, VertexAttrType::I16, 4) => {
                MTLVertexFormat::Short4Normalized
            }
            (VertexAttrClass::Float, VertexAttrType::F32, 1) => MTLVertexFormat::Float,
            (VertexAttrClass::Float, VertexAttrType::F32, 2) => MTLVertexFormat::Float2,
            (VertexAttrClass::Float, VertexAttrType::F32, 3) => MTLVertexFormat::Float3,
            (VertexAttrClass::Float, VertexAttrType::F32, 4) => MTLVertexFormat::Float4,
            (VertexAttrClass::Int, VertexAttrType::I8, 1) => MTLVertexFormat::Char,
            (VertexAttrClass::FloatNorm, VertexAttrType::I8, 1) => MTLVertexFormat::CharNormalized,
            (VertexAttrClass::Int, VertexAttrType::I16, 1) => MTLVertexFormat::Short,
            (VertexAttrClass::FloatNorm, VertexAttrType::U16, 1) => {
                MTLVertexFormat::UShortNormalized
            }
            (VertexAttrClass::FloatNorm, VertexAttrType::I16, 1) => {
                MTLVertexFormat::ShortNormalized
            }
            (_, _, _) => panic!("Unsupported vertex class/type/size combination!"),
        }
        attr_info.set_offset(descriptor.offset);
        attr_info.set_buffer_index(descriptor.buffer_index + FIRST_VERTEX_BUFFER_INDEX);
    }

    fn create_framebuffer(&self, texture: TextureRef) -> MetalFramebuffer {
        MetalFramebuffer(texture)
    }

    fn create_buffer(&self) -> MetalBuffer {
        MetalBuffer { buffer: RefCell::new(None) }
    }

    fn allocate_buffer<T>(&self,
                          buffer: &MetalBuffer,
                          data: BufferData<T>,
                          target: BufferTarget,
                          mode: BufferUploadMode) {
        let mut options = match mode {
            BufferUploadMode::Static => MTLResourceOptions::CPUCacheModeWriteCombined,
            BufferUploadMode::Dynamic => MTLResourceOptions::CPUCacheModeDefaultCache,
        };
        options |= MTLResourceOptions::StorageModeManaged;

        match data {
            BufferData::Uninitialized(size) => {
                let size = (size * mem::size_of::<T>()) as u64;
                let new_buffer = self.device.new_buffer(size, options);
                *buffer.buffer.borrow_mut() = Some(new_buffer);
            }
            BufferData::Memory(slice) => {
                let size = (slice.len() * mem::size_of::<T>()) as u64;
                let new_buffer = self.device.new_buffer_with_data(slice.as_ptr() as *const _,
                                                                  size,
                                                                  options);
                *buffer.buffer.borrow_mut() = Some(new_buffer);
            }
        }
    }

    fn framebuffer_texture<'f>(&self, framebuffer: &'f MetalFramebuffer) -> &'f TextureRef {
        &framebuffer.0
    }

    fn texture_size(&self, texture: &TextureRef) -> Vector2I {
        Vector2I::new(texture.width(), texture.height())
    }

    fn upload_to_texture(&self, texture: &TextureRef, size: Vector2I, data: &[u8]) {
        assert!(data.len() >= size.x() as usize * size.y() as usize);
        let origin = MTLOrigin { x: 0, y: 0, z: 0 };
        let size = MTLSize { width: size.x, height: size.y, depth: 1 };
        texture.replace_region(MTLRegion { origin, size }, size.width, data.as_ptr());
    }

    fn read_pixels_from_default_framebuffer(&self, size: Vector2I) -> Vec<u8> {
        // TODO(pcwalton)
        vec![]
    }

    fn begin_commands(&self) {
        *self.command_buffer.borrow_mut() = Some(self.command_queue.new_command_buffer());
    }

    fn end_commands(&self) {
        self.command_buffer.borrow_mut().take().unwrap().commit();
    }

    fn clear(&self, target: &RenderTarget<Self>, params: &ClearParams) {
        // TODO(pcwalton): Specify rect, depth, and stencil!
        let color = match params.color { Some(color) => color, None => return };
        let render_pass_descriptor = self.create_render_pass_descriptor(target);
        let color_attachment = render_pass_descriptor.color_attachments().object_at(0).unwrap();
        color_attachment.set_clear_color(MTLClearColor::new(color.r, color.g, color.b, color.a));
        color_attachment.set_load_action(MTLLoadAction::Clear);

        self.command_buffer
            .borrow()
            .unwrap()
            .new_render_command_encoder(render_pass_descriptor)
            .end_encoding();
    }

    fn draw_arrays(&self, index_count: u32, render_state: &RenderState<MetalDevice>) {
        let encoder = self.prepare_to_draw(render_state);
        encoder.draw_primitives(render_state.primitive.to_metal_primitive(), 0, index_count);
        encoder.end_encoding();
    }

    fn draw_elements(&self, index_count: u32, render_state: &RenderState<MetalDevice>) {
        let encoder = self.prepare_to_draw(render_state);
        let primitive = render_state.primitive.to_metal_primitive();
        let index_type = MTLIndexType::UInt32;
        let index_buffer = render_state.vertex_array
                                       .index_buffer
                                       .as_ref()
                                       .expect("No index buffer bound to VAO!");
        let index_buffer = index_buffer.buffer.borrow().expect("Index buffer not allocated!");
        encoder.draw_indexed_primitives(primitive, index_count, index_type, index_buffer, 0);
        encoder.end_encoding();
    }

    fn draw_elements_instanced(&self,
                               index_count: u32,
                               instance_count: u32,
                               render_state: &RenderState<MetalDevice>) {
        let encoder = self.prepare_to_draw(render_state);
        let primitive = render_state.primitive.to_metal_primitive();
        let index_type = MTLIndexType::UInt32;
        let index_buffer = render_state.vertex_array
                                       .index_buffer
                                       .as_ref()
                                       .expect("No index buffer bound to VAO!");
        let index_buffer = index_buffer.buffer.borrow().expect("Index buffer not allocated!");
        encoder.draw_indexed_primitives(primitive,
                                        index_count,
                                        index_type,
                                        index_buffer,
                                        0,
                                        instance_count);
        encoder.end_encoding();
    }

    fn create_timer_query(&self) -> MetalTimerQuery { MetalTimerQuery }
    fn begin_timer_query(&self, _: &MetalTimerQuery) {}
    fn end_timer_query(&self, query: &MetalTimerQuery) {}
    fn timer_query_is_available(&self, query: &MetalTimerQuery) -> bool { true }
    fn get_timer_query(&self, query: &MetalTimerQuery) -> Duration { Duration::from_secs(0) }
}

impl MetalDevice {
    fn get_uniform_index(&self, shader: &MetalShader, name: &str) -> Option<u64> {
        // FIXME(pcwalton): Does this work for fragment attributes?
        for vertex_attribute in shader.function.vertex_attributes() {
            if vertex_attribute.name() == name {
                return Some(vertex_attribute.attribute_index)
            }
        }
        None
    }

    fn prepare_to_draw(&self, render_state: &RenderState<MetalDevice>) -> Encoder {
        let render_pass_descriptor = self.create_render_pass_descriptor(render_state.target,
                                                                        MTLLoadAction::Load);

        let encoder = self.command_buffer
                          .borrow()
                          .unwrap()
                          .new_render_command_encoder(render_pass_descriptor);

        let render_pipeline_descriptor = RenderPipelineDescriptor::new();
        render_pipeline_descriptor.set_vertex_function(render_state.program.vertex.function);
        render_pipeline_descriptor.set_fragment_function(render_state.program.fragment.function);
        render_pipeline_descriptor.set_vertex_descriptor(render_state.vertex_array.descriptor);

        for (vertex_buffer_index, vertex_buffer) in render_state.vertex_array   
                                                                .vertex_buffers
                                                                .iter()
                                                                .enumerate() {
            encoder.set_vertex_buffer(vertex_buffer_index + FIRST_VERTEX_BUFFER_INDEX,
                                      &*vertex_buffer.buffer.borrow(),
                                      0);
        }

        self.set_uniforms(&encoder, render_state);

        let pipeline_color_attachment = render_pipeline_descriptor.color_attachments()
                                                                  .object_at(0)
                                                                  .unwrap();
        self.prepare_color_attachment_for_render(&pipeline_color_attachment, render_state);

        let render_pipeline_state =
            self.device.new_render_pipeline_state(render_pipeline_descriptor);
        encoder.set_render_pipeline_state(render_pipeline_state);

        self.set_depth_stencil_state(encoder, render_state);

        encoder
    }

    fn set_uniforms(&self, encoder: &Encoder, render_state: &RenderState) {
        for (uniform, uniform_data) in &render_state.uniforms {
            if let Some(vertex_index) = uniform.vertex_index {
                match uniform_data {
                    UniformData::TextureUnit(unit) => {
                        let texture = render_state.samplers[unit as usize];
                        encoder.set_vertex_texture(texture, vertex_index);
                        encoder.set_vertex_sampler_state(self.sampler, vertex_index);
                    }
                    _ => {
                        let slice = uniform_data.as_bytes();
                        encoder.set_vertex_bytes(slice.as_ptr(), slice.len(), vertex_index);
                    }
                }
            }
            if let Some(vertex_index) = uniform.fragment_index {
                match uniform_data {
                    UniformData::TextureUnit(unit) => {
                        let texture = render_state.samplers[unit as usize];
                        encoder.set_fragment_texture(texture, vertex_index);
                        encoder.set_fragment_sampler_state(self.sampler, fragment_index);
                    }
                    _ => {
                        let slice = uniform_data.as_bytes();
                        encoder.set_fragment_bytes(slice.as_ptr(), slice.len(), fragment_index);
                    }
                }
            }
        }
    }

    fn prepare_color_attachment_for_render(&self,
                                           color_attachment: &RenderPipelineAttachmentDescriptor,
                                           render_state: &RenderState) {
        pipeline_color_attachment.set_pixel_format(render_state.target.pixel_format());

        let blending_enabled = render_state.options.blend != BlendState::Off;
        pipeline_color_attachment.set_blending_enabled(blending_enabled);
        match render_state.options.blend {
            BlendState::Off => {}
            BlendState::RGBOneAlphaOne => {
                pipeline_color_attachment.set_source_rgb_blend_factor(MTLBlendFactor::One);
                pipeline_color_attachment.set_destination_rgb_blend_factor(MTLBlendFactor::One);
                pipeline_color_attachment.set_source_alpha_blend_factor(MTLBlendFactor::One);
                pipeline_color_attachment.set_destination_alpha_blend_factor(MTLBlendFactor::One);
            }
            BlendState::RGBOneAlphaOneMinusSrcAlpha => {
                pipeline_color_attachment.set_source_rgb_blend_factor(MTLBlendFactor::One);
                pipeline_color_attachment.set_destination_rgb_blend_factor(
                    MTLBlendFactor::OneMinusSourceAlpha);
                pipeline_color_attachment.set_source_alpha_blend_factor(MTLBlendFactor::One);
                pipeline_color_attachment.set_destination_alpha_blend_factor(MTLBlendFactor::One);
            }
            BlendState::RGBOneAlphaOneMinusSrcAlpha => {
                pipeline_color_attachment.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
                pipeline_color_attachment.set_destination_rgb_blend_factor(
                    MTLBlendFactor::OneMinusSourceAlpha);
                pipeline_color_attachment.set_source_alpha_blend_factor(MTLBlendFactor::One);
                pipeline_color_attachment.set_destination_alpha_blend_factor(MTLBlendFactor::One);
            }
        }

        if render_state.options.color_mask {
            pipeline_color_attachment.set_write_mask(MTLColorWriteMask::All);
        } else {
            pipeline_color_attachment.set_write_mask(MTLColorWriteMask::None);
        }
    }

    fn create_render_pass_descriptor(&self,
                                     target: &RenderTarget<MetalDevice>,
                                     load_action: MTLLoadAction)
                                     -> RenderPassDescriptor {
        let render_pass_descriptor = RenderPassDescriptor::new();
        let color_attachment = render_pass_descriptor.color_attachments().object_at(0).unwrap();
        match *target {
            RenderTarget::Default { viewport } => {
                // TODO(pcwalton): Use the viewport!
                // TODO(pcwalton): Depth and stencil!
                color_attachment.set_texture(Some(self.drawable.texture()))
            }
            RenderTarget::Framebuffer { texture } => color_attachment.set_texture(Some(texture)),
        }
        color_attachment.set_load_action(load_action);
        color_attachment.set_store_action(MTLStoreAction::Store);
    }

    fn set_depth_stencil_state(&self, encoder: &Encoder, render_state: &RenderState) {
        let depth_stencil_descriptor = DepthStencilDescriptor::new();

        match render_state.options.depth {
            Some(depth_state) => {
                let compare_function = depth_state.func.to_metal_compare_function();
                depth_stencil_descriptor.set_depth_compare_function(compare_function);
                depth_stencil_descriptor.set_depth_write_enabled(depth_state.write);
            }
            None => {
                depth_stencil_descriptor.set_depth_compare_function(MTLCompareFunction::Always);
                depth_stencil_descriptor.set_depth_write_enabled(false);
            }
        }

        match render_state.options.stencil {
            Some(stencil_state) => {
                let stencil_descriptor = StencilDescriptor::new();
                let compare_function = stencil_state.func.to_metal_compare_function();
                let (pass_operation, write_mask) = if stencil_state.write {
                    (MTLStencilOperation::Replace, stencil_state.mask)
                } else {
                    (MTLStencilOperation::Keep, 0)
                };
                stencil_descriptor.set_stencil_compare_function(compare_function);
                stencil_descriptor.set_stencil_failure_operation(MTLCompareFunction::Keep);
                stencil_descriptor.set_depth_failure_operation(MTLCompareFunction::Keep);
                stencil_descriptor.set_depth_stencil_pass_operation(pass_operation);
                stencil_descriptor.set_write_mask(write_mask);
                depth_stencil_descriptor.set_front_face_stencil(Some(stencil_descriptor));
                depth_stencil_descriptor.set_back_face_stencil(Some(stencil_descriptor));
                encoder.set_stencil_reference_value(stencil_state.reference);
            }
            None => {
                depth_stencil_descriptor.set_front_face_stencil(None);
                depth_stencil_descriptor.set_back_face_stencil(None);
            }
        }

        let depth_stencil_state = self.device.new_depth_stencil_state(depth_stencil_descriptor);
        encoder.set_depth_stencil_state(depth_stencil_state);
    }
}

trait DepthFuncExt {
    fn to_metal_compare_function(self) -> MTLCompareFunction;
}

impl DepthFuncExt for DepthFunc {
    fn to_metal_compare_function(self) -> MTLCompareFunction {
        match self {
            DepthFunc::Less => MTLCompareFunction::Less,
            DepthFunc::Always => MTLCompareFunction::Always,
        }
    }
}

trait PrimitiveExt {
    fn to_metal_primitive(self) -> MTLPrimitiveType;
}

impl PrimitiveExt for Primitive {
    fn to_metal_primitive(self) -> MTLPrimitiveType {
        match render_state.primitive {
            Primitive::Triangles => MTLPrimitiveType::Triangle,
            Primitive::Lines => MTLPrimitiveType::Line,
        }
    }
}

trait StencilFuncExt {
    fn to_metal_compare_function(self) -> MTLCompareFunction;
}

impl StencilFuncExt for StencilFunc {
    fn to_metal_compare_function(self) -> MTLCompareFunction {
        match self {
            StencilFunc::Always => MTLCompareFunction::Always,
            StencilFunc::Equal => MTLCompareFunction::Equal,
        }
    }
}
