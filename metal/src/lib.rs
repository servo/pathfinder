// pathfinder/metal/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A Metal implementation of the device abstraction, for macOS and iOS.

#![allow(non_upper_case_globals)]

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate objc;

use block::{Block, ConcreteBlock, RcBlock};
use byteorder::{NativeEndian, WriteBytesExt};
use cocoa::foundation::{NSRange, NSUInteger};
use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use foreign_types::{ForeignType, ForeignTypeRef};
use half::f16;
use io_surface::IOSurfaceRef;
use libc::size_t;
use metal::{self, Argument, ArgumentEncoder, Buffer, CommandBuffer, CommandBufferRef};
use metal::{CommandQueue, CompileOptions, ComputeCommandEncoderRef, ComputePipelineDescriptor};
use metal::{ComputePipelineState, CoreAnimationDrawable, CoreAnimationDrawableRef};
use metal::{CoreAnimationLayer, CoreAnimationLayerRef, DepthStencilDescriptor, Function, Library};
use metal::{MTLArgument, MTLArgumentEncoder, MTLArgumentType, MTLBlendFactor, MTLBlendOperation};
use metal::{MTLClearColor, MTLColorWriteMask, MTLCompareFunction, MTLComputePipelineState};
use metal::{MTLDataType, MTLDevice, MTLIndexType, MTLLoadAction, MTLOrigin, MTLPixelFormat};
use metal::{MTLPrimitiveType, MTLRegion, MTLRenderPipelineReflection, MTLRenderPipelineState};
use metal::{MTLResourceOptions, MTLResourceUsage, MTLSamplerAddressMode, MTLSamplerMinMagFilter};
use metal::{MTLSize, MTLStencilOperation, MTLStorageMode, MTLStoreAction, MTLTextureType};
use metal::{MTLTextureUsage, MTLVertexFormat, MTLVertexStepFunction, MTLViewport};
use metal::{RenderCommandEncoder, RenderCommandEncoderRef, RenderPassDescriptor};
use metal::{RenderPassDescriptorRef, RenderPipelineColorAttachmentDescriptorRef};
use metal::{RenderPipelineDescriptor, RenderPipelineReflection, RenderPipelineReflectionRef};
use metal::{RenderPipelineState, SamplerDescriptor, SamplerState, StencilDescriptor};
use metal::{StructMemberRef, StructType, StructTypeRef, TextureDescriptor, Texture, TextureRef};
use metal::{VertexAttribute, VertexAttributeRef, VertexDescriptor, VertexDescriptorRef};
use objc::runtime::{Class, Object};
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::vector::{Vector2I, vec2i};
use pathfinder_gpu::{BlendFactor, BlendOp, BufferData, BufferTarget, BufferUploadMode};
use pathfinder_gpu::{ComputeDimensions, ComputeState, DepthFunc, Device, FeatureLevel};
use pathfinder_gpu::{ImageAccess, Primitive, ProgramKind, RenderState, RenderTarget, ShaderKind};
use pathfinder_gpu::{StencilFunc, TextureData, TextureDataRef, TextureFormat};
use pathfinder_gpu::{TextureSamplingFlags, UniformData, VertexAttrClass};
use pathfinder_gpu::{VertexAttrDescriptor, VertexAttrType};
use pathfinder_resources::ResourceLoader;
use pathfinder_simd::default::{F32x2, F32x4, I32x2};
use std::cell::{Cell, RefCell};
use std::mem;
use std::ops::Range;
use std::ptr;
use std::rc::Rc;
use std::slice;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

const FIRST_VERTEX_BUFFER_INDEX: u64 = 16;

pub struct MetalDevice {
    device: metal::Device,
    main_color_texture: Texture,
    main_depth_stencil_texture: Texture,
    command_queue: CommandQueue,
    command_buffers: RefCell<Vec<CommandBuffer>>,
    samplers: Vec<SamplerState>,
    shared_event: SharedEvent,
    shared_event_listener: SharedEventListener,
    next_timer_query_event_value: Cell<u64>,
}

pub enum MetalProgram {
    Raster(MetalRasterProgram),
    Compute(MetalComputeProgram),
}

pub struct MetalRasterProgram {
    vertex_shader: MetalShader,
    fragment_shader: MetalShader,
}

pub struct MetalComputeProgram {
    shader: MetalShader,
    local_size: MTLSize,
}

#[derive(Clone)]
pub struct MetalBuffer {
    buffer: Rc<RefCell<Option<Buffer>>>,
    mode: BufferUploadMode,
}

impl MetalDevice {
    #[inline]
    pub unsafe fn new<T>(device: metal::Device, texture: T) -> MetalDevice where T: IntoTexture {
        let command_queue = device.new_command_queue();

        let samplers = (0..16).map(|sampling_flags_value| {
            let sampling_flags = TextureSamplingFlags::from_bits(sampling_flags_value).unwrap();
            let sampler_descriptor = SamplerDescriptor::new();
            sampler_descriptor.set_support_argument_buffers(true);
            sampler_descriptor.set_normalized_coordinates(true);
            sampler_descriptor.set_min_filter(
                if sampling_flags.contains(TextureSamplingFlags::NEAREST_MIN) {
                    MTLSamplerMinMagFilter::Nearest
                } else {
                    MTLSamplerMinMagFilter::Linear
                });
            sampler_descriptor.set_mag_filter(
                if sampling_flags.contains(TextureSamplingFlags::NEAREST_MAG) {
                    MTLSamplerMinMagFilter::Nearest
                } else {
                    MTLSamplerMinMagFilter::Linear
                });
            sampler_descriptor.set_address_mode_s(
                if sampling_flags.contains(TextureSamplingFlags::REPEAT_U) {
                    MTLSamplerAddressMode::Repeat
                } else {
                    MTLSamplerAddressMode::ClampToEdge
                });
            sampler_descriptor.set_address_mode_t(
                if sampling_flags.contains(TextureSamplingFlags::REPEAT_V) {
                    MTLSamplerAddressMode::Repeat
                } else {
                    MTLSamplerAddressMode::ClampToEdge
                });
            device.new_sampler(&sampler_descriptor)
        }).collect();

        let texture = texture.into_texture(&device);
        let framebuffer_size = vec2i(texture.width() as i32, texture.height() as i32);
        let main_depth_stencil_texture = device.create_depth_stencil_texture(framebuffer_size);

        let shared_event = device.new_shared_event();

        MetalDevice {
            device,
            main_color_texture: texture,
            main_depth_stencil_texture,
            command_queue,
            command_buffers: RefCell::new(vec![]),
            samplers,
            shared_event,
            shared_event_listener: SharedEventListener::new(),
            next_timer_query_event_value: Cell::new(1),
        }
    }

    #[inline]
    pub fn swap_texture<T>(&mut self, new_texture: T) -> Texture where T: IntoTexture {
        unsafe {
            let new_texture = new_texture.into_texture(&self.device);
            mem::replace(&mut self.main_color_texture, new_texture)
        }
    }

    #[inline]
    pub fn metal_device(&self) -> metal::Device {
        self.device.clone()
    }
}

pub struct MetalFramebuffer(MetalTexture);

pub struct MetalShader {
    #[allow(dead_code)]
    library: Library,
    function: Function,
    #[allow(dead_code)]
    name: String,
    arguments: RefCell<Option<ArgumentArray>>,
}

pub struct MetalTexture {
    texture: Texture,
    sampling_flags: Cell<TextureSamplingFlags>,
    dirty: Cell<bool>,
}

#[derive(Clone)]
pub struct MetalTextureDataReceiver(Arc<MetalTextureDataReceiverInfo>);

struct MetalTextureDataReceiverInfo {
    mutex: Mutex<MetalTextureDataReceiverState>,
    cond: Condvar,
    texture: Texture,
    viewport: RectI,
}

enum MetalTextureDataReceiverState {
    Pending,
    Downloaded(TextureData),
    Finished,
}

#[derive(Clone)]
pub struct MetalTimerQuery(Arc<MetalTimerQueryInfo>);

struct MetalTimerQueryInfo {
    mutex: Mutex<MetalTimerQueryData>,
    cond: Condvar,
    event_value: u64,
}

struct MetalTimerQueryData {
    start_time: Option<Instant>,
    end_time: Option<Instant>,
}

#[derive(Clone)]
pub struct MetalUniform {
    indices: RefCell<Option<MetalUniformIndices>>,
    name: String,
}

#[derive(Clone)]
pub struct MetalTextureParameter {
    indices: RefCell<Option<MetalTextureIndices>>,
    name: String,
}

#[derive(Clone)]
pub struct MetalImageParameter {
    indices: RefCell<Option<MetalImageIndices>>,
    name: String,
}

#[derive(Clone)]
pub struct MetalStorageBuffer {
    indices: RefCell<Option<MetalStorageBufferIndices>>,
    name: String,
}

#[derive(Clone, Copy, Debug)]
pub struct MetalUniformIndices(ProgramKind<Option<MetalUniformIndex>>);

#[derive(Clone, Copy, Debug)]
pub struct MetalUniformIndex(u64);

#[derive(Clone, Copy)]
pub struct MetalTextureIndices(ProgramKind<Option<MetalTextureIndex>>);

#[derive(Clone, Copy, Debug)]
pub struct MetalTextureIndex {
    main: u64,
    sampler: u64,
}

#[derive(Clone, Copy)]
pub struct MetalImageIndices(ProgramKind<Option<MetalImageIndex>>);

#[derive(Clone, Copy, Debug)]
pub struct MetalImageIndex(u64);

#[derive(Clone, Copy)]
pub struct MetalStorageBufferIndices(ProgramKind<Option<MetalStorageBufferIndex>>);

#[derive(Clone, Copy, Debug)]
pub struct MetalStorageBufferIndex(u64);

#[derive(Clone)]
pub struct MetalFence(Arc<MetalFenceInfo>);

struct MetalFenceInfo {
    mutex: Mutex<MetalFenceStatus>,
    cond: Condvar,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum MetalFenceStatus {
    Pending,
    Resolved,
}

pub struct MetalVertexArray {
    descriptor: VertexDescriptor,
    vertex_buffers: RefCell<Vec<MetalBuffer>>,
    index_buffer: RefCell<Option<MetalBuffer>>,
}

impl Device for MetalDevice {
    type Buffer = MetalBuffer;
    type Fence = MetalFence;
    type Framebuffer = MetalFramebuffer;
    type ImageParameter = MetalImageParameter;
    type Program = MetalProgram;
    type Shader = MetalShader;
    type StorageBuffer = MetalStorageBuffer;
    type Texture = MetalTexture;
    type TextureDataReceiver = MetalTextureDataReceiver;
    type TextureParameter = MetalTextureParameter;
    type TimerQuery = MetalTimerQuery;
    type Uniform = MetalUniform;
    type VertexArray = MetalVertexArray;
    type VertexAttr = VertexAttribute;

