// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use batch::Batch;
use compute_shader::device::Device;
use compute_shader::event::Event;
use compute_shader::program::Program;
use compute_shader::queue::{Queue, Uniform};
use compute_shader::texture::Texture;
use coverage::CoverageBuffer;
use euclid::rect::Rect;
use gl::types::{GLchar, GLenum, GLint, GLsizei, GLuint};
use gl;
use glyph_buffer::GlyphBuffers;
use std::ptr;

// TODO(pcwalton): Don't force that these be compiled in.
// TODO(pcwalton): GLSL version.
static ACCUM_CL_SHADER: &'static str = include_str!("../resources/shaders/accum.cl");

static DRAW_VERTEX_SHADER: &'static str = include_str!("../resources/shaders/draw.vs.glsl");
static DRAW_TESS_CONTROL_SHADER: &'static str = include_str!("../resources/shaders/draw.tcs.glsl");
static DRAW_TESS_EVALUATION_SHADER: &'static str =
    include_str!("../resources/shaders/draw.tes.glsl");
static DRAW_FRAGMENT_SHADER: &'static str = include_str!("../resources/shaders/draw.fs.glsl");

pub struct Rasterizer {
    pub device: Device,
    pub queue: Queue,
    draw_program: GLuint,
    accum_program: Program,
}

impl Rasterizer {
    pub fn new(device: Device, queue: Queue) -> Result<Rasterizer, ()> {
        let draw_program;
        unsafe {
            let shaders = [
                try!(compile_gl_shader(gl::VERTEX_SHADER,
                                       "Vertex shader",
                                       DRAW_VERTEX_SHADER)),
                try!(compile_gl_shader(gl::TESS_CONTROL_SHADER,
                                       "Tessellation control shader",
                                       DRAW_TESS_CONTROL_SHADER)),
                try!(compile_gl_shader(gl::TESS_EVALUATION_SHADER,
                                       "Tessellation evaluation shader",
                                       DRAW_TESS_EVALUATION_SHADER)),
                try!(compile_gl_shader(gl::FRAGMENT_SHADER,
                                       "Fragment shader",
                                       DRAW_FRAGMENT_SHADER)),
            ];

            draw_program = gl::CreateProgram();
            for &shader in &shaders {
                gl::AttachShader(draw_program, shader);
            }

            gl::LinkProgram(draw_program);

            try!(check_gl_object_status(draw_program,
                                        gl::LINK_STATUS,
                                        "Program",
                                        gl::GetProgramiv,
                                        gl::GetProgramInfoLog))
        }

        // FIXME(pcwalton): Don't panic if this fails to compile; just return an error.
        let accum_program = device.create_program(ACCUM_CL_SHADER).unwrap();

        Ok(Rasterizer {
            device: device,
            queue: queue,
            draw_program: draw_program,
            accum_program: accum_program,
        })
    }

    pub fn draw_atlas(&self,
                      atlas_rect: &Rect<u32>,
                      atlas_shelf_height: u32,
                      glyph_buffers: &GlyphBuffers,
                      batch: &Batch,
                      coverage_buffer: &CoverageBuffer,
                      texture: &Texture)
                      -> Result<Event, ()> {
        // TODO(pcwalton)

        let atlas_rect_uniform = [
            atlas_rect.origin.x,
            atlas_rect.origin.y,
            atlas_rect.max_x(),
            atlas_rect.max_y()
        ];

        let accum_uniforms = [
            (0, Uniform::Texture(texture)),
            (1, Uniform::Texture(&coverage_buffer.texture)),
            (2, Uniform::UVec4(atlas_rect_uniform)),
            (3, Uniform::U32(atlas_shelf_height)),
        ];

        let accum_columns = atlas_rect.size.width * (atlas_rect.size.height / atlas_shelf_height);

        self.queue.submit_compute(&self.accum_program,
                                  &[accum_columns],
                                  &accum_uniforms,
                                  &[]).map_err(drop)
    }
}

fn compile_gl_shader(shader_type: GLuint, description: &str, source: &str) -> Result<GLuint, ()> {
    unsafe {
        let shader = gl::CreateShader(shader_type);
        gl::ShaderSource(shader, 1, &(source.as_ptr() as *const GLchar), &(source.len() as GLint));
        gl::CompileShader(shader);
        try!(check_gl_object_status(shader,
                                    gl::COMPILE_STATUS,
                                    description,
                                    gl::GetShaderiv,
                                    gl::GetShaderInfoLog));
        Ok(shader)
    }
}

fn check_gl_object_status(object: GLuint,
                          parameter: GLenum,
                          description: &str,
                          get_status: unsafe fn(GLuint, GLenum, *mut GLint),
                          get_log: unsafe fn(GLuint, GLsizei, *mut GLsizei, *mut GLchar))
                          -> Result<(), ()> {
    unsafe {
        let mut status = 0;
        get_status(object, parameter, &mut status);
        if status == gl::TRUE as i32 {
            return Ok(())
        }

        let mut info_log_length = 0;
        get_status(object, gl::INFO_LOG_LENGTH, &mut info_log_length);

        let mut info_log = vec![0; info_log_length as usize];
        get_log(object, info_log_length, ptr::null_mut(), info_log.as_mut_ptr() as *mut GLchar);
        if let Ok(string) = String::from_utf8(info_log) {
            println!("{} error:\n{}", description, string);
        }
        Err(())
    }
}

