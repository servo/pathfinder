// pathfinder/glow/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A GLOW (GL on Whatever) implementation of the device abstraction.
//!
//! These bindings can be used in many environments (Open GL, OpenGL ES, and WebGL) and avoid
//! target-specific code. This can be used with wasm32-unknown-unknown + web-sys, stdweb, and
//! natively.
//!
//! See examples/canvas_glow for an example of how to use this.

use glow::*;
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::vector::Vector2I;
use pathfinder_gpu::resources::ResourceLoader;
use pathfinder_gpu::{BlendState, BufferData, BufferTarget, BufferUploadMode, RenderTarget};
use pathfinder_gpu::{ClearOps, DepthFunc, Device, Primitive, RenderOptions, RenderState};
use pathfinder_gpu::{ShaderKind, StencilFunc, TextureData, TextureFormat, UniformData};
use pathfinder_gpu::{VertexAttrClass, VertexAttrDescriptor, VertexAttrType};
use std::mem;
use std::str;
use std::sync::Arc;
use std::time::Duration;

pub struct GLOWDevice {
    context: Arc<Context>,
}

impl GLOWDevice {
    #[inline]
    pub fn new(context: Context) -> GLOWDevice {
        GLOWDevice {
            context: Arc::new(context),
        }
    }

    fn set_texture_parameters(&self, texture: &GLTexture) {
        self.bind_texture(texture, 0);
        unsafe {
            self.context.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            ck(&self.context);
            self.context.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            ck(&self.context);
            self.context.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            ck(&self.context);
            self.context.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            ck(&self.context);
        }
    }

    fn set_render_state(&self, render_state: &RenderState<GLOWDevice>) {
        self.bind_render_target(render_state.target);

        unsafe {
            let (origin, size) = (render_state.viewport.origin(), render_state.viewport.size());
            self.context
                .viewport(origin.x(), origin.y(), size.x(), size.y());
        }

        if render_state.options.clear_ops.has_ops() {
            self.clear(&render_state.options.clear_ops);
        }

        self.use_program(render_state.program);
        self.bind_vertex_array(render_state.vertex_array);
        for (texture_unit, texture) in render_state.textures.iter().enumerate() {
            self.bind_texture(texture, texture_unit as u32);
        }

        render_state
            .uniforms
            .iter()
            .for_each(|(uniform, data)| self.set_uniform(uniform, data));
        self.set_render_options(&render_state.options);
    }

    fn set_render_options(&self, render_options: &RenderOptions) {
        unsafe {
            // Set blend.
            match render_options.blend {
                BlendState::Off => {
                    self.context.disable(glow::BLEND);
                    ck(&self.context);
                }
                BlendState::RGBOneAlphaOne => {
                    self.context.blend_equation(glow::FUNC_ADD);
                    ck(&self.context);
                    self.context.blend_func(glow::ONE, glow::ONE);
                    ck(&self.context);
                    self.context.enable(glow::BLEND);
                    ck(&self.context);
                }
                BlendState::RGBOneAlphaOneMinusSrcAlpha => {
                    self.context.blend_equation(glow::FUNC_ADD);
                    ck(&self.context);
                    self.context.blend_func_separate(
                        glow::ONE,
                        glow::ONE_MINUS_SRC_ALPHA,
                        glow::ONE,
                        glow::ONE,
                    );
                    ck(&self.context);
                    self.context.enable(glow::BLEND);
                    ck(&self.context);
                }
                BlendState::RGBSrcAlphaAlphaOneMinusSrcAlpha => {
                    self.context.blend_equation(glow::FUNC_ADD);
                    ck(&self.context);
                    self.context.blend_func_separate(
                        glow::SRC_ALPHA,
                        glow::ONE_MINUS_SRC_ALPHA,
                        glow::ONE,
                        glow::ONE,
                    );
                    ck(&self.context);
                    self.context.enable(glow::BLEND);
                    ck(&self.context);
                }
            }

            // Set depth.
            match render_options.depth {
                None => {
                    self.context.disable(glow::DEPTH_TEST);
                    ck(&self.context);
                }
                Some(ref state) => {
                    self.context.depth_func(state.func.to_gl_depth_func());
                    ck(&self.context);
                    self.context.depth_mask(state.write as bool);
                    ck(&self.context);
                    self.context.enable(glow::DEPTH_TEST);
                    ck(&self.context);
                }
            }

            // Set stencil.
            match render_options.stencil {
                None => {
                    self.context.disable(glow::STENCIL_TEST);
                    ck(&self.context);
                }
                Some(ref state) => {
                    self.context.stencil_func(
                        state.func.to_gl_stencil_func(),
                        state.reference as i32,
                        state.mask,
                    );
                    ck(&self.context);
                    let (pass_action, write_mask) = if state.write {
                        (glow::REPLACE, state.mask)
                    } else {
                        (glow::KEEP, 0)
                    };
                    self.context.stencil_op(glow::KEEP, glow::KEEP, pass_action);
                    ck(&self.context);
                    self.context.stencil_mask(write_mask);
                    self.context.enable(glow::STENCIL_TEST);
                    ck(&self.context);
                }
            }

            // Set color mask.
            let color_mask = render_options.color_mask as bool;
            self.context
                .color_mask(color_mask, color_mask, color_mask, color_mask);
            ck(&self.context);
        }
    }