    #[inline]
    fn feature_level(&self) -> FeatureLevel {
        FeatureLevel::D3D11
    }

    // TODO: Add texture usage hint.
    fn create_texture(&self, format: TextureFormat, size: Vector2I) -> MetalTexture {
        let descriptor = TextureDescriptor::new();
        descriptor.set_texture_type(MTLTextureType::D2);
        match format {
            TextureFormat::R8 => descriptor.set_pixel_format(MTLPixelFormat::R8Unorm),
            TextureFormat::R16F => descriptor.set_pixel_format(MTLPixelFormat::R16Float),
            TextureFormat::RGBA8 => descriptor.set_pixel_format(MTLPixelFormat::RGBA8Unorm),
            TextureFormat::RGBA16F => descriptor.set_pixel_format(MTLPixelFormat::RGBA16Float),
            TextureFormat::RGBA32F => descriptor.set_pixel_format(MTLPixelFormat::RGBA32Float),
        }
        descriptor.set_width(size.x() as u64);
        descriptor.set_height(size.y() as u64);
        descriptor.set_storage_mode(MTLStorageMode::Managed);
        descriptor.set_usage(MTLTextureUsage::Unknown);
        MetalTexture {
            texture: self.device.new_texture(&descriptor),
            sampling_flags: Cell::new(TextureSamplingFlags::empty()),
            dirty: Cell::new(false),
        }
    }

    fn create_texture_from_data(&self, format: TextureFormat, size: Vector2I, data: TextureDataRef)
                                -> MetalTexture {
        let texture = self.create_texture(format, size);
        self.upload_to_texture(&texture, RectI::new(Vector2I::default(), size), data);
        texture
    }

    fn create_shader_from_source(&self, name: &str, source: &[u8], _: ShaderKind) -> MetalShader {
        let source = String::from_utf8(source.to_vec()).expect("Source wasn't valid UTF-8!");

        let compile_options = CompileOptions::new();
        let library = self.device.new_library_with_source(&source, &compile_options).unwrap();
        let function = library.get_function("main0", None).unwrap();

        MetalShader {
            library,
            function,
            name: name.to_owned(),
            arguments: RefCell::new(None),
        }
    }

    fn create_vertex_array(&self) -> MetalVertexArray {
        MetalVertexArray {
            descriptor: VertexDescriptor::new().retain(),
            vertex_buffers: RefCell::new(vec![]),
            index_buffer: RefCell::new(None),
        }
    }

    fn bind_buffer(&self,
                   vertex_array: &MetalVertexArray,
                   buffer: &MetalBuffer,
                   target: BufferTarget) {
        match target {
            BufferTarget::Vertex => {
                vertex_array.vertex_buffers.borrow_mut().push((*buffer).clone())
            }
            BufferTarget::Index => {
                *vertex_array.index_buffer.borrow_mut() = Some((*buffer).clone())
            }
            _ => panic!("Buffers bound to vertex arrays must be vertex or index buffers!"),
        }
    }

    fn create_program_from_shaders(&self,
                                   _: &dyn ResourceLoader,
                                   _: &str,
                                   shaders: ProgramKind<MetalShader>)
                                   -> MetalProgram {
        match shaders {
            ProgramKind::Raster { vertex: vertex_shader, fragment: fragment_shader } => {
                MetalProgram::Raster(MetalRasterProgram { vertex_shader, fragment_shader })
            }
            ProgramKind::Compute(shader) => {
                let local_size = MTLSize { width: 0, height: 0, depth: 0 };
                MetalProgram::Compute(MetalComputeProgram { shader, local_size })
            }
        }
    }

    // FIXME(pcwalton): Is there a way to introspect the shader to find `gl_WorkGroupSize`? That
    // would obviate the need for this function.
    fn set_compute_program_local_size(&self,
                                      program: &mut MetalProgram,
                                      new_local_size: ComputeDimensions) {
        match *program {
            MetalProgram::Compute(MetalComputeProgram { ref mut local_size, .. }) => {
                *local_size = new_local_size.to_metal_size()
            }
            _ => panic!("Program was not a compute program!"),
        }
    }

    fn get_vertex_attr(&self, program: &MetalProgram, name: &str) -> Option<VertexAttribute> {
        // TODO(pcwalton): Cache the function?
        let attributes = match *program {
            MetalProgram::Raster(MetalRasterProgram { ref vertex_shader, .. }) => {
                vertex_shader.function.real_vertex_attributes()
            }
            _ => unreachable!(),
        };
        for attribute_index in 0..attributes.len() {
            let attribute = attributes.object_at(attribute_index);
            let this_name = attribute.name().as_bytes();
            if this_name[0] == b'a' && this_name[1..] == *name.as_bytes() {
                //println!("found attribute: \"{}\"", name);
                return Some(attribute.retain())
            }
        }
        None
    }

    fn get_uniform(&self, _: &Self::Program, name: &str) -> MetalUniform {
        MetalUniform { indices: RefCell::new(None), name: name.to_owned() }
    }

    fn get_texture_parameter(&self, _: &Self::Program, name: &str) -> MetalTextureParameter {
        MetalTextureParameter { indices: RefCell::new(None), name: name.to_owned() }
    }

    fn get_image_parameter(&self, _: &Self::Program, name: &str) -> MetalImageParameter {
        MetalImageParameter { indices: RefCell::new(None), name: name.to_owned() }
    }

    fn get_storage_buffer(&self, _: &Self::Program, name: &str, _: u32) -> MetalStorageBuffer {
        MetalStorageBuffer { indices: RefCell::new(None), name: name.to_owned() }
    }

    fn configure_vertex_attr(&self,
                             vertex_array: &MetalVertexArray,
                             attr: &VertexAttribute,
                             descriptor: &VertexAttrDescriptor) {
        debug_assert_ne!(descriptor.stride, 0);

        let attribute_index = attr.attribute_index();

        let attr_info = vertex_array.descriptor
                                    .attributes()
                                    .object_at(attribute_index as usize)
                                    .unwrap();
        let format = match (descriptor.class, descriptor.attr_type, descriptor.size) {
            (VertexAttrClass::Int, VertexAttrType::I8, 2) => MTLVertexFormat::Char2,
            (VertexAttrClass::Int, VertexAttrType::I8, 3) => MTLVertexFormat::Char3,
            (VertexAttrClass::Int, VertexAttrType::I8, 4) => MTLVertexFormat::Char4,
            (VertexAttrClass::Int, VertexAttrType::U8, 2) => MTLVertexFormat::UChar2,
            (VertexAttrClass::Int, VertexAttrType::U8, 3) => MTLVertexFormat::UChar3,
            (VertexAttrClass::Int, VertexAttrType::U8, 4) => MTLVertexFormat::UChar4,
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
            (VertexAttrClass::Int, VertexAttrType::U16, 2) => MTLVertexFormat::UShort2,
            (VertexAttrClass::Int, VertexAttrType::U16, 3) => MTLVertexFormat::UShort3,
            (VertexAttrClass::Int, VertexAttrType::U16, 4) => MTLVertexFormat::UShort4,
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
            (VertexAttrClass::Int, VertexAttrType::U8, 1) => MTLVertexFormat::UChar,
            (VertexAttrClass::FloatNorm, VertexAttrType::I8, 1) => MTLVertexFormat::CharNormalized,
            (VertexAttrClass::FloatNorm, VertexAttrType::U8, 1) => {
                MTLVertexFormat::UCharNormalized
            }
            (VertexAttrClass::Int, VertexAttrType::I16, 1) => MTLVertexFormat::Short,
            (VertexAttrClass::Int, VertexAttrType::U16, 1) => MTLVertexFormat::UShort,
            (VertexAttrClass::FloatNorm, VertexAttrType::U16, 1) => {
                MTLVertexFormat::UShortNormalized
            }
            (VertexAttrClass::FloatNorm, VertexAttrType::I16, 1) => {
                MTLVertexFormat::ShortNormalized
            }
            (attr_class, attr_type, attr_size) => {
                panic!("Unsupported vertex class/type/size combination: {:?}/{:?}/{}!",
                       attr_class,
                       attr_type,
                       attr_size)
            }
        };
        attr_info.set_format(format);
        attr_info.set_offset(descriptor.offset as u64);
        let buffer_index = descriptor.buffer_index as u64 + FIRST_VERTEX_BUFFER_INDEX;
        attr_info.set_buffer_index(buffer_index);

        // FIXME(pcwalton): Metal separates out per-buffer info from per-vertex info, while our
        // GL-like API does not. So we end up setting this state over and over again. Not great.
        let layout = vertex_array.descriptor.layouts().object_at(buffer_index as usize).unwrap();
        if descriptor.divisor == 0 {
            layout.set_step_function(MTLVertexStepFunction::PerVertex);
            layout.set_step_rate(1);
        } else {
            layout.set_step_function(MTLVertexStepFunction::PerInstance);
            layout.set_step_rate(descriptor.divisor as u64);
        }
        layout.set_stride(descriptor.stride as u64);
    }

    fn create_framebuffer(&self, texture: MetalTexture) -> MetalFramebuffer {
        MetalFramebuffer(texture)
    }

    fn create_buffer(&self, mode: BufferUploadMode) -> MetalBuffer {
        MetalBuffer { buffer: Rc::new(RefCell::new(None)), mode }
    }

    fn allocate_buffer<T>(&self,
                          buffer: &MetalBuffer,
                          data: BufferData<T>,
                          _: BufferTarget) {
        let options = buffer.mode.to_metal_resource_options();
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

    fn upload_to_buffer<T>(&self,
                           buffer: &MetalBuffer,
                           start: usize,
                           data: &[T],
                           _: BufferTarget) {
        let mut buffer = buffer.buffer.borrow_mut();
        let buffer = buffer.as_mut().unwrap();
        self.upload_to_metal_buffer(buffer, start, data)
    }

    #[inline]
    fn framebuffer_texture<'f>(&self, framebuffer: &'f MetalFramebuffer) -> &'f MetalTexture {
        &framebuffer.0
    }

    #[inline]
    fn destroy_framebuffer(&self, framebuffer: MetalFramebuffer) -> MetalTexture {
        framebuffer.0
    }

