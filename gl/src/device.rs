// pathfinder/demo/src/device.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Minimal abstractions over GPU device capabilities.

use gl::types::{GLchar, GLint, GLsizei, GLsizeiptr, GLuint, GLvoid};
use pathfinder_geometry::basic::point::Point2DI32;
use std::env;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::mem;
use std::path::PathBuf;
use std::ptr;

pub struct Device {
    pub resources_directory: PathBuf,
}

impl Device {
    #[inline]
    pub fn new() -> Device {
        Device { resources_directory: locate_resources_directory() }
    }

    #[inline]
    pub fn create_texture_from_png(&self, name: &str) -> Texture {
        let mut path = self.resources_directory.clone();
        path.push("textures");
        path.push(format!("{}.png", name));

        let image = image::open(&path).unwrap().to_luma();

        let mut texture = Texture {
            gl_texture: 0,
            size: Point2DI32::new(image.width() as i32, image.height() as i32),
        };

        unsafe {
            gl::GenTextures(1, &mut texture.gl_texture);
            texture.bind(0);
            gl::TexImage2D(gl::TEXTURE_2D,
                           0,
                           gl::RED as GLint,
                           image.width() as GLsizei,
                           image.height() as GLsizei,
                           0,
                           gl::RED,
                           gl::UNSIGNED_BYTE,
                           image.as_ptr() as *const GLvoid);
        }

        texture.set_parameters();
        texture
    }

    fn create_shader(&self, name: &str, kind: ShaderKind) -> Shader {
        let suffix = match kind { ShaderKind::Vertex => 'v', ShaderKind::Fragment => 'f' };
        let mut path = self.resources_directory.clone();
        path.push("shaders");
        path.push(format!("{}.{}s.glsl", name, suffix));

        let mut source = vec![];
        File::open(&path).unwrap().read_to_end(&mut source).unwrap();
        unsafe {
            let gl_shader_kind = match kind {
                ShaderKind::Vertex => gl::VERTEX_SHADER,
                ShaderKind::Fragment => gl::FRAGMENT_SHADER,
            };
            let gl_shader = gl::CreateShader(gl_shader_kind);
            gl::ShaderSource(gl_shader,
                             1,
                             [source.as_ptr() as *const GLchar].as_ptr(),
                             [source.len() as GLint].as_ptr());
            gl::CompileShader(gl_shader);

            let mut compile_status = 0;
            gl::GetShaderiv(gl_shader, gl::COMPILE_STATUS, &mut compile_status);
            if compile_status != gl::TRUE as GLint {
                let mut info_log_length = 0;
                gl::GetShaderiv(gl_shader, gl::INFO_LOG_LENGTH, &mut info_log_length);
                let mut info_log = vec![0; info_log_length as usize];
                gl::GetShaderInfoLog(gl_shader,
                                     info_log.len() as GLint,
                                     ptr::null_mut(),
                                     info_log.as_mut_ptr() as *mut GLchar);
                eprintln!("Shader info log:\n{}", String::from_utf8_lossy(&info_log));
                panic!("{:?} shader '{}' compilation failed", kind, name);
            }

            Shader { gl_shader }
        }
    }

    pub fn create_program(&self, name: &str) -> Program {
        let vertex_shader = self.create_shader(name, ShaderKind::Vertex);
        let fragment_shader = self.create_shader(name, ShaderKind::Fragment);

        let gl_program;
        unsafe {
            gl_program = gl::CreateProgram();
            gl::AttachShader(gl_program, vertex_shader.gl_shader);
            gl::AttachShader(gl_program, fragment_shader.gl_shader);
            gl::LinkProgram(gl_program);

            let mut link_status = 0;
            gl::GetProgramiv(gl_program, gl::LINK_STATUS, &mut link_status);
            if link_status != gl::TRUE as GLint {
                let mut info_log_length = 0;
                gl::GetProgramiv(gl_program, gl::INFO_LOG_LENGTH, &mut info_log_length);
                let mut info_log = vec![0; info_log_length as usize];
                gl::GetProgramInfoLog(gl_program,
                                      info_log.len() as GLint,
                                      ptr::null_mut(),
                                      info_log.as_mut_ptr() as *mut GLchar);
                eprintln!("Program info log:\n{}", String::from_utf8_lossy(&info_log));
                panic!("Program '{}' linking failed", name);
            }
        }

        Program { gl_program, vertex_shader, fragment_shader }
    }
}

pub struct VertexArray {
    pub gl_vertex_array: GLuint,
}