    fn set_uniform(&self, uniform: &GLUniform, data: &UniformData) {
        let location = uniform.location;
        if location.is_none() {
            return;
        }
        unsafe {
            match *data {
                UniformData::Float(value) => {
                    self.context.uniform_1_f32(location, value);
                    ck(&self.context);
                }
                UniformData::Int(value) => {
                    self.context.uniform_1_i32(location, value);
                    ck(&self.context);
                }
                UniformData::Mat2(data) => {
                    self.context.uniform_matrix_2_f32_slice(
                        location,
                        false,
                        &[data.x(), data.y(), data.z(), data.w()],
                    );
                }
                UniformData::Mat4(data) => {
                    self.context.uniform_matrix_4_f32_slice(
                        location,
                        false,
                        &[
                            data[0].x(),
                            data[0].y(),
                            data[0].z(),
                            data[0].w(),
                            data[1].x(),
                            data[1].y(),
                            data[1].z(),
                            data[1].w(),
                            data[2].x(),
                            data[2].y(),
                            data[2].z(),
                            data[2].w(),
                            data[3].x(),
                            data[3].y(),
                            data[3].z(),
                            data[3].w(),
                        ],
                    );
                }
                UniformData::Vec2(data) => {
                    self.context.uniform_2_f32(location, data.x(), data.y());
                    ck(&self.context);
                }
                UniformData::Vec4(data) => {
                    self.context
                        .uniform_4_f32(location, data.x(), data.y(), data.z(), data.w());
                    ck(&self.context);
                }
                UniformData::TextureUnit(unit) => {
                    self.context.uniform_1_i32(location, unit as i32);
                    ck(&self.context);
                }
            }
        }
    }

    fn reset_render_state(&self, render_state: &RenderState<GLOWDevice>) {
        self.reset_render_options(&render_state.options);
        for texture_unit in 0..(render_state.textures.len() as u32) {
            self.unbind_texture(texture_unit);
        }
        self.unuse_program();
        self.unbind_vertex_array();
    }

    fn reset_render_options(&self, render_options: &RenderOptions) {
        unsafe {
            match render_options.blend {
                BlendState::Off => {}
                BlendState::RGBOneAlphaOneMinusSrcAlpha
                | BlendState::RGBOneAlphaOne
                | BlendState::RGBSrcAlphaAlphaOneMinusSrcAlpha => {
                    self.context.disable(glow::BLEND);
                    ck(&self.context);
                }
            }

            if render_options.depth.is_some() {
                self.context.disable(glow::DEPTH_TEST);
                ck(&self.context);
            }

            if render_options.stencil.is_some() {
                self.context.stencil_mask(!0);
                ck(&self.context);
                self.context.disable(glow::STENCIL_TEST);
                ck(&self.context);
            }

            self.context.color_mask(true, true, true, true);
            ck(&self.context);
        }
    }
}