    fn texture_format(&self, texture: &MetalTexture) -> TextureFormat {
        match texture.texture.pixel_format() {
            MTLPixelFormat::R8Unorm => TextureFormat::R8,
            MTLPixelFormat::R16Float => TextureFormat::R16F,
            MTLPixelFormat::RGBA8Unorm => TextureFormat::RGBA8,
            MTLPixelFormat::RGBA16Float => TextureFormat::RGBA16F,
            MTLPixelFormat::RGBA32Float => TextureFormat::RGBA32F,
            _ => panic!("Unexpected Metal texture format!"),
        }
    }

    fn texture_size(&self, texture: &MetalTexture) -> Vector2I {
        vec2i(texture.texture.width() as i32, texture.texture.height() as i32)
    }

    fn set_texture_sampling_mode(&self, texture: &MetalTexture, flags: TextureSamplingFlags) {
        texture.sampling_flags.set(flags)
    }

    fn upload_to_texture(&self, texture: &MetalTexture, rect: RectI, data: TextureDataRef) {
        let texture_size = self.texture_size(texture);
        assert!(rect.size().x() >= 0);
        assert!(rect.size().y() >= 0);
        assert!(rect.max_x() <= texture_size.x());
        assert!(rect.max_y() <= texture_size.y());

        let format = self.texture_format(&texture.texture).expect("Unexpected texture format!");
        let data_ptr = data.check_and_extract_data_ptr(rect.size(), format);

        let origin = MTLOrigin { x: rect.origin().x() as u64, y: rect.origin().y() as u64, z: 0 };
        let size = MTLSize {
            width: rect.size().x() as u64,
            height: rect.size().y() as u64,
            depth: 1,
        };
        let region = MTLRegion { origin, size };
        let stride = format.bytes_per_pixel() as u64 * size.width;
        texture.texture.replace_region(region, 0, stride, data_ptr);

        texture.dirty.set(true);
    }

    fn read_pixels(&self, target: &RenderTarget<MetalDevice>, viewport: RectI)
                   -> MetalTextureDataReceiver {
        let texture = self.render_target_color_texture(target);
        let texture_data_receiver =
            MetalTextureDataReceiver(Arc::new(MetalTextureDataReceiverInfo {
                mutex: Mutex::new(MetalTextureDataReceiverState::Pending),
                cond: Condvar::new(),
                texture,
                viewport,
            }));

        let texture_data_receiver_for_block = texture_data_receiver.clone();
        let block = ConcreteBlock::new(move |_| {
            texture_data_receiver_for_block.download();
        });

        self.synchronize_texture(&texture_data_receiver.0.texture, block.copy());
        texture_data_receiver
    }

    fn begin_commands(&self) {
        self.command_buffers.borrow_mut().push(self.command_queue.new_command_buffer().retain());
    }

    fn end_commands(&self) {
        let command_buffer = self.command_buffers.borrow_mut().pop().unwrap();
        command_buffer.commit();
    }

    fn draw_arrays(&self, index_count: u32, render_state: &RenderState<MetalDevice>) {
        let encoder = self.prepare_to_draw(render_state);
        let primitive = render_state.primitive.to_metal_primitive();
        encoder.draw_primitives(primitive, 0, index_count as u64);
        encoder.end_encoding();
    }

    fn draw_elements(&self, index_count: u32, render_state: &RenderState<MetalDevice>) {
        let encoder = self.prepare_to_draw(render_state);
        let primitive = render_state.primitive.to_metal_primitive();
        let index_type = MTLIndexType::UInt32;
        let index_count = index_count as u64;
        let index_buffer = render_state.vertex_array
                                       .index_buffer
                                       .borrow();
        let index_buffer = index_buffer.as_ref().expect("No index buffer bound to VAO!");
        let index_buffer = index_buffer.buffer.borrow();
        let index_buffer = index_buffer.as_ref().expect("Index buffer not allocated!");
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
                                       .borrow();
        let index_buffer = index_buffer.as_ref().expect("No index buffer bound to VAO!");
        let index_buffer = index_buffer.buffer.borrow();
        let index_buffer = index_buffer.as_ref().expect("Index buffer not allocated!");
        encoder.draw_indexed_primitives_instanced(primitive,
                                                  index_count as u64,
                                                  index_type,
                                                  index_buffer,
                                                  0,
                                                  instance_count as u64);
        encoder.end_encoding();
    }

    fn dispatch_compute(&self,
                        size: ComputeDimensions,
                        compute_state: &ComputeState<MetalDevice>) {
        let command_buffers = self.command_buffers.borrow();
        let command_buffer = command_buffers.last().unwrap();

        let encoder = command_buffer.new_compute_command_encoder();

        let program = match compute_state.program {
            MetalProgram::Compute(ref compute_program) => compute_program,
            _ => panic!("Compute render command must use a compute program!"),
        };

        let compute_pipeline_descriptor = ComputePipelineDescriptor::new();
        compute_pipeline_descriptor.set_compute_function(Some(&program.shader.function));

        let compute_pipeline_state = unsafe {
            if program.shader.arguments.borrow().is_none() {
            // FIXME(pcwalton): Factor these raw Objective-C method calls out into a trait.
                let mut reflection: *mut Object = ptr::null_mut();
                let reflection_options = MTLPipelineOption::ArgumentInfo |
                    MTLPipelineOption::BufferTypeInfo;
                let mut error: *mut Object = ptr::null_mut();
                let raw_compute_pipeline_state: *mut MTLComputePipelineState = msg_send![
                    self.device.as_ptr(),
                    newComputePipelineStateWithDescriptor:compute_pipeline_descriptor.as_ptr()
                                                options:reflection_options
                                             reflection:&mut reflection
                                                  error:&mut error];
                let compute_pipeline_state =
                    ComputePipelineState::from_ptr(raw_compute_pipeline_state);
                *program.shader.arguments.borrow_mut() =
                    Some(ArgumentArray::from_ptr(msg_send![reflection, arguments]));
                compute_pipeline_state
            } else {
                self.device
                    .new_compute_pipeline_state(&compute_pipeline_descriptor)
                    .expect("Failed to create compute pipeline state!")
            }
        };

        self.set_compute_uniforms(&encoder, &compute_state);
        encoder.set_compute_pipeline_state(&compute_pipeline_state);

        let local_size = match compute_state.program {
            MetalProgram::Compute(MetalComputeProgram { ref local_size, .. }) => *local_size,
            _ => panic!("Program was not a compute program!"),
        };

        encoder.dispatch_thread_groups(size.to_metal_size(), local_size);
        encoder.end_encoding();
    }

    fn create_timer_query(&self) -> MetalTimerQuery {
        let event_value = self.next_timer_query_event_value.get();
        self.next_timer_query_event_value.set(event_value + 2);

        let query = MetalTimerQuery(Arc::new(MetalTimerQueryInfo {
            event_value,
            mutex: Mutex::new(MetalTimerQueryData { start_time: None, end_time: None }),
            cond: Condvar::new(),
        }));

        let captured_query = query.clone();
        let start_block = ConcreteBlock::new(move |_: *mut Object, _: u64| {
            let start_time = Instant::now();
            let mut guard = captured_query.0.mutex.lock().unwrap();
            guard.start_time = Some(start_time);
        });
        let captured_query = query.clone();
        let end_block = ConcreteBlock::new(move |_: *mut Object, _: u64| {
            let end_time = Instant::now();
            let mut guard = captured_query.0.mutex.lock().unwrap();
            guard.end_time = Some(end_time);
            captured_query.0.cond.notify_all();
        });
        self.shared_event.notify_listener_at_value(&self.shared_event_listener,
                                                   event_value,
                                                   start_block.copy());
        self.shared_event.notify_listener_at_value(&self.shared_event_listener,
                                                   event_value + 1,
                                                   end_block.copy());

        query
    }

    fn begin_timer_query(&self, query: &MetalTimerQuery) {
        self.command_buffers
            .borrow_mut()
            .last()
            .unwrap()
            .encode_signal_event(&self.shared_event, query.0.event_value);
    }

    fn end_timer_query(&self, query: &MetalTimerQuery) {
        self.command_buffers
            .borrow_mut()
            .last()
            .unwrap()
            .encode_signal_event(&self.shared_event, query.0.event_value + 1);
    }

    fn try_recv_timer_query(&self, query: &MetalTimerQuery) -> Option<Duration> {
        try_recv_timer_query_with_guard(&mut query.0.mutex.lock().unwrap())
    }

    fn recv_timer_query(&self, query: &MetalTimerQuery) -> Duration {
        let mut guard = query.0.mutex.lock().unwrap();
        loop {
            let duration = try_recv_timer_query_with_guard(&mut guard);
            if let Some(duration) = duration {
                return duration
            }
            guard = query.0.cond.wait(guard).unwrap();
        }
    }

    fn try_recv_texture_data(&self, receiver: &MetalTextureDataReceiver) -> Option<TextureData> {
        try_recv_texture_data_with_guard(&mut receiver.0.mutex.lock().unwrap())
    }

    fn recv_texture_data(&self, receiver: &MetalTextureDataReceiver) -> TextureData {
        let mut guard = receiver.0.mutex.lock().unwrap();
        loop {
            let texture_data = try_recv_texture_data_with_guard(&mut guard);
            if let Some(texture_data) = texture_data {
                return texture_data
            }
            guard = receiver.0.cond.wait(guard).unwrap();
        }
    }

    #[inline]
    fn create_shader(
        &self,
        resources: &dyn ResourceLoader,
        name: &str,
        kind: ShaderKind,
    ) -> Self::Shader {
        let suffix = match kind {
            ShaderKind::Vertex => 'v',
            ShaderKind::Fragment => 'f',
            ShaderKind::Compute => 'c',
        };
        let path = format!("shaders/metal/{}.{}s.metal", name, suffix);
        self.create_shader_from_source(name, &resources.slurp(&path).unwrap(), kind)
    }

    fn add_fence(&self) -> MetalFence {
        let fence = MetalFence(Arc::new(MetalFenceInfo {
            mutex: Mutex::new(MetalFenceStatus::Pending),
            cond: Condvar::new(),
        }));
        let captured_fence = fence.clone();
        let block = ConcreteBlock::new(move |_| {
            *captured_fence.0.mutex.lock().unwrap() = MetalFenceStatus::Resolved;
            captured_fence.0.cond.notify_all();
        });
        self.command_buffers.borrow_mut().last().unwrap().add_completed_handler(block.copy());
        self.end_commands();
        self.begin_commands();
        fence
    }