impl Drop for VertexArray {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &mut self.gl_vertex_array);
        }
    }
}

impl VertexArray {
    #[inline]
    pub fn new() -> VertexArray {
        unsafe {
            let mut array = VertexArray { gl_vertex_array: 0 };
            gl::GenVertexArrays(1, &mut array.gl_vertex_array);
            array
        }
    }
}

pub struct VertexAttr {
    attr: GLuint,
}

impl VertexAttr {
    pub fn new(program: &Program, name: &str) -> VertexAttr {
        let name = CString::new(format!("a{}", name)).unwrap();
        let attr = unsafe {
            gl::GetAttribLocation(program.gl_program, name.as_ptr() as *const GLchar) as GLuint
        };
        VertexAttr { attr }
    }

    pub fn configure_float(&self,
                           size: GLint,
                           gl_type: GLuint,
                           normalized: bool,
                           stride: GLsizei,
                           offset: usize,
                           divisor: GLuint) {
        unsafe {
            gl::VertexAttribPointer(self.attr,
                                    size,
                                    gl_type,
                                    if normalized { gl::TRUE } else { gl::FALSE },
                                    stride,
                                    offset as *const GLvoid);
            gl::VertexAttribDivisor(self.attr, divisor);
            gl::EnableVertexAttribArray(self.attr);
        }
    }

    pub fn configure_int(&self,
                         size: GLint,
                         gl_type: GLuint,
                         stride: GLsizei,
                         offset: usize,
                         divisor: GLuint) {
        unsafe {
            gl::VertexAttribIPointer(self.attr, size, gl_type, stride, offset as *const GLvoid);
            gl::VertexAttribDivisor(self.attr, divisor);
            gl::EnableVertexAttribArray(self.attr);
        }
    }
}

pub struct Framebuffer {
    pub gl_framebuffer: GLuint,
    pub texture: Texture,
}

impl Framebuffer {
    pub fn new(texture: Texture) -> Framebuffer {
        let mut gl_framebuffer = 0;
        unsafe {
            gl::GenFramebuffers(1, &mut gl_framebuffer);
            assert_eq!(gl::GetError(), gl::NO_ERROR);
            gl::BindFramebuffer(gl::FRAMEBUFFER, gl_framebuffer);
            texture.bind(0);
            gl::FramebufferTexture2D(gl::FRAMEBUFFER,
                                     gl::COLOR_ATTACHMENT0,
                                     gl::TEXTURE_2D,
                                     texture.gl_texture,
                                     0);
            assert_eq!(gl::CheckFramebufferStatus(gl::FRAMEBUFFER), gl::FRAMEBUFFER_COMPLETE);
        }
        Framebuffer { gl_framebuffer, texture }
    }

    pub fn bind(&self) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.gl_framebuffer);
        }
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(1, &mut self.gl_framebuffer)
        }
    }
}

pub struct Buffer {
    pub gl_buffer: GLuint,
}

impl Buffer {
    pub fn new() -> Buffer {
        unsafe {
            let mut gl_buffer = 0;
            gl::GenBuffers(1, &mut gl_buffer);
            Buffer { gl_buffer }
        }
    }

    pub fn upload<T>(&self, data: &[T], target: BufferTarget, mode: BufferUploadMode) {
        let target = match target {
            BufferTarget::Vertex => gl::ARRAY_BUFFER,
            BufferTarget::Index => gl::ELEMENT_ARRAY_BUFFER,
        };
        let mode = match mode {
            BufferUploadMode::Static => gl::STATIC_DRAW,
            BufferUploadMode::Dynamic => gl::DYNAMIC_DRAW,
        };
        unsafe {
            gl::BindBuffer(target, self.gl_buffer);
            gl::BufferData(target,
                           (data.len() * mem::size_of::<T>()) as GLsizeiptr,
                           data.as_ptr() as *const GLvoid,
                           mode);
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &mut self.gl_buffer)
        }
    }
}

pub enum BufferTarget {
    Vertex,
    Index,
}

pub enum BufferUploadMode {
    Static,
    Dynamic,
}

#[derive(Debug)]
pub struct Uniform {
    pub location: GLint,
}

impl Uniform {
    pub fn new(program: &Program, name: &str) -> Uniform {
        let name = CString::new(format!("u{}", name)).unwrap();
        let location = unsafe {
            gl::GetUniformLocation(program.gl_program, name.as_ptr() as *const GLchar)
        };
        Uniform { location }
    }
}