impl Device for GLOWDevice {
    type Buffer = GLBuffer;
    type Framebuffer = GLFramebuffer;
    type Program = GLProgram;
    type Shader = GLShader;
    type Texture = GLTexture;
    type TimerQuery = GLTimerQuery;
    type Uniform = GLUniform;
    type VertexArray = GLVertexArray;
    type VertexAttr = GLVertexAttr;

    fn create_texture(&self, format: TextureFormat, size: Vector2I) -> GLTexture {
        let texture = GLTexture {
            gl_texture: unsafe {
                self.context
                    .create_texture()
                    .expect("Could not create texture")
            },
            size,
            format,
        };
        unsafe {
            ck(&self.context);
            self.bind_texture(&texture, 0);
            self.context.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                format.gl_internal_format(),
                size.x() as i32,
                size.y() as i32,
                0,
                format.gl_format(),
                format.gl_type(),
                None,
            );
            ck(&self.context);
        }

        self.set_texture_parameters(&texture);
        texture
    }

    fn create_texture_from_data(&self, size: Vector2I, data: &[u8]) -> GLTexture {
        assert!(data.len() >= size.x() as usize * size.y() as usize);

        let texture = GLTexture {
            gl_texture: unsafe {
                self.context
                    .create_texture()
                    .expect("Could not create texture")
            },
            size,
            format: TextureFormat::R8,
        };
        unsafe {
            ck(&self.context);
            self.bind_texture(&texture, 0);
            self.context.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::R8 as i32,
                size.x() as i32,
                size.y() as i32,
                0,
                glow::RED,
                glow::UNSIGNED_BYTE,
                Some(data),
            );
            ck(&self.context);
        }

        self.set_texture_parameters(&texture);
        texture
    }

    fn create_shader_from_source(&self, name: &str, source: &[u8], kind: ShaderKind) -> GLShader {
        let glsl_version_spec = "300 es";

        let mut output = vec![];
        self.preprocess(&mut output, source, glsl_version_spec);
        let source = output;

        let gl_shader_kind = match kind {
            ShaderKind::Vertex => glow::VERTEX_SHADER,
            ShaderKind::Fragment => glow::FRAGMENT_SHADER,
        };

        unsafe {
            let gl_shader = self
                .context
                .create_shader(gl_shader_kind)
                .expect("Could not create shader");
            ck(&self.context);
            self.context.shader_source(
                gl_shader,
                str::from_utf8(&source).expect("Shader needs to be utf8"),
            );
            ck(&self.context);
            self.context.compile_shader(gl_shader);
            ck(&self.context);

            let compile_status = self.context.get_shader_compile_status(gl_shader);
            ck(&self.context);
            if !compile_status {
                let info_log = self.context.get_shader_info_log(gl_shader);
                ck(&self.context);
                println!("Shader info log:\n{}", &info_log);
                panic!("{:?} shader '{}' compilation failed", kind, name);
            }

            GLShader {
                context: self.context.clone(),
                gl_shader,
            }
        }
    }

    fn create_program_from_shaders(
        &self,
        _resources: &dyn ResourceLoader,
        name: &str,
        vertex_shader: GLShader,
        fragment_shader: GLShader,
    ) -> GLProgram {
        let gl_program;
        unsafe {
            gl_program = self
                .context
                .create_program()
                .expect("Could not create program");
            ck(&self.context);
            self.context
                .attach_shader(gl_program, vertex_shader.gl_shader);
            ck(&self.context);
            self.context
                .attach_shader(gl_program, fragment_shader.gl_shader);
            ck(&self.context);
            self.context.link_program(gl_program);
            ck(&self.context);

            let link_status = self.context.get_program_link_status(gl_program);
            ck(&self.context);
            if !link_status {
                let info_log = self.context.get_program_info_log(gl_program);
                ck(&self.context);
                println!("Program info log:\n{}", &info_log);
                panic!("Program '{}' linking failed", name);
            }
        }

        GLProgram {
            context: self.context.clone(),
            gl_program,
            vertex_shader,
            fragment_shader,
        }
    }

    #[inline]
    fn create_vertex_array(&self) -> GLVertexArray {
        unsafe {
            GLVertexArray {
                context: self.context.clone(),
                gl_vertex_array: self.context.create_vertex_array().unwrap(),
            }
        }
    }

    fn get_vertex_attr(&self, program: &Self::Program, name: &str) -> Option<GLVertexAttr> {
        let attr = unsafe {
            self.context
                .get_attrib_location(program.gl_program, &format!("a{}", name))?
        };
        ck(&self.context);
        Some(GLVertexAttr {
            context: self.context.clone(),
            attr: attr as u32,
        })
    }

    fn get_uniform(&self, program: &GLProgram, name: &str) -> GLUniform {
        let location = unsafe {
            self.context
                .get_uniform_location(program.gl_program, &format!("u{}", name))
        };
        ck(&self.context);
        GLUniform { location }
    }

    fn configure_vertex_attr(
        &self,
        vertex_array: &GLVertexArray,
        attr: &GLVertexAttr,
        descriptor: &VertexAttrDescriptor,
    ) {
        debug_assert_ne!(descriptor.stride, 0);

        self.bind_vertex_array(vertex_array);

        unsafe {
            let attr_type = descriptor.attr_type.to_gl_type();
            match descriptor.class {
                VertexAttrClass::Float | VertexAttrClass::FloatNorm => {
                    let normalized = if descriptor.class == VertexAttrClass::FloatNorm {
                        true
                    } else {
                        false
                    };
                    self.context.vertex_attrib_pointer_f32(
                        attr.attr,
                        descriptor.size as i32,
                        attr_type,
                        normalized,
                        descriptor.stride as i32,
                        descriptor.offset as i32,
                    );
                    ck(&self.context);
                }
                VertexAttrClass::Int => {
                    self.context.vertex_attrib_pointer_i32(
                        attr.attr,
                        descriptor.size as i32,
                        attr_type,
                        descriptor.stride as i32,
                        descriptor.offset as i32,
                    );
                    ck(&self.context);
                }
            }

            self.context
                .vertex_attrib_divisor(attr.attr, descriptor.divisor);
            ck(&self.context);
            self.context.enable_vertex_attrib_array(attr.attr);
            ck(&self.context);
        }

        self.unbind_vertex_array();
    }

    fn create_framebuffer(&self, texture: GLTexture) -> GLFramebuffer {
        let gl_framebuffer = unsafe { self.context.create_framebuffer().unwrap() };

        unsafe {
            ck(&self.context);
            self.context
                .bind_framebuffer(glow::FRAMEBUFFER, Some(gl_framebuffer));
            ck(&self.context);
            self.bind_texture(&texture, 0);
            self.context.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(texture.gl_texture),
                0,
            );
            ck(&self.context);
            assert_eq!(
                self.context.check_framebuffer_status(glow::FRAMEBUFFER),
                glow::FRAMEBUFFER_COMPLETE
            );
        }

        GLFramebuffer {
            context: self.context.clone(),
            gl_framebuffer,
            texture,
        }
    }

    fn create_buffer(&self) -> GLBuffer {
        unsafe {
            let gl_buffer = self.context.create_buffer().unwrap();
            ck(&self.context);
            GLBuffer {
                context: self.context.clone(),
                gl_buffer,
            }
        }
    }

    fn allocate_buffer<T>(
        &self,
        buffer: &GLBuffer,
        data: BufferData<T>,
        target: BufferTarget,
        mode: BufferUploadMode,
    ) {
        let target = match target {
            BufferTarget::Vertex => glow::ARRAY_BUFFER,
            BufferTarget::Index => glow::ELEMENT_ARRAY_BUFFER,
        };
        let usage = mode.to_gl_usage();
        unsafe {
            self.context.bind_buffer(target, Some(buffer.gl_buffer));
            ck(&self.context);
            match data {
                BufferData::Uninitialized(len) => {
                    self.context.buffer_data_size(target, len as i32, usage);
                }
                BufferData::Memory(buffer) => {
                    let len = buffer.len() * mem::size_of::<T>();
                    // Eek! Is this right?
                    let slice: &[u8] =
                        std::slice::from_raw_parts(buffer.as_ptr() as *const u8, len);
                    self.context.buffer_data_u8_slice(target, slice, usage);
                }
            }
            ck(&self.context);
        }
    }

    #[inline]
    fn framebuffer_texture<'f>(&self, framebuffer: &'f Self::Framebuffer) -> &'f Self::Texture {
        &framebuffer.texture
    }

    #[inline]
    fn texture_size(&self, texture: &Self::Texture) -> Vector2I {
        texture.size
    }

    fn upload_to_texture(&self, texture: &Self::Texture, size: Vector2I, data: &[u8]) {
        assert!(data.len() >= size.x() as usize * size.y() as usize * 4);
        unsafe {
            self.bind_texture(texture, 0);
            self.context.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                size.x() as i32,
                size.y() as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(data),
            );
            ck(&self.context);
        }

        self.set_texture_parameters(texture);
    }

    fn read_pixels(
        &self,
        _render_target: &RenderTarget<GLOWDevice>,
        _viewport: RectI,
    ) -> TextureData {
        panic!("read_pixels not supported");
    }

    fn begin_commands(&self) {
        // TODO(pcwalton): Add some checks in debug mode to make sure render commands are bracketed
        // by these?
    }

    fn end_commands(&self) {
        unsafe {
            self.context.flush();
        }
    }

    fn draw_arrays(&self, index_count: u32, render_state: &RenderState<Self>) {
        self.set_render_state(render_state);
        unsafe {
            self.context.draw_arrays(
                render_state.primitive.to_gl_primitive(),
                0,
                index_count as i32,
            );
            ck(&self.context);
        }
        self.reset_render_state(render_state);
    }

    fn draw_elements(&self, index_count: u32, render_state: &RenderState<Self>) {
        self.set_render_state(render_state);
        unsafe {
            self.context.draw_elements(
                render_state.primitive.to_gl_primitive(),
                index_count as i32,
                glow::UNSIGNED_INT,
                0,
            );
            ck(&self.context);
        }
        self.reset_render_state(render_state);
    }

    fn draw_elements_instanced(
        &self,
        index_count: u32,
        instance_count: u32,
        render_state: &RenderState<Self>,
    ) {
        self.set_render_state(render_state);
        unsafe {
            self.context.draw_elements_instanced(
                render_state.primitive.to_gl_primitive(),
                index_count as i32,
                glow::UNSIGNED_INT,
                0,
                instance_count as i32,
            );
            ck(&self.context);
        }
        self.reset_render_state(render_state);
    }

    #[inline]
    fn create_timer_query(&self) -> GLTimerQuery {
        // Stub.
        GLTimerQuery {}
    }

    #[inline]
    fn begin_timer_query(&self, _query: &Self::TimerQuery) {
        // Not implemented.
    }

    #[inline]
    fn end_timer_query(&self, _: &Self::TimerQuery) {
        // Not implemented
    }

    #[inline]
    fn get_timer_query(&self, _query: &Self::TimerQuery) -> Option<Duration> {
        // Stub
        None
    }

    #[inline]
    fn bind_buffer(&self, vertex_array: &GLVertexArray, buffer: &GLBuffer, target: BufferTarget) {
        self.bind_vertex_array(vertex_array);
        unsafe {
            self.context
                .bind_buffer(target.to_gl_target(), Some(buffer.gl_buffer));
            ck(&self.context);
        }
        self.unbind_vertex_array();
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
        };
        let path = format!("shaders/gl3/{}.{}s.glsl", name, suffix);
        self.create_shader_from_source(name, &resources.slurp(&path).unwrap(), kind)
    }
}