    fn wait_for_fence(&self, fence: &MetalFence) {
        let mut guard = fence.0.mutex.lock().unwrap();
        while let MetalFenceStatus::Pending = *guard {
            guard = fence.0.cond.wait(guard).unwrap();
        }
    }
}

impl MetalDevice {
    fn get_uniform_index(&self, shader: &MetalShader, name: &str) -> Option<MetalUniformIndex> {
        let uniforms = shader.arguments.borrow();
        let arguments = match *uniforms {
            None => panic!("get_uniform_index() called before reflection!"),
            Some(ref arguments) => arguments,
        };
        let main_name = format!("u{}", name);
        for argument_index in 0..arguments.len() {
            let argument = arguments.object_at(argument_index);
            let argument_name = argument.name();
            if argument_name == &main_name {
                return Some(MetalUniformIndex(argument.index()))
            }
        }
        None
    }

    fn get_texture_index(&self, shader: &MetalShader, name: &str) -> Option<MetalTextureIndex> {
        let arguments = shader.arguments.borrow();
        let arguments = match *arguments {
            None => panic!("get_texture_index() called before reflection!"),
            Some(ref arguments) => arguments,
        };
        let (main_name, sampler_name) = (format!("u{}", name), format!("u{}Smplr", name));
        let (mut main_argument, mut sampler_argument) = (None, None);
        for argument_index in 0..arguments.len() {
            let argument = arguments.object_at(argument_index);
            let argument_name = argument.name();
            if argument_name == &main_name {
                main_argument = Some(argument.index());
            } else if argument_name == &sampler_name {
                sampler_argument = Some(argument.index());
            }
        }
        match (main_argument, sampler_argument) {
            (Some(main), Some(sampler)) => Some(MetalTextureIndex { main, sampler }),
            _ => None,
        }
    }

    fn get_image_index(&self, shader: &MetalShader, name: &str) -> Option<MetalImageIndex> {
        let uniforms = shader.arguments.borrow();
        let arguments = match *uniforms {
            None => panic!("get_image_index() called before reflection!"),
            Some(ref arguments) => arguments,
        };
        let main_name = format!("u{}", name);
        for argument_index in 0..arguments.len() {
            let argument = arguments.object_at(argument_index);
            let argument_name = argument.name();
            if argument_name == &main_name {
                return Some(MetalImageIndex(argument.index()))
            }
        }
        None
    }

    fn get_storage_buffer_index(&self, shader: &MetalShader, name: &str)
                                -> Option<MetalStorageBufferIndex> {
        let uniforms = shader.arguments.borrow();
        let arguments = match *uniforms {
            None => panic!("get_storage_buffer_index() called before reflection!"),
            Some(ref arguments) => arguments,
        };
        let main_name = format!("i{}", name);
        let mut main_argument = None;
        for argument_index in 0..arguments.len() {
            let argument = arguments.object_at(argument_index);
            match argument.type_() {
                MTLArgumentType::Buffer => {}
                _ => continue,
            }
            match argument.buffer_data_type() {
                MTLDataType::Struct => {}
                _ => continue,
            }
            let struct_type = argument.buffer_struct_type();
            if struct_type.member_from_name(&main_name).is_some() {
                main_argument = Some(argument.index());
            }
        }
        main_argument.map(MetalStorageBufferIndex)
    }

    fn populate_uniform_indices_if_necessary(&self,
                                             uniform: &MetalUniform,
                                             program: &MetalProgram) {
        let mut indices = uniform.indices.borrow_mut();
        if indices.is_some() {
            return;
        }

        *indices = match program {
            MetalProgram::Raster(MetalRasterProgram {
                ref vertex_shader,
                ref fragment_shader,
            }) => {
                Some(MetalUniformIndices(ProgramKind::Raster {
                    vertex: self.get_uniform_index(vertex_shader, &uniform.name),
                    fragment: self.get_uniform_index(fragment_shader, &uniform.name),
                }))
            }
            MetalProgram::Compute(MetalComputeProgram { ref shader, .. }) => {
                let uniform_index = self.get_uniform_index(shader, &uniform.name);
                Some(MetalUniformIndices(ProgramKind::Compute(uniform_index)))
            }
        }
    }

    fn populate_texture_indices_if_necessary(&self,
                                             texture_parameter: &MetalTextureParameter,
                                             program: &MetalProgram) {
        let mut indices = texture_parameter.indices.borrow_mut();
        if indices.is_some() {
            return;
        }

        *indices = match program {
            MetalProgram::Raster(MetalRasterProgram {
                ref vertex_shader,
                ref fragment_shader,
            }) => {
                Some(MetalTextureIndices(ProgramKind::Raster {
                    vertex: self.get_texture_index(vertex_shader, &texture_parameter.name),
                    fragment: self.get_texture_index(fragment_shader, &texture_parameter.name),
                }))
            }
            MetalProgram::Compute(MetalComputeProgram { ref shader, .. }) => {
                let image_index = self.get_texture_index(shader, &texture_parameter.name);
                Some(MetalTextureIndices(ProgramKind::Compute(image_index)))
            }
        }
    }

    fn populate_image_indices_if_necessary(&self,
                                           image_parameter: &MetalImageParameter,
                                           program: &MetalProgram) {
        let mut indices = image_parameter.indices.borrow_mut();
        if indices.is_some() {
            return;
        }

        *indices = match program {
            MetalProgram::Raster(MetalRasterProgram {
                ref vertex_shader,
                ref fragment_shader,
            }) => {
                Some(MetalImageIndices(ProgramKind::Raster {
                    vertex: self.get_image_index(vertex_shader, &image_parameter.name),
                    fragment: self.get_image_index(fragment_shader, &image_parameter.name),
                }))
            }
            MetalProgram::Compute(MetalComputeProgram { ref shader, .. }) => {
                let image_index = self.get_image_index(shader, &image_parameter.name);
                Some(MetalImageIndices(ProgramKind::Compute(image_index)))
            }
        }
    }

    fn populate_storage_buffer_indices_if_necessary(&self,
                                                    storage_buffer: &MetalStorageBuffer,
                                                    program: &MetalProgram) {
        let mut indices = storage_buffer.indices.borrow_mut();
        if indices.is_some() {
            return;
        }

        *indices = match program {
            MetalProgram::Raster(MetalRasterProgram {
                ref vertex_shader,
                ref fragment_shader,
            }) => {
                Some(MetalStorageBufferIndices(ProgramKind::Raster {
                    vertex: self.get_storage_buffer_index(vertex_shader, &storage_buffer.name),
                    fragment: self.get_storage_buffer_index(fragment_shader, &storage_buffer.name),
                }))
            }
            MetalProgram::Compute(MetalComputeProgram { ref shader, .. }) => {
                let storage_buffer_index = self.get_storage_buffer_index(shader,
                                                                         &storage_buffer.name);
                Some(MetalStorageBufferIndices(ProgramKind::Compute(storage_buffer_index)))
            }
        }
    }

    fn render_target_color_texture(&self, render_target: &RenderTarget<MetalDevice>)
                                   -> Texture {
        match *render_target {
            RenderTarget::Default {..} => self.main_color_texture.retain(),
            RenderTarget::Framebuffer(framebuffer) => framebuffer.0.texture.retain(),
        }
    }

    fn render_target_depth_texture(&self, render_target: &RenderTarget<MetalDevice>)
                                   -> Option<Texture> {
        match *render_target {
            RenderTarget::Default {..} => Some(self.main_depth_stencil_texture.retain()),
            RenderTarget::Framebuffer(_) => None,
        }
    }

    fn render_target_has_depth(&self, render_target: &RenderTarget<MetalDevice>) -> bool {
        match *render_target {
            RenderTarget::Default {..} => true,
            RenderTarget::Framebuffer(_) => false,
        }
    }

    fn prepare_to_draw(&self, render_state: &RenderState<MetalDevice>) -> RenderCommandEncoder {
        let command_buffers = self.command_buffers.borrow();
        let command_buffer = command_buffers.last().unwrap();

        // FIXME(pcwalton): Is this necessary?
        let mut blit_command_encoder = None;
        for &(_, texture) in render_state.textures {
            if !texture.dirty.get() {
                continue;
            }
            if blit_command_encoder.is_none() {
                blit_command_encoder = Some(command_buffer.new_blit_command_encoder());
            }
            let blit_command_encoder =
                blit_command_encoder.as_ref().expect("Where's the blit command encoder?");
            blit_command_encoder.synchronize_resource(&texture.texture);
            texture.dirty.set(false);
        }
        if let Some(blit_command_encoder) = blit_command_encoder {
            blit_command_encoder.end_encoding();
        }

        let render_pass_descriptor = self.create_render_pass_descriptor(render_state);

        let encoder = command_buffer.new_render_command_encoder(&render_pass_descriptor).retain();
        self.set_viewport(&encoder, &render_state.viewport);

        let program = match render_state.program {
            MetalProgram::Raster(ref raster_program) => raster_program,
            _ => panic!("Raster render command must use a raster program!"),
        };

        let render_pipeline_descriptor = RenderPipelineDescriptor::new();
        render_pipeline_descriptor.set_vertex_function(Some(&program.vertex_shader.function));
        render_pipeline_descriptor.set_fragment_function(Some(&program.fragment_shader.function));
        render_pipeline_descriptor.set_vertex_descriptor(Some(&render_state.vertex_array
                                                                           .descriptor));

        // Create render pipeline state.
        let pipeline_color_attachment =
            render_pipeline_descriptor.color_attachments()
                                      .object_at(0)
                                      .expect("Where's the color attachment?");
        self.prepare_pipeline_color_attachment_for_render(pipeline_color_attachment,
                                                          render_state);

        if self.render_target_has_depth(render_state.target) {
            let depth_stencil_format = MTLPixelFormat::Depth32Float_Stencil8;
            render_pipeline_descriptor.set_depth_attachment_pixel_format(depth_stencil_format);
            render_pipeline_descriptor.set_stencil_attachment_pixel_format(depth_stencil_format);
        }

        let render_pipeline_state = if program.vertex_shader.arguments.borrow().is_none() ||
                program.fragment_shader.arguments.borrow().is_none() {
            let reflection_options = MTLPipelineOption::ArgumentInfo |
                MTLPipelineOption::BufferTypeInfo;
            let (render_pipeline_state, reflection) =
                self.device
                    .real_new_render_pipeline_state_with_reflection(&render_pipeline_descriptor,
                                                                    reflection_options);
            let mut vertex_arguments = program.vertex_shader.arguments.borrow_mut();
            let mut fragment_arguments = program.fragment_shader.arguments.borrow_mut();
            if vertex_arguments.is_none() {
                *vertex_arguments = Some(reflection.real_vertex_arguments());
            }
            if fragment_arguments.is_none() {
                *fragment_arguments = Some(reflection.real_fragment_arguments());
            }
            render_pipeline_state
        } else {
            self.device
                .new_render_pipeline_state(&render_pipeline_descriptor)
                .expect("Failed to create render pipeline state!")
        };

        for (vertex_buffer_index, vertex_buffer) in render_state.vertex_array
                                                                .vertex_buffers
                                                                .borrow()
                                                                .iter()
                                                                .enumerate() {
            let real_index = vertex_buffer_index as u64 + FIRST_VERTEX_BUFFER_INDEX;
            let buffer = vertex_buffer.buffer.borrow();
            let buffer = buffer.as_ref()
                               .map(|buffer| buffer.as_ref())
                               .expect("Where's the vertex buffer?");
            encoder.set_vertex_buffer(real_index, Some(buffer), 0);
        }

        self.set_raster_uniforms(&encoder, render_state);
        encoder.set_render_pipeline_state(&render_pipeline_state);
        self.set_depth_stencil_state(&encoder, render_state);
        encoder
    }