pub struct Program {
    pub gl_program: GLuint,
    #[allow(dead_code)]
    vertex_shader: Shader,
    #[allow(dead_code)]
    fragment_shader: Shader,
}

impl Drop for Program {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.gl_program)
        }
    }
}

struct Shader {
    gl_shader: GLuint,
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteShader(self.gl_shader)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ShaderKind {
    Vertex,
    Fragment,
}

pub struct Texture {
    gl_texture: GLuint,
    pub size: Point2DI32,
}

impl Texture {
    pub fn new_r16f(size: Point2DI32) -> Texture {
        let mut texture = Texture { gl_texture: 0, size };
        unsafe {
            gl::GenTextures(1, &mut texture.gl_texture);
            texture.bind(0);
            gl::TexImage2D(gl::TEXTURE_2D,
                           0,
                           gl::R16F as GLint,
                           size.x() as GLsizei,
                           size.y() as GLsizei,
                           0,
                           gl::RED,
                           gl::HALF_FLOAT,
                           ptr::null());
        }

        texture.set_parameters();
        texture
    }

    pub fn new_rgba(size: Point2DI32) -> Texture {
        let mut texture = Texture { gl_texture: 0, size };
        unsafe {
            gl::GenTextures(1, &mut texture.gl_texture);
            texture.bind(0);
            gl::TexImage2D(gl::TEXTURE_2D,
                           0,
                           gl::RGBA as GLint,
                           size.x() as GLsizei,
                           size.y() as GLsizei,
                           0,
                           gl::RGBA,
                           gl::UNSIGNED_BYTE,
                           ptr::null());
        }

        texture.set_parameters();
        texture
    }

    pub fn bind(&self, unit: u32) {
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0 + unit);
            gl::BindTexture(gl::TEXTURE_2D, self.gl_texture);
        }
    }

    pub fn upload_rgba(&self, size: Point2DI32, data: &[u8]) {
        assert!(data.len() >= size.x() as usize * size.y() as usize * 4);
        unsafe {
            self.bind(0);
            gl::TexImage2D(gl::TEXTURE_2D,
                           0,
                           gl::RGBA as GLint,
                           size.x() as GLsizei,
                           size.y() as GLsizei,
                           0,
                           gl::RGBA,
                           gl::UNSIGNED_BYTE,
                           data.as_ptr() as *const GLvoid);
        }

        self.set_parameters();
    }

    fn set_parameters(&self) {
        self.bind(0);
        unsafe {
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);
        }
    }
}

pub struct TimerQuery {
    gl_query: GLuint,
}

impl Drop for TimerQuery {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            gl::DeleteQueries(1, &mut self.gl_query);
        }
    }
}

impl TimerQuery {
    #[inline]
    pub fn new() -> TimerQuery {
        let mut query = TimerQuery { gl_query: 0 };
        unsafe {
            gl::GenQueries(1, &mut query.gl_query);
        }
        query
    }

    #[inline]
    pub fn begin(&self) {
        unsafe {
            gl::BeginQuery(gl::TIME_ELAPSED, self.gl_query);
        }
    }

    #[inline]
    pub fn end(&self) {
        unsafe {
            gl::EndQuery(gl::TIME_ELAPSED);
        }
    }

    #[inline]
    pub fn is_available(&self) -> bool {
        unsafe {
            let mut result = 0;
            gl::GetQueryObjectiv(self.gl_query, gl::QUERY_RESULT_AVAILABLE, &mut result);
            result != gl::FALSE as GLint
        }
    }

    #[inline]
    pub fn get(&self) -> u64 {
        unsafe {
            let mut result = 0;
            gl::GetQueryObjectui64v(self.gl_query, gl::QUERY_RESULT, &mut result);
            result
        }
    }
}

// FIXME(pcwalton): Do something better!
fn locate_resources_directory() -> PathBuf {
    let mut parent_directory = env::current_dir().unwrap();
    loop {
        // So ugly :(
        let mut resources_directory = parent_directory.clone();
        resources_directory.push("resources");
        if resources_directory.is_dir() {
            let mut shaders_directory = resources_directory.clone();
            let mut textures_directory = resources_directory.clone();
            shaders_directory.push("shaders");
            textures_directory.push("textures");
            if shaders_directory.is_dir() && textures_directory.is_dir() {
                return resources_directory;
            }
        }

        if !parent_directory.pop() {
            break;
        }
    }

    panic!("No suitable `resources/` directory found!");
}