impl GLOWDevice {
    fn bind_render_target(&self, attachment: &RenderTarget<GLOWDevice>) {
        match *attachment {
            RenderTarget::Default => self.bind_default_framebuffer(),
            RenderTarget::Framebuffer(framebuffer) => self.bind_framebuffer(framebuffer),
        }
    }

    fn bind_vertex_array(&self, vertex_array: &GLVertexArray) {
        unsafe {
            self.context
                .bind_vertex_array(Some(vertex_array.gl_vertex_array));
            ck(&self.context);
        }
    }

    fn unbind_vertex_array(&self) {
        unsafe {
            self.context.bind_vertex_array(None);
            ck(&self.context);
        }
    }

    fn bind_texture(&self, texture: &GLTexture, unit: u32) {
        unsafe {
            self.context.active_texture(glow::TEXTURE0 + unit);
            ck(&self.context);
            self.context
                .bind_texture(glow::TEXTURE_2D, Some(texture.gl_texture));
            ck(&self.context);
        }
    }

    fn unbind_texture(&self, unit: u32) {
        unsafe {
            self.context.active_texture(glow::TEXTURE0 + unit);
            ck(&self.context);
            self.context.bind_texture(glow::TEXTURE_2D, None);
            ck(&self.context);
        }
    }

    fn use_program(&self, program: &GLProgram) {
        unsafe {
            self.context.use_program(Some(program.gl_program));
            ck(&self.context);
        }
    }