    fn set_raster_uniforms(&self,
                           render_command_encoder: &RenderCommandEncoderRef,
                           render_state: &RenderState<MetalDevice>) {
        let program = match render_state.program {
            MetalProgram::Raster(ref raster_program) => raster_program,
            _ => unreachable!(),
        };

        let vertex_arguments = program.vertex_shader.arguments.borrow();
        let fragment_arguments = program.fragment_shader.arguments.borrow();
        if vertex_arguments.is_none() && fragment_arguments.is_none() {
            return;
        }

        // Set uniforms.
        let uniform_buffer = self.create_uniform_buffer(&render_state.uniforms);
        for (&(uniform, _), buffer_range) in
                render_state.uniforms.iter().zip(uniform_buffer.ranges.iter()) {
            self.populate_uniform_indices_if_necessary(uniform, &render_state.program);

            let indices = uniform.indices.borrow_mut();
            let indices = indices.as_ref().unwrap();
            let (vertex_indices, fragment_indices) = match indices.0 {
                ProgramKind::Raster { ref vertex, ref fragment } => (vertex, fragment),
                _ => unreachable!(),
            };

            if let Some(vertex_index) = *vertex_indices {
                self.set_vertex_uniform(vertex_index,
                                        &uniform_buffer.data,
                                        buffer_range,
                                        render_command_encoder);
            }
            if let Some(fragment_index) = *fragment_indices {
                self.set_fragment_uniform(fragment_index,
                                          &uniform_buffer.data,
                                          buffer_range,
                                          render_command_encoder);
            }
        }

        // Set textures.
        for &(texture_param, texture) in render_state.textures {
            self.populate_texture_indices_if_necessary(texture_param, &render_state.program);

            let indices = texture_param.indices.borrow_mut();
            let indices = indices.as_ref().unwrap();
            let (vertex_indices, fragment_indices) = match indices.0 {
                ProgramKind::Raster { ref vertex, ref fragment } => (vertex, fragment),
                _ => unreachable!(),
            };

            if let Some(vertex_index) = *vertex_indices {
                self.encode_vertex_texture_parameter(vertex_index,
                                                     render_command_encoder,
                                                     texture);
            }
            if let Some(fragment_index) = *fragment_indices {
                self.encode_fragment_texture_parameter(fragment_index,
                                                       render_command_encoder,
                                                       texture);
            }
        }

        // Set images.
        for &(image_param, image, _) in render_state.images {
            self.populate_image_indices_if_necessary(image_param, &render_state.program);

            let indices = image_param.indices.borrow_mut();
            let indices = indices.as_ref().unwrap();
            let (vertex_indices, fragment_indices) = match indices.0 {
                ProgramKind::Raster { ref vertex, ref fragment } => (vertex, fragment),
                _ => unreachable!(),
            };

            if let Some(vertex_index) = *vertex_indices {
                render_command_encoder.set_vertex_texture(vertex_index.0, Some(&image.texture));
            }
            if let Some(fragment_index) = *fragment_indices {
                render_command_encoder.set_fragment_texture(fragment_index.0,
                                                            Some(&image.texture));
            }
        }
    }

    fn set_compute_uniforms(&self,
                            compute_command_encoder: &ComputeCommandEncoderRef,
                            compute_state: &ComputeState<MetalDevice>) {
        // Set uniforms.
        let uniform_buffer = self.create_uniform_buffer(&compute_state.uniforms);
        for (&(uniform, _), buffer_range) in
                compute_state.uniforms.iter().zip(uniform_buffer.ranges.iter()) {
            self.populate_uniform_indices_if_necessary(uniform, &compute_state.program);

            let indices = uniform.indices.borrow_mut();
            let indices = indices.as_ref().unwrap();
            let indices = match indices.0 {
                ProgramKind::Compute(ref indices) => indices,
                _ => unreachable!(),
            };

            if let Some(indices) = *indices {
                self.set_compute_uniform(indices,
                                         &uniform_buffer.data,
                                         buffer_range,
                                         compute_command_encoder);
            }
        }

        // Set textures.
        for &(texture_param, texture) in compute_state.textures {
            self.populate_texture_indices_if_necessary(texture_param, &compute_state.program);

            let indices = texture_param.indices.borrow_mut();
            let indices = indices.as_ref().unwrap();
            let indices = match indices.0 {
                ProgramKind::Compute(ref indices) => indices,
                _ => unreachable!(),
            };

            if let Some(indices) = *indices {
                self.encode_compute_texture_parameter(indices, compute_command_encoder, texture);
            }
        }

        // Set images.
        for &(image_param, image, _) in compute_state.images {
            self.populate_image_indices_if_necessary(image_param, &compute_state.program);

            let indices = image_param.indices.borrow_mut();
            let indices = indices.as_ref().unwrap();
            let indices = match indices.0 {
                ProgramKind::Compute(ref indices) => indices,
                _ => unreachable!(),
            };

            if let Some(indices) = *indices {
                compute_command_encoder.set_texture(indices.0, Some(&image.texture));
            }
        }

        // Set storage buffers.
        for &(storage_buffer_id, storage_buffer_binding) in compute_state.storage_buffers {
            self.populate_storage_buffer_indices_if_necessary(storage_buffer_id,
                                                              &compute_state.program);

            let indices = storage_buffer_id.indices.borrow_mut();
            let indices = indices.as_ref().unwrap();
            let indices = match indices.0 {
                ProgramKind::Compute(ref indices) => indices,
                _ => unreachable!(),
            };

            if let Some(index) = *indices {
                if let Some(ref buffer) = *storage_buffer_binding.buffer.borrow() {
                    compute_command_encoder.set_buffer(index.0, Some(buffer), 0);
                }
            }

        }
    }

    fn create_uniform_buffer(&self, uniforms: &[(&MetalUniform, UniformData)]) -> UniformBuffer {
        let (mut uniform_buffer_data, mut uniform_buffer_ranges) = (vec![], vec![]);
        for &(_, uniform_data) in uniforms.iter() {
            let start_index = uniform_buffer_data.len();
            match uniform_data {
                UniformData::Float(value) => {
                    uniform_buffer_data.write_f32::<NativeEndian>(value).unwrap()
                }
                UniformData::IVec2(vector) => {
                    uniform_buffer_data.write_i32::<NativeEndian>(vector.x()).unwrap();
                    uniform_buffer_data.write_i32::<NativeEndian>(vector.y()).unwrap();
                }
                UniformData::IVec3(values) => {
                    uniform_buffer_data.write_i32::<NativeEndian>(values[0]).unwrap();
                    uniform_buffer_data.write_i32::<NativeEndian>(values[1]).unwrap();
                    uniform_buffer_data.write_i32::<NativeEndian>(values[2]).unwrap();
                }
                UniformData::Int(value) => {
                    uniform_buffer_data.write_i32::<NativeEndian>(value).unwrap()
                }
                UniformData::Mat2(matrix) => {
                    uniform_buffer_data.write_f32::<NativeEndian>(matrix.x()).unwrap();
                    uniform_buffer_data.write_f32::<NativeEndian>(matrix.y()).unwrap();
                    uniform_buffer_data.write_f32::<NativeEndian>(matrix.z()).unwrap();
                    uniform_buffer_data.write_f32::<NativeEndian>(matrix.w()).unwrap();
                }
                UniformData::Mat4(matrix) => {
                    for column in &matrix {
                        uniform_buffer_data.write_f32::<NativeEndian>(column.x()).unwrap();
                        uniform_buffer_data.write_f32::<NativeEndian>(column.y()).unwrap();
                        uniform_buffer_data.write_f32::<NativeEndian>(column.z()).unwrap();
                        uniform_buffer_data.write_f32::<NativeEndian>(column.w()).unwrap();
                    }
                }
                UniformData::Vec2(vector) => {
                    uniform_buffer_data.write_f32::<NativeEndian>(vector.x()).unwrap();
                    uniform_buffer_data.write_f32::<NativeEndian>(vector.y()).unwrap();
                }
                UniformData::Vec3(array) => {
                    uniform_buffer_data.write_f32::<NativeEndian>(array[0]).unwrap();
                    uniform_buffer_data.write_f32::<NativeEndian>(array[1]).unwrap();
                    uniform_buffer_data.write_f32::<NativeEndian>(array[2]).unwrap();
                }
                UniformData::Vec4(vector) => {
                    uniform_buffer_data.write_f32::<NativeEndian>(vector.x()).unwrap();
                    uniform_buffer_data.write_f32::<NativeEndian>(vector.y()).unwrap();
                    uniform_buffer_data.write_f32::<NativeEndian>(vector.z()).unwrap();
                    uniform_buffer_data.write_f32::<NativeEndian>(vector.w()).unwrap();
                }
            }
            let end_index = uniform_buffer_data.len();
            while uniform_buffer_data.len() % 256 != 0 {
                uniform_buffer_data.push(0);
            }
            uniform_buffer_ranges.push(start_index..end_index);
        }

        UniformBuffer {
            data: uniform_buffer_data,
            ranges: uniform_buffer_ranges,
        }
    }

    fn set_vertex_uniform(&self,
                          argument_index: MetalUniformIndex,
                          buffer: &[u8],
                          buffer_range: &Range<usize>,
                          render_command_encoder: &RenderCommandEncoderRef) {
        render_command_encoder.set_vertex_bytes(
            argument_index.0,
            (buffer_range.end - buffer_range.start) as u64,
            &buffer[buffer_range.start as usize] as *const u8 as *const _)
    }

