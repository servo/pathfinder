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

use metal::{BufferRef, CommandBufferRef, CommandQueueRef, CompileOptions, CoreAnimationLayerRef};
use metal::{DeviceRef, FunctionRef, LibraryRef, MTLOrigin, MTLPixelFormat, MTLRegion};
use metal::{MTLResourceOptions, MTLSize, MTLStorageMode, MTLTextureType, MTLTextureUsage};
use metal::{MTLVertexFormat, MTLVertexStepFunction, TextureDescriptor, TextureRef};
use metal::{VertexAttributeRef, VertexDescriptor, VertexDescriptorRef};
use pathfinder_geometry::basic::vector::Vector2I;
use pathfinder_gpu::resources::ResourceLoader;
use pathfinder_gpu::{BufferData, BufferTarget, BufferUploadMode, Device, ShaderKind};
use pathfinder_gpu::{TextureFormat, UniformData, VertexAttrClass};
use pathfinder_gpu::{VertexAttrDescriptor, VertexAttrType};
use std::cell::RefCell;
use std::mem;
use std::time::Duration;

const FIRST_VERTEX_BUFFER_INDEX: u32 = 16;

pub struct MetalDevice {
    device: DeviceRef,
    layer: CoreAnimationLayerRef,
    command_queue: CommandQueueRef,
}

pub struct MetalProgram {
    vertex: MetalShader,
    fragment: MetalShader,
    uniforms: RefCell<Vec<UniformBinding>>,
}

struct UniformBinding {
    name: MetalUniform,
    data: UniformData,
}

struct MetalBuffer {
    buffer: RefCell<Option<BufferRef>>,
}

impl MetalDevice {
    #[inline]
    pub fn new(layer: CoreAnimationLayerRef) -> MetalDevice {
        let device = unsafe { DeviceRef::from_ptr(msg_send![layer.as_ptr(), device]) };
        MetalDevice { device, layer }
    }
}

pub struct MetalFramebuffer(TextureRef);

pub struct MetalShader {
    library: LibraryRef,
    function: FunctionRef,
}

// TODO(pcwalton): Use `MTLEvent`s.
pub struct MetalTimerQuery;

// FIXME(pcwalton): This isn't great.
#[derive(Clone)]
pub struct MetalUniform(String);

pub struct MetalVertexArray {
    descriptor: VertexDescriptorRef,
    buffers: Vec<BufferRef>,
}

impl Device for MetalDevice {
    type Buffer = MetalBuffer;
    type CommandBuffer = CommandBufferRef;
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
            if attribute.name() == name {
                return attribute
            }
        }
        panic!("No vertex attribute named `{}` found!", name);
    }

    fn get_uniform(&self, _: &Self::Program, name: &str) -> MetalUniform {
        MetalUniform(name.to_owned())
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

    fn set_uniform(&self, program: &MetalProgram, uniform: &MetalUniform, data: UniformData) {
        program.uniforms.borrow_mut().unwrap().push((*uniform).clone(), data);
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

    fn create_command_buffer(&self) -> CommandBufferRef {
        self.command_queue.new_command_buffer()
    }

    fn submit_command_buffer(&self, command_buffer: CommandBufferRef) {
        command_buffer.commit();
    }

    fn clear(&self, command_buffer: &CommandBufferRef, params: &ClearParams) {
    }

    fn create_timer_query(&self) -> MetalTimerQuery { MetalTimerQuery }
    fn begin_timer_query(&self, _: &MetalTimerQuery) {}
    fn end_timer_query(&self, query: &MetalTimerQuery) {}
    fn timer_query_is_available(&self, query: &MetalTimerQuery) -> bool { true }
    fn get_timer_query(&self, query: &MetalTimerQuery) -> Duration { Duration::from_secs(0) }
}