    fn unuse_program(&self) {
        unsafe {
            self.context.use_program(None);
            ck(&self.context);
        }
    }

    fn bind_default_framebuffer(&self) {
        unsafe {
            self.context.bind_framebuffer(glow::FRAMEBUFFER, None);
            ck(&self.context);
        }
    }

    fn bind_framebuffer(&self, framebuffer: &GLFramebuffer) {
        unsafe {
            self.context
                .bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer.gl_framebuffer));
            ck(&self.context);
        }
    }

    fn preprocess(&self, output: &mut Vec<u8>, source: &[u8], version: &str) {
        let mut index = 0;
        while index < source.len() {
            if source[index..].starts_with(b"{{") {
                let end_index = source[index..]
                    .iter()
                    .position(|character| *character == b'}')
                    .expect("Expected `}`!")
                    + index;
                assert_eq!(source[end_index + 1], b'}');
                let ident = String::from_utf8_lossy(&source[(index + 2)..end_index]);
                if ident == "version" {
                    output.extend_from_slice(version.as_bytes());
                } else {
                    panic!("unknown template variable: `{}`", ident);
                }
                index = end_index + 2;
            } else {
                output.push(source[index]);
                index += 1;
            }
        }
    }

    fn clear(&self, ops: &ClearOps) {
        unsafe {
            let mut flags = 0;
            if let Some(color) = ops.color {
                self.context.color_mask(true, true, true, true);
                ck(&self.context);
                self.context
                    .clear_color(color.r(), color.g(), color.b(), color.a());
                ck(&self.context);
                flags |= glow::COLOR_BUFFER_BIT;
            }
            if let Some(depth) = ops.depth {
                self.context.depth_mask(true);
                ck(&self.context);
                self.context.clear_depth_f32(depth as _);
                ck(&self.context); // FIXME(pcwalton): GLES
                flags |= glow::DEPTH_BUFFER_BIT;
            }
            if let Some(stencil) = ops.stencil {
                self.context.stencil_mask(!0);
                ck(&self.context);
                self.context.clear_stencil(stencil as i32);
                ck(&self.context);
                flags |= glow::STENCIL_BUFFER_BIT;
            }
            if flags != 0 {
                self.context.clear(flags);
                ck(&self.context);
            }
        }
    }
}