    fn set_fragment_uniform(&self,
                            argument_index: MetalUniformIndex,
                            buffer: &[u8],
                            buffer_range: &Range<usize>,
                            render_command_encoder: &RenderCommandEncoderRef) {
        render_command_encoder.set_fragment_bytes(
            argument_index.0,
            (buffer_range.end - buffer_range.start) as u64,
            &buffer[buffer_range.start as usize] as *const u8 as *const _)
    }

    fn set_compute_uniform(&self,
                           argument_index: MetalUniformIndex,
                           buffer: &[u8],
                           buffer_range: &Range<usize>,
                           compute_command_encoder: &ComputeCommandEncoderRef) {
        compute_command_encoder.set_bytes(
            argument_index.0,
            (buffer_range.end - buffer_range.start) as u64,
            &buffer[buffer_range.start as usize] as *const u8 as *const _)
    }

    fn encode_vertex_texture_parameter(&self,
                                       argument_index: MetalTextureIndex,
                                       render_command_encoder: &RenderCommandEncoderRef,
                                       texture: &MetalTexture) {
        render_command_encoder.set_vertex_texture(argument_index.main, Some(&texture.texture));
        let sampler = &self.samplers[texture.sampling_flags.get().bits() as usize];
        render_command_encoder.set_vertex_sampler_state(argument_index.sampler, Some(sampler));
    }

    fn encode_fragment_texture_parameter(&self,
                                         argument_index: MetalTextureIndex,
                                         render_command_encoder: &RenderCommandEncoderRef,
                                         texture: &MetalTexture) {
        render_command_encoder.set_fragment_texture(argument_index.main, Some(&texture.texture));
        let sampler = &self.samplers[texture.sampling_flags.get().bits() as usize];
        render_command_encoder.set_fragment_sampler_state(argument_index.sampler, Some(sampler));
    }

    fn encode_compute_texture_parameter(&self,
                                        argument_index: MetalTextureIndex,
                                        compute_command_encoder: &ComputeCommandEncoderRef,
                                        texture: &MetalTexture) {
        compute_command_encoder.set_texture(argument_index.main, Some(&texture.texture));
        let sampler = &self.samplers[texture.sampling_flags.get().bits() as usize];
        compute_command_encoder.set_sampler_state(argument_index.sampler, Some(sampler));
    }

    fn prepare_pipeline_color_attachment_for_render(
            &self,
            pipeline_color_attachment: &RenderPipelineColorAttachmentDescriptorRef,
            render_state: &RenderState<MetalDevice>) {
        let pixel_format = self.render_target_color_texture(&render_state.target).pixel_format();
        pipeline_color_attachment.set_pixel_format(pixel_format);

        match render_state.options.blend {
            None => pipeline_color_attachment.set_blending_enabled(false),
            Some(ref blend) => {
                pipeline_color_attachment.set_blending_enabled(true);

                pipeline_color_attachment.set_source_rgb_blend_factor(
                    blend.src_rgb_factor.to_metal_blend_factor());
                pipeline_color_attachment.set_destination_rgb_blend_factor(
                    blend.dest_rgb_factor.to_metal_blend_factor());
                pipeline_color_attachment.set_source_alpha_blend_factor(
                    blend.src_alpha_factor.to_metal_blend_factor());
                pipeline_color_attachment.set_destination_alpha_blend_factor(
                    blend.dest_alpha_factor.to_metal_blend_factor());

                let blend_op = blend.op.to_metal_blend_op();
                pipeline_color_attachment.set_rgb_blend_operation(blend_op);
                pipeline_color_attachment.set_alpha_blend_operation(blend_op);
            }
        }

        if render_state.options.color_mask {
            pipeline_color_attachment.set_write_mask(MTLColorWriteMask::all());
        } else {
            pipeline_color_attachment.set_write_mask(MTLColorWriteMask::empty());
        }
    }

    fn create_render_pass_descriptor(&self, render_state: &RenderState<MetalDevice>)
                                     -> RenderPassDescriptor {
        let render_pass_descriptor = RenderPassDescriptor::new().retain();
        let color_attachment = render_pass_descriptor.color_attachments().object_at(0).unwrap();
        color_attachment.set_texture(Some(&self.render_target_color_texture(render_state.target)));

        match render_state.options.clear_ops.color {
            Some(color) => {
                let color = MTLClearColor::new(color.r() as f64,
                                               color.g() as f64,
                                               color.b() as f64,
                                               color.a() as f64);
                color_attachment.set_clear_color(color);
                color_attachment.set_load_action(MTLLoadAction::Clear);
            }
            None => color_attachment.set_load_action(MTLLoadAction::Load),
        }
        color_attachment.set_store_action(MTLStoreAction::Store);

        let depth_stencil_texture = self.render_target_depth_texture(render_state.target);
        if let Some(depth_stencil_texture) = depth_stencil_texture {
            let depth_attachment = render_pass_descriptor.depth_attachment().unwrap();
            let stencil_attachment = render_pass_descriptor.stencil_attachment().unwrap();
            depth_attachment.set_texture(Some(&depth_stencil_texture));
            stencil_attachment.set_texture(Some(&depth_stencil_texture));

            match render_state.options.clear_ops.depth {
                Some(depth) => {
                    depth_attachment.set_clear_depth(depth as f64);
                    depth_attachment.set_load_action(MTLLoadAction::Clear);
                }
                None => depth_attachment.set_load_action(MTLLoadAction::Load),
            }
            depth_attachment.set_store_action(MTLStoreAction::Store);

            match render_state.options.clear_ops.stencil {
                Some(value) => {
                    stencil_attachment.set_clear_stencil(value as u32);
                    stencil_attachment.set_load_action(MTLLoadAction::Clear);
                }
                None => stencil_attachment.set_load_action(MTLLoadAction::Load),
            }
            stencil_attachment.set_store_action(MTLStoreAction::Store);
        }

        render_pass_descriptor
    }

    fn set_depth_stencil_state(&self,
                               encoder: &RenderCommandEncoderRef,
                               render_state: &RenderState<MetalDevice>) {
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
                stencil_descriptor.set_stencil_failure_operation(MTLStencilOperation::Keep);
                stencil_descriptor.set_depth_failure_operation(MTLStencilOperation::Keep);
                stencil_descriptor.set_depth_stencil_pass_operation(pass_operation);
                stencil_descriptor.set_write_mask(write_mask);
                depth_stencil_descriptor.set_front_face_stencil(Some(&stencil_descriptor));
                depth_stencil_descriptor.set_back_face_stencil(Some(&stencil_descriptor));
                encoder.set_stencil_reference_value(stencil_state.reference);
            }
            None => {
                depth_stencil_descriptor.set_front_face_stencil(None);
                depth_stencil_descriptor.set_back_face_stencil(None);
            }
        }

        let depth_stencil_state = self.device.new_depth_stencil_state(&depth_stencil_descriptor);
        encoder.set_depth_stencil_state(&depth_stencil_state);
    }

    fn texture_format(&self, texture: &Texture) -> Option<TextureFormat> {
        TextureFormat::from_metal_pixel_format(texture.pixel_format())
    }

    fn set_viewport(&self, encoder: &RenderCommandEncoderRef, viewport: &RectI) {
        encoder.set_viewport(MTLViewport {
            originX: viewport.origin().x() as f64,
            originY: viewport.origin().y() as f64,
            width: viewport.size().x() as f64,
            height: viewport.size().y() as f64,
            znear: 0.0,
            zfar: 1.0,
        })
    }

    fn synchronize_texture(&self, texture: &Texture, block: RcBlock<(*mut Object,), ()>) {
        let command_buffers = self.command_buffers.borrow();
        let command_buffer = command_buffers.last().unwrap();
        let encoder = command_buffer.new_blit_command_encoder();
        encoder.synchronize_resource(&texture);
        command_buffer.add_completed_handler(block);
        encoder.end_encoding();

        self.end_commands();
        self.begin_commands();
    }

    fn upload_to_metal_buffer<T>(&self, buffer: &Buffer, start: usize, data: &[T]) {
        unsafe {
            let start = (start * mem::size_of::<T>()) as u64;
            let size = (data.len() * mem::size_of::<T>()) as u64;
            assert!(start + size <= buffer.length());
            ptr::copy_nonoverlapping(data.as_ptr() as *const u8,
                                     (buffer.contents() as *mut u8).offset(start as isize),
                                     size as usize);
            buffer.did_modify_range(NSRange::new(start, size));
        }
    }
}

trait DeviceExtra {
    fn create_depth_stencil_texture(&self, size: Vector2I) -> Texture;
}

impl DeviceExtra for metal::Device {
    fn create_depth_stencil_texture(&self, size: Vector2I) -> Texture {
        let descriptor = TextureDescriptor::new();
        descriptor.set_texture_type(MTLTextureType::D2);
        descriptor.set_pixel_format(MTLPixelFormat::Depth32Float_Stencil8);
        descriptor.set_width(size.x() as u64);
        descriptor.set_height(size.y() as u64);
        descriptor.set_storage_mode(MTLStorageMode::Private);
        descriptor.set_usage(MTLTextureUsage::Unknown);
        self.new_texture(&descriptor)
    }
}

// Helper types

struct UniformBuffer {
    data: Vec<u8>,
    ranges: Vec<Range<usize>>,
}

// Miscellaneous extra public methods

impl MetalTexture {
    #[inline]
    pub fn metal_texture(&self) -> Texture {
        self.texture.clone()
    }
}

pub trait IntoTexture {
    unsafe fn into_texture(self, metal_device: &metal::Device) -> Texture;
}

impl IntoTexture for Texture {
    #[inline]
    unsafe fn into_texture(self, _: &metal::Device) -> Texture {
        self
    }
}

impl IntoTexture for IOSurfaceRef {
    #[inline]
    unsafe fn into_texture(self, metal_device: &metal::Device) -> Texture {
        let width = IOSurfaceGetWidth(self);
        let height = IOSurfaceGetHeight(self);

        let descriptor = TextureDescriptor::new();
        descriptor.set_texture_type(MTLTextureType::D2);
        descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        descriptor.set_width(width as u64);
        descriptor.set_height(height as u64);
        descriptor.set_storage_mode(MTLStorageMode::Managed);
        descriptor.set_usage(MTLTextureUsage::Unknown);

        msg_send![*metal_device, newTextureWithDescriptor:descriptor iosurface:self plane:0]
    }
}

