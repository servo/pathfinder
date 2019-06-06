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

use metal::{CompileOptions, CoreAnimationLayerRef, DeviceRef, FunctionRef, LibraryRef, MTLOrigin};
use metal::{MTLPixelFormat, MTLRegion, MTLSize, MTLStorageMode, MTLTextureType, MTLTextureUsage};
use metal::{TextureDescriptor, TextureRef, VertexAttributeRef};
use pathfinder_geometry::basic::vector::Vector2I;
use pathfinder_gpu::resources::ResourceLoader;
use pathfinder_gpu::{Device, ShaderKind, TextureFormat};

pub struct MetalDevice {
    device: DeviceRef,
    layer: CoreAnimationLayerRef,
}

pub struct MetalProgram {
    vertex: MetalShader,
    fragment: MetalShader,
}

impl MetalDevice {
    #[inline]
    pub fn new(layer: CoreAnimationLayerRef) -> MetalDevice {
        let device = unsafe { DeviceRef::from_ptr(msg_send![layer.as_ptr(), device]) };
        MetalDevice { device, layer }
    }
}

pub struct MetalShader {
    library: LibraryRef,
    function: FunctionRef,
}

// TODO(pcwalton): Use `MTLEvent`s.
pub struct MetalTimerQuery;

impl Device for MetalDevice {
    type Program = MetalProgram;
    type Shader = MetalShader;
    type Texture = TextureRef;
    type TimerQuery = MetalTimerQuery;

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
        let origin = MTLOrigin { x: 0, y: 0, z: 0 };
        let size = MTLSize { width: size.x, height: size.y, depth: 1 };
        texture.replace_region(MTLRegion { origin, size }, size.width, data.as_ptr());
        texture
    }

    fn create_shader_from_source(&self, name: &str, source: &[u8], _: ShaderKind) -> MetalShader {
        let library = self.device.new_library_with_source(source, &CompileOptions::new()).unwrap();
        let function = library.get_function("main0", None).unwrap();
        MetalShader { library, function }
    }

    fn create_program_from_shaders(&self,
                                   _: &dyn ResourceLoader,
                                   _: &str,
                                   vertex_shader: MetalShader,
                                   fragment_shader: MetalShader)
                                   -> MetalProgram {
        MetalProgram { vertex_shader, fragment_shader }
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

    fn get_uniform(&self, program: &Self::Program, name: &str) -> Self::Uniform {
    }

    fn create_timer_query(&self) -> MetalTimerQuery { MetalTimerQuery }
    fn begin_timer_query(&self, _: &MetalTimerQuery) {}
    fn end_timer_query(&self, query: &MetalTimerQuery) {}
    fn timer_query_is_available(&self, query: &MetalTimerQuery) -> bool { true }
    fn get_timer_query(&self, query: &MetalTimerQuery) -> Duration { Duration::from_secs(0) }
}