pub struct GLVertexArray {
    context: Arc<Context>,
    pub gl_vertex_array: <Context as HasContext>::VertexArray,
}

impl Drop for GLVertexArray {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.context.delete_vertex_array(self.gl_vertex_array);
            ck(&self.context);
        }
    }
}

pub struct GLVertexAttr {
    context: Arc<Context>,
    attr: u32,
}

impl GLVertexAttr {
    pub fn configure_float(
        &self,
        size: i32,
        gl_type: u32,
        normalized: bool,
        stride: i32,
        offset: usize,
        divisor: u32,
    ) {
        unsafe {
            self.context.vertex_attrib_pointer_f32(
                self.attr,
                size,
                gl_type,
                if normalized { true } else { false },
                stride,
                offset as i32,
            );
            ck(&self.context);
            self.context.vertex_attrib_divisor(self.attr, divisor);
            ck(&self.context);
            self.context.enable_vertex_attrib_array(self.attr);
            ck(&self.context);
        }
    }

    pub fn configure_int(&self, size: i32, gl_type: u32, stride: i32, offset: usize, divisor: u32) {
        unsafe {
            self.context
                .vertex_attrib_pointer_i32(self.attr, size, gl_type, stride, offset as i32);
            ck(&self.context);
            self.context.vertex_attrib_divisor(self.attr, divisor);
            ck(&self.context);
            self.context.enable_vertex_attrib_array(self.attr);
            ck(&self.context);
        }
    }
}