impl<'a> IntoTexture for &'a CoreAnimationDrawableRef {
    #[inline]
    unsafe fn into_texture(self, _: &metal::Device) -> Texture {
        self.texture().retain()
    }
}

// Conversion helpers

trait BlendFactorExt {
    fn to_metal_blend_factor(self) -> MTLBlendFactor;
}

impl BlendFactorExt for BlendFactor {
    #[inline]
    fn to_metal_blend_factor(self) -> MTLBlendFactor {
        match self {
            BlendFactor::Zero => MTLBlendFactor::Zero,
            BlendFactor::One => MTLBlendFactor::One,
            BlendFactor::SrcAlpha => MTLBlendFactor::SourceAlpha,
            BlendFactor::OneMinusSrcAlpha => MTLBlendFactor::OneMinusSourceAlpha,
            BlendFactor::DestAlpha => MTLBlendFactor::DestinationAlpha,
            BlendFactor::OneMinusDestAlpha => MTLBlendFactor::OneMinusDestinationAlpha,
            BlendFactor::DestColor => MTLBlendFactor::DestinationColor,
        }
    }
}

trait BlendOpExt {
    fn to_metal_blend_op(self) -> MTLBlendOperation;
}

impl BlendOpExt for BlendOp {
    #[inline]
    fn to_metal_blend_op(self) -> MTLBlendOperation {
        match self {
            BlendOp::Add => MTLBlendOperation::Add,
            BlendOp::Subtract => MTLBlendOperation::Subtract,
            BlendOp::ReverseSubtract => MTLBlendOperation::ReverseSubtract,
            BlendOp::Min => MTLBlendOperation::Min,
            BlendOp::Max => MTLBlendOperation::Max,
        }
    }
}

trait BufferUploadModeExt {
    fn to_metal_resource_options(self) -> MTLResourceOptions;
}

impl BufferUploadModeExt for BufferUploadMode {
    #[inline]
    fn to_metal_resource_options(self) -> MTLResourceOptions {
        let mut options = match self {
            BufferUploadMode::Static => MTLResourceOptions::CPUCacheModeWriteCombined,
            BufferUploadMode::Dynamic => MTLResourceOptions::CPUCacheModeDefaultCache,
        };
        options |= MTLResourceOptions::StorageModeManaged;
        options
    }
}

trait ComputeDimensionsExt {
    fn to_metal_size(self) -> MTLSize;
}