pub struct GLFramebuffer {
    context: Arc<Context>,
    pub gl_framebuffer: <Context as HasContext>::Framebuffer,
    pub texture: GLTexture,
}

impl Drop for GLFramebuffer {
    fn drop(&mut self) {
        unsafe {
            self.context.delete_framebuffer(self.gl_framebuffer);
            ck(&self.context);
        }
    }
}

pub struct GLBuffer {
    context: Arc<Context>,
    pub gl_buffer: <Context as HasContext>::Buffer,
}

impl Drop for GLBuffer {
    fn drop(&mut self) {
        unsafe {
            self.context.delete_buffer(self.gl_buffer);
            ck(&self.context);
        }
    }
}

#[derive(Debug)]
pub struct GLUniform {
    location: Option<<Context as HasContext>::UniformLocation>,
}

pub struct GLProgram {
    context: Arc<Context>,
    pub gl_program: <Context as HasContext>::Program,
    #[allow(dead_code)]
    vertex_shader: GLShader,
    #[allow(dead_code)]
    fragment_shader: GLShader,
}

impl Drop for GLProgram {
    fn drop(&mut self) {
        unsafe {
            self.context.delete_program(self.gl_program);
            ck(&self.context);
        }
    }
}

pub struct GLShader {
    context: Arc<Context>,
    gl_shader: <Context as HasContext>::Shader,
}

impl Drop for GLShader {
    fn drop(&mut self) {
        unsafe {
            self.context.delete_shader(self.gl_shader);
            ck(&self.context);
        }
    }
}

pub struct GLTexture {
    gl_texture: <Context as HasContext>::Texture,
    pub size: Vector2I,
    pub format: TextureFormat,
}

pub struct GLTimerQuery {}

trait BufferTargetExt {
    fn to_gl_target(self) -> u32;
}

impl BufferTargetExt for BufferTarget {
    fn to_gl_target(self) -> u32 {
        match self {
            BufferTarget::Vertex => glow::ARRAY_BUFFER,
            BufferTarget::Index => glow::ELEMENT_ARRAY_BUFFER,
        }
    }
}

trait BufferUploadModeExt {
    fn to_gl_usage(self) -> u32;
}

impl BufferUploadModeExt for BufferUploadMode {
    fn to_gl_usage(self) -> u32 {
        match self {
            BufferUploadMode::Static => glow::STATIC_DRAW,
            BufferUploadMode::Dynamic => glow::DYNAMIC_DRAW,
        }
    }
}

trait DepthFuncExt {
    fn to_gl_depth_func(self) -> u32;
}

impl DepthFuncExt for DepthFunc {
    fn to_gl_depth_func(self) -> u32 {
        match self {
            DepthFunc::Less => glow::LESS,
            DepthFunc::Always => glow::ALWAYS,
        }
    }
}

trait PrimitiveExt {
    fn to_gl_primitive(self) -> u32;
}

impl PrimitiveExt for Primitive {
    fn to_gl_primitive(self) -> u32 {
        match self {
            Primitive::Triangles => glow::TRIANGLES,
            Primitive::Lines => glow::LINES,
        }
    }
}

trait StencilFuncExt {
    fn to_gl_stencil_func(self) -> u32;
}

impl StencilFuncExt for StencilFunc {
    fn to_gl_stencil_func(self) -> u32 {
        match self {
            StencilFunc::Always => glow::ALWAYS,
            StencilFunc::Equal => glow::EQUAL,
        }
    }
}

trait TextureFormatExt {
    fn gl_internal_format(self) -> i32;
    fn gl_format(self) -> u32;
    fn gl_type(self) -> u32;
}

impl TextureFormatExt for TextureFormat {
    fn gl_internal_format(self) -> i32 {
        match self {
            TextureFormat::R8 => glow::R8 as i32,
            TextureFormat::R16F => glow::R16F as i32,
            TextureFormat::RGBA8 => glow::RGBA as i32,
        }
    }

    fn gl_format(self) -> u32 {
        match self {
            TextureFormat::R8 | TextureFormat::R16F => glow::RED,
            TextureFormat::RGBA8 => glow::RGBA,
        }
    }

    fn gl_type(self) -> u32 {
        match self {
            TextureFormat::R8 | TextureFormat::RGBA8 => glow::UNSIGNED_BYTE,
            TextureFormat::R16F => glow::HALF_FLOAT,
        }
    }
}

trait VertexAttrTypeExt {
    fn to_gl_type(self) -> u32;
}

impl VertexAttrTypeExt for VertexAttrType {
    fn to_gl_type(self) -> u32 {
        match self {
            VertexAttrType::F32 => glow::FLOAT,
            VertexAttrType::I16 => glow::SHORT,
            VertexAttrType::I8 => glow::BYTE,
            VertexAttrType::U16 => glow::UNSIGNED_SHORT,
            VertexAttrType::U8 => glow::UNSIGNED_BYTE,
        }
    }
}

// Error checking

#[cfg(debug_assertions)]
fn ck(context: &Context) {
    unsafe {
        // Note that ideally we should be calling glow::GetError() in a loop until it
        // returns glow::NO_ERROR, but for now we'll just report the first one we find.
        let err = context.get_error();
        if err != glow::NO_ERROR {
            panic!(
                "GL error: 0x{:x} ({})",
                err,
                match err {
                    glow::INVALID_ENUM => "INVALID_ENUM",
                    glow::INVALID_VALUE => "INVALID_VALUE",
                    glow::INVALID_OPERATION => "INVALID_OPERATION",
                    glow::INVALID_FRAMEBUFFER_OPERATION => "INVALID_FRAMEBUFFER_OPERATION",
                    glow::OUT_OF_MEMORY => "OUT_OF_MEMORY",
                    glow::STACK_UNDERFLOW => "STACK_UNDERFLOW",
                    glow::STACK_OVERFLOW => "STACK_OVERFLOW",
                    _ => "Unknown",
                }
            );
        }
    }
}

#[cfg(not(debug_assertions))]
fn ck(context: &Context) {}