impl ComputeDimensionsExt for ComputeDimensions {
    #[inline]
    fn to_metal_size(self) -> MTLSize {
        MTLSize { width: self.x as u64, height: self.y as u64, depth: self.z as u64 }
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

trait ImageAccessExt {
    fn to_metal_resource_usage(self) -> MTLResourceUsage;
}

impl ImageAccessExt for ImageAccess {
    fn to_metal_resource_usage(self) -> MTLResourceUsage {
        match self {
            ImageAccess::Read => MTLResourceUsage::Read,
            ImageAccess::Write => MTLResourceUsage::Write,
            ImageAccess::ReadWrite => MTLResourceUsage::Read | MTLResourceUsage::Write,
        }
    }
}

trait PrimitiveExt {
    fn to_metal_primitive(self) -> MTLPrimitiveType;
}

impl PrimitiveExt for Primitive {
    fn to_metal_primitive(self) -> MTLPrimitiveType {
        match self {
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

trait UniformDataExt {
    fn as_bytes(&self) -> &[u8];
}

impl UniformDataExt for UniformData {
    fn as_bytes(&self) -> &[u8] {
        unsafe {
            match *self {
                UniformData::Float(ref data) => {
                    slice::from_raw_parts(data as *const f32 as *const u8, 4 * 1)
                }
                UniformData::IVec2(ref data) => {
                    slice::from_raw_parts(data as *const I32x2 as *const u8, 4 * 3)
                }
                UniformData::IVec3(ref data) => {
                    slice::from_raw_parts(data as *const i32 as *const u8, 4 * 3)
                }
                UniformData::Int(ref data) => {
                    slice::from_raw_parts(data as *const i32 as *const u8, 4 * 1)
                }
                UniformData::Mat2(ref data) => {
                    slice::from_raw_parts(data as *const F32x4 as *const u8, 4 * 4)
                }
                UniformData::Mat4(ref data) => {
                    slice::from_raw_parts(&data[0] as *const F32x4 as *const u8, 4 * 16)
                }
                UniformData::Vec2(ref data) => {
                    slice::from_raw_parts(data as *const F32x2 as *const u8, 4 * 2)
                }
                UniformData::Vec3(ref data) => {
                    slice::from_raw_parts(data as *const f32 as *const u8, 4 * 3)
                }
                UniformData::Vec4(ref data) => {
                    slice::from_raw_parts(data as *const F32x4 as *const u8, 4 * 4)
                }
            }
        }
    }
}

trait TextureFormatExt: Sized {
    fn from_metal_pixel_format(metal_pixel_format: MTLPixelFormat) -> Option<Self>;
}

impl TextureFormatExt for TextureFormat {
    fn from_metal_pixel_format(metal_pixel_format: MTLPixelFormat) -> Option<TextureFormat> {
        match metal_pixel_format {
            MTLPixelFormat::R8Unorm => Some(TextureFormat::R8),
            MTLPixelFormat::R16Float => Some(TextureFormat::R16F),
            MTLPixelFormat::RGBA8Unorm => Some(TextureFormat::RGBA8),
            MTLPixelFormat::BGRA8Unorm => {
                // FIXME(pcwalton): This is wrong! But it prevents a crash for now.
                Some(TextureFormat::RGBA8)
            }
            MTLPixelFormat::RGBA16Float => Some(TextureFormat::RGBA16F),
            MTLPixelFormat::RGBA32Float => Some(TextureFormat::RGBA32F),
            _ => None,
        }
    }
}

// Synchronization helpers

fn try_recv_timer_query_with_guard(guard: &mut MutexGuard<MetalTimerQueryData>)
                                   -> Option<Duration> {
    match (guard.start_time, guard.end_time) {
        (Some(start_time), Some(end_time)) => Some(end_time - start_time),
        _ => None,
    }
}

impl MetalTextureDataReceiver {
    fn download(&self) {
        let (origin, size) = (self.0.viewport.origin(), self.0.viewport.size());
        let metal_origin = MTLOrigin { x: origin.x() as u64, y: origin.y() as u64, z: 0 };
        let metal_size = MTLSize { width: size.x() as u64, height: size.y() as u64, depth: 1 };
        let metal_region = MTLRegion { origin: metal_origin, size: metal_size };

        let format = TextureFormat::from_metal_pixel_format(self.0.texture.pixel_format());
        let format = format.expect("Unexpected framebuffer texture format!");

        let texture_data = match format {
            TextureFormat::R8 | TextureFormat::RGBA8 => {
                let channels = format.channels();
                let stride = size.x() as usize * channels;
                let mut pixels = vec![0; stride * size.y() as usize];
                self.0.texture.get_bytes(pixels.as_mut_ptr() as *mut _,
                                         metal_region,
                                         0,
                                         stride as u64);
                TextureData::U8(pixels)
            }
            TextureFormat::R16F | TextureFormat::RGBA16F => {
                let channels = format.channels();
                let stride = size.x() as usize * channels;
                let mut pixels = vec![f16::default(); stride * size.y() as usize];
                self.0.texture.get_bytes(pixels.as_mut_ptr() as *mut _,
                                         metal_region,
                                         0,
                                         stride as u64 * 2);
                TextureData::F16(pixels)
            }
            TextureFormat::RGBA32F => {
                let channels = format.channels();
                let stride = size.x() as usize * channels;
                let mut pixels = vec![0.0; stride * size.y() as usize];
                self.0.texture.get_bytes(pixels.as_mut_ptr() as *mut _,
                                         metal_region,
                                         0,
                                         stride as u64 * 4);
                TextureData::F32(pixels)
            }
        };

        let mut guard = self.0.mutex.lock().unwrap();
        *guard = MetalTextureDataReceiverState::Downloaded(texture_data);
        self.0.cond.notify_all();
    }
}

fn try_recv_texture_data_with_guard(guard: &mut MutexGuard<MetalTextureDataReceiverState>)
                                    -> Option<TextureData> {
    match **guard {
        MetalTextureDataReceiverState::Pending | MetalTextureDataReceiverState::Finished => {
            return None
        }
        MetalTextureDataReceiverState::Downloaded(_) => {}
    }
    match mem::replace(&mut **guard, MetalTextureDataReceiverState::Finished) {
        MetalTextureDataReceiverState::Downloaded(texture_data) => Some(texture_data),
        _ => unreachable!(),
    }
}

// Extra structs missing from `metal-rs`

bitflags! {
    struct MTLPipelineOption: NSUInteger {
        const ArgumentInfo   = 1 << 0;
        const BufferTypeInfo = 1 << 1;
    }
}

// Extra objects missing from `metal-rs`

struct ArgumentArray(*mut Object);

impl Drop for ArgumentArray {
    fn drop(&mut self) {
        unsafe { msg_send![self.0, release] }
    }
}

impl ArgumentArray {
    unsafe fn from_ptr(object: *mut Object) -> ArgumentArray {
        ArgumentArray(msg_send![object, retain])
    }

    fn len(&self) -> u64 {
        unsafe { msg_send![self.0, count] }
    }

    fn object_at(&self, index: u64) -> Argument {
        unsafe {
            let argument: *mut MTLArgument = msg_send![self.0, objectAtIndex:index];
            Argument::from_ptr(msg_send![argument, retain])
        }
    }
}

struct SharedEvent(*mut Object);

impl Drop for SharedEvent {
    fn drop(&mut self) {
        unsafe { msg_send![self.0, release] }
    }
}

impl SharedEvent {
    fn notify_listener_at_value(&self,
                                listener: &SharedEventListener,
                                value: u64,
                                block: RcBlock<(*mut Object, u64), ()>) {
        unsafe {
            // If the block doesn't have a signature, this segfaults.
            let block = &*block as
                *const Block<(*mut Object, u64), ()> as
                *mut Block<(*mut Object, u64), ()> as
                *mut BlockBase<(*mut Object, u64), ()>;
            (*block).flags |= BLOCK_HAS_SIGNATURE | BLOCK_HAS_COPY_DISPOSE;
            (*block).extra = &BLOCK_EXTRA;
            let () = msg_send![self.0, notifyListener:listener.0 atValue:value block:block];
            mem::forget(block);
        }

        extern "C" fn dtor(_: *mut BlockBase<(*mut Object, u64), ()>) {}

        static mut SIGNATURE: &[u8] = b"v16@?0Q8\0";
        static mut SIGNATURE_PTR: *const i8 = unsafe { &SIGNATURE[0] as *const u8 as *const i8 };
        static mut BLOCK_EXTRA: BlockExtra<(*mut Object, u64), ()> = BlockExtra {
            unknown0: 0 as *mut i32,
            unknown1: 0 as *mut i32,
            unknown2: 0 as *mut i32,
            dtor: dtor,
            signature: unsafe { &SIGNATURE_PTR },
        };
    }
}

struct SharedEventListener(*mut Object);

impl Drop for SharedEventListener {
    fn drop(&mut self) {
        unsafe { msg_send![self.0, release] }
    }
}

impl SharedEventListener {
    fn new() -> SharedEventListener {
        unsafe {
            let listener: *mut Object = msg_send![class!(MTLSharedEventListener), alloc];
            SharedEventListener(msg_send![listener, init])
        }
    }
}

struct VertexAttributeArray(*mut Object);

impl Drop for VertexAttributeArray {
    fn drop(&mut self) {
        unsafe { msg_send![self.0, release] }
    }
}

impl VertexAttributeArray {
    unsafe fn from_ptr(object: *mut Object) -> VertexAttributeArray {
        VertexAttributeArray(msg_send![object, retain])
    }

    fn len(&self) -> u64 {
        unsafe { msg_send![self.0, count] }
    }

    fn object_at(&self, index: u64) -> &VertexAttributeRef {
        unsafe { VertexAttributeRef::from_ptr(msg_send![self.0, objectAtIndex:index]) }
    }
}

// Extra methods missing from `metal-rs`

trait CoreAnimationLayerExt {
    fn device(&self) -> metal::Device;
}

impl CoreAnimationLayerExt for CoreAnimationLayer {
    fn device(&self) -> metal::Device {
        unsafe {
            let device: *mut MTLDevice = msg_send![self.as_ptr(), device];
            metal::Device::from_ptr(msg_send![device, retain])
        }
    }
}

trait CommandBufferExt {
    fn encode_signal_event(&self, event: &SharedEvent, value: u64);
    fn add_completed_handler(&self, block: RcBlock<(*mut Object,), ()>);
}

impl CommandBufferExt for CommandBuffer {
    fn encode_signal_event(&self, event: &SharedEvent, value: u64) {
        unsafe {
            msg_send![self.as_ptr(), encodeSignalEvent:event.0 value:value]
        }
    }

    fn add_completed_handler(&self, block: RcBlock<(*mut Object,), ()>) {
        unsafe {
            msg_send![self.as_ptr(), addCompletedHandler:&*block]
        }
    }
}

trait DeviceExt {
    // `new_render_pipeline_state_with_reflection()` in `metal-rs` doesn't correctly initialize the
    // `reflection` argument. This is a better definition.
    fn real_new_render_pipeline_state_with_reflection(&self,
                                                      descriptor: &RenderPipelineDescriptor,
                                                      options: MTLPipelineOption)
                                                      -> (RenderPipelineState,
                                                          RenderPipelineReflection);
    fn new_shared_event(&self) -> SharedEvent;
}

impl DeviceExt for metal::Device {
    fn real_new_render_pipeline_state_with_reflection(&self,
                                                      descriptor: &RenderPipelineDescriptor,
                                                      options: MTLPipelineOption)
                                                      -> (RenderPipelineState,
                                                          RenderPipelineReflection) {
        unsafe {
            let mut reflection_ptr: *mut MTLRenderPipelineReflection = ptr::null_mut();
            let mut error_ptr: *mut Object = ptr::null_mut();
            let render_pipeline_state_ptr: *mut MTLRenderPipelineState =
                msg_send![self.as_ptr(),
                          newRenderPipelineStateWithDescriptor:descriptor.as_ptr()
                                                       options:options
                                                    reflection:&mut reflection_ptr
                                                         error:&mut error_ptr];
            if !error_ptr.is_null() {
                let description: CFStringRef = msg_send![error_ptr, description];
                panic!("Render pipeline state construction failed: {}",
                       CFString::wrap_under_get_rule(description).to_string());
            }
            assert!(!render_pipeline_state_ptr.is_null());
            assert!(!reflection_ptr.is_null());
            (RenderPipelineState::from_ptr(render_pipeline_state_ptr),
             RenderPipelineReflection::from_ptr(msg_send![reflection_ptr, retain]))
        }
    }

    fn new_shared_event(&self) -> SharedEvent {
        unsafe { SharedEvent(msg_send![self.as_ptr(), newSharedEvent]) }
    }
}

trait FunctionExt {
    // `vertex_attributes()` in `metal-rs` segfaults! This is a better definition.
    fn real_vertex_attributes(&self) -> VertexAttributeArray;
    fn new_argument_encoder_with_reflection(&self, buffer_index: u64)
                                            -> (ArgumentEncoder, Argument);
}

impl FunctionExt for Function {
    fn real_vertex_attributes(&self) -> VertexAttributeArray {
        unsafe {
            VertexAttributeArray::from_ptr(msg_send![(*self).as_ptr(), vertexAttributes])
        }
    }

    fn new_argument_encoder_with_reflection(&self, buffer_index: u64)
                                            -> (ArgumentEncoder, Argument) {
        unsafe {
            let mut reflection = ptr::null_mut();
            let encoder: *mut MTLArgumentEncoder =
                msg_send![self.as_ptr(), newArgumentEncoderWithBufferIndex:buffer_index
                                                                reflection:&mut reflection];
            let () = msg_send![reflection, retain];
            (ArgumentEncoder::from_ptr(encoder), Argument::from_ptr(reflection))
        }
    }
}

trait RenderPipelineReflectionExt {
    // `vertex_arguments()` in `metal-rs` segfaults! This is a better definition.
    fn real_vertex_arguments(&self) -> ArgumentArray;
    // `fragment_arguments()` in `metal-rs` segfaults! This is a better definition.
    fn real_fragment_arguments(&self) -> ArgumentArray;
}

impl RenderPipelineReflectionExt for RenderPipelineReflectionRef {
    fn real_vertex_arguments(&self) -> ArgumentArray {
        unsafe { ArgumentArray::from_ptr(msg_send![self.as_ptr(), vertexArguments]) }
    }

    fn real_fragment_arguments(&self) -> ArgumentArray {
        unsafe { ArgumentArray::from_ptr(msg_send![self.as_ptr(), fragmentArguments]) }
    }
}

trait StructMemberExt {
    fn argument_index(&self) -> u64;
    fn pointer_type(&self) -> *mut Object;
}

impl StructMemberExt for StructMemberRef {
    fn argument_index(&self) -> u64 {
        unsafe { msg_send![self.as_ptr(), argumentIndex] }
    }

    fn pointer_type(&self) -> *mut Object {
        unsafe { msg_send![self.as_ptr(), pointerType] }
    }
}

// Memory management helpers

trait Retain {
    type Owned;
    fn retain(&self) -> Self::Owned;
}

impl Retain for CommandBufferRef {
    type Owned = CommandBuffer;
    fn retain(&self) -> CommandBuffer {
        unsafe { CommandBuffer::from_ptr(msg_send![self.as_ptr(), retain]) }
    }
}

impl Retain for CoreAnimationDrawableRef {
    type Owned = CoreAnimationDrawable;
    fn retain(&self) -> CoreAnimationDrawable {
        unsafe { CoreAnimationDrawable::from_ptr(msg_send![self.as_ptr(), retain]) }
    }
}

impl Retain for CoreAnimationLayerRef {
    type Owned = CoreAnimationLayer;
    fn retain(&self) -> CoreAnimationLayer {
        unsafe { CoreAnimationLayer::from_ptr(msg_send![self.as_ptr(), retain]) }
    }
}

impl Retain for RenderCommandEncoderRef {
    type Owned = RenderCommandEncoder;
    fn retain(&self) -> RenderCommandEncoder {
        unsafe { RenderCommandEncoder::from_ptr(msg_send![self.as_ptr(), retain]) }
    }
}

impl Retain for RenderPassDescriptorRef {
    type Owned = RenderPassDescriptor;
    fn retain(&self) -> RenderPassDescriptor {
        unsafe { RenderPassDescriptor::from_ptr(msg_send![self.as_ptr(), retain]) }
    }
}

impl Retain for StructTypeRef {
    type Owned = StructType;
    fn retain(&self) -> StructType {
        unsafe { StructType::from_ptr(msg_send![self.as_ptr(), retain]) }
    }
}

impl Retain for TextureRef {
    type Owned = Texture;
    fn retain(&self) -> Texture {
        unsafe { Texture::from_ptr(msg_send![self.as_ptr(), retain]) }
    }
}

impl Retain for VertexAttributeRef {
    type Owned = VertexAttribute;
    fn retain(&self) -> VertexAttribute {
        unsafe { VertexAttribute::from_ptr(msg_send![self.as_ptr(), retain]) }
    }
}

impl Retain for VertexDescriptorRef {
    type Owned = VertexDescriptor;
    fn retain(&self) -> VertexDescriptor {
        unsafe { VertexDescriptor::from_ptr(msg_send![self.as_ptr(), retain]) }
    }
}

// Extra block stuff not supported by `block`

const BLOCK_HAS_COPY_DISPOSE: i32 = 0x02000000;
const BLOCK_HAS_SIGNATURE:    i32 = 0x40000000;

#[repr(C)]
struct BlockBase<A, R> {
    isa: *const Class,                                      // 0x00
    flags: i32,                                             // 0x08
    _reserved: i32,                                         // 0x0c
    invoke: unsafe extern fn(*mut Block<A, R>, ...) -> R,   // 0x10
    extra: *const BlockExtra<A, R>,                         // 0x18
}

type BlockExtraDtor<A, R> = extern "C" fn(*mut BlockBase<A, R>);

#[repr(C)]
struct BlockExtra<A, R> {
    unknown0: *mut i32,             // 0x00
    unknown1: *mut i32,             // 0x08
    unknown2: *mut i32,             // 0x10
    dtor: BlockExtraDtor<A, R>,     // 0x18
    signature: *const *const i8,    // 0x20
}

// TODO(pcwalton): These should go upstream to `core-foundation-rs`.
#[link(name = "IOSurface", kind = "framework")]
extern {
    fn IOSurfaceGetWidth(buffer: IOSurfaceRef) -> size_t;
    fn IOSurfaceGetHeight(buffer: IOSurfaceRef) -> size_t;
}
