// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A GPU rasterizer for glyphs.

use atlas::Atlas;
use compute_shader::device::Device;
use compute_shader::image::{Format, Image};
use compute_shader::instance::{Instance, ShadingLanguage};
use compute_shader::profile_event::ProfileEvent;
use compute_shader::program::Program;
use compute_shader::queue::{Queue, Uniform};
use coverage::CoverageBuffer;
use error::{InitError, RasterError};
use euclid::rect::Rect;
use gl::types::{GLchar, GLenum, GLint, GLsizei, GLuint, GLvoid};
use gl;
use outline::{Outlines, Vertex};
use std::ascii::AsciiExt;
use std::env;
use std::fs::File;
use std::io::Read;
use std::mem;
use std::path::{Path, PathBuf};
use std::ptr;

static COMPUTE_PREAMBLE_FILENAME: &'static str = "preamble.cs.glsl";

static ACCUM_CL_SHADER_FILENAME: &'static str = "accum.cl";
static ACCUM_COMPUTE_SHADER_FILENAME: &'static str = "accum.cs.glsl";

static DRAW_VERTEX_SHADER_FILENAME: &'static str = "draw.vs.glsl";
static DRAW_TESS_CONTROL_SHADER_FILENAME: &'static str = "draw.tcs.glsl";
static DRAW_TESS_EVALUATION_SHADER_FILENAME: &'static str = "draw.tes.glsl";
static DRAW_GEOMETRY_SHADER_FILENAME: &'static str = "draw.gs.glsl";
static DRAW_FRAGMENT_SHADER_FILENAME: &'static str = "draw.fs.glsl";

/// A GPU rasterizer for glyphs.
pub struct Rasterizer {
    device: Device,
    queue: Queue,
    shading_language: ShadingLanguage,
    draw_program: GLuint,
    accum_program_r8: Program,
    accum_program_rgba8: Program,
    draw_vertex_array: GLuint,
    draw_position_attribute: GLint,
    draw_glyph_index_attribute: GLint,
    draw_atlas_size_uniform: GLint,
    draw_glyph_descriptors_uniform: GLuint,
    draw_image_descriptors_uniform: GLuint,
    draw_query: GLuint,
    options: RasterizerOptions,
}

/// Profiling events that can be used to profile Pathfinder's performance.
pub struct DrawAtlasProfilingEvents {
    /// An OpenGL timer query object that measures the length of time that Pathfinder took to draw
    /// the glyph edges.
    ///
    /// You can get the results with `gl::GetQueryObjectui64v(..., gl::TIME_ELAPSED, ...)`.
    pub draw: GLuint,

    /// A `compute-shader` profile event that measures the length of time that Pathfinder took to
    /// perform the accumulation (fill) step.
    pub accum: ProfileEvent,
}

impl Rasterizer {
    /// Creates a new rasterizer.
    ///
    /// This rasterizer can be used for as many draw calls as you like.
    ///
    /// * `instance` is the `compute-shader` instance to use.
    ///
    /// * `device` is the compute device to use.
    ///
    /// * `queue` is the queue on that compute device to use.
    ///
    /// * `options` is a set of options that control the rasterizer's behavior.
    pub fn new(instance: &Instance, device: Device, queue: Queue, options: RasterizerOptions)
               -> Result<Rasterizer, InitError> {
        let (draw_program, draw_position_attribute, draw_glyph_index_attribute);
        let (draw_glyph_descriptors_uniform, draw_image_descriptors_uniform);
        let draw_atlas_size_uniform;
        let (mut draw_vertex_array, mut draw_query) = (0, 0);
        unsafe {
            draw_program = gl::CreateProgram();

            let vertex_shader = try!(compile_gl_shader(gl::VERTEX_SHADER,
                                                       "Vertex shader",
                                                       DRAW_VERTEX_SHADER_FILENAME,
                                                       &options.shader_path));
            gl::AttachShader(draw_program, vertex_shader);
            let fragment_shader = try!(compile_gl_shader(gl::FRAGMENT_SHADER,
                                                         "Fragment shader",
                                                         DRAW_FRAGMENT_SHADER_FILENAME,
                                                         &options.shader_path));
            gl::AttachShader(draw_program, fragment_shader);

            if options.force_geometry_shader {
                let geometry_shader = try!(compile_gl_shader(gl::GEOMETRY_SHADER,
                                                             "Geometry shader",
                                                             DRAW_GEOMETRY_SHADER_FILENAME,
                                                             &options.shader_path));
                gl::AttachShader(draw_program, geometry_shader);
            } else {
                let tess_control_shader = try!(compile_gl_shader(gl::TESS_CONTROL_SHADER,
                                                                 "Tessellation control shader",
                                                                 DRAW_TESS_CONTROL_SHADER_FILENAME,
                                                                 &options.shader_path));
                gl::AttachShader(draw_program, tess_control_shader);
                let tess_evaluation_shader =
                    try!(compile_gl_shader(gl::TESS_EVALUATION_SHADER,
                                           "Tessellation evaluation shader",
                                           DRAW_TESS_EVALUATION_SHADER_FILENAME,
                                           &options.shader_path));
                gl::AttachShader(draw_program, tess_evaluation_shader);
            }

            gl::LinkProgram(draw_program);

            try!(check_gl_object_status(draw_program,
                                        gl::LINK_STATUS,
                                        gl::GetProgramiv,
                                        gl::GetProgramInfoLog).map_err(InitError::LinkFailed));

            gl::GenVertexArrays(1, &mut draw_vertex_array);

            draw_position_attribute =
                gl::GetAttribLocation(draw_program, b"aPosition\0".as_ptr() as *const GLchar);
            draw_glyph_index_attribute =
                gl::GetAttribLocation(draw_program, b"aGlyphIndex\0".as_ptr() as *const GLchar);

            draw_atlas_size_uniform =
                gl::GetUniformLocation(draw_program, b"uAtlasSize\0".as_ptr() as *const GLchar);
            draw_glyph_descriptors_uniform =
                gl::GetUniformBlockIndex(draw_program,
                                         b"ubGlyphDescriptors\0".as_ptr() as *const GLchar);
            draw_image_descriptors_uniform =
                gl::GetUniformBlockIndex(draw_program,
                                         b"ubImageDescriptors\0".as_ptr() as *const GLchar);

            gl::GenQueries(1, &mut draw_query)
        }

        // FIXME(pcwalton): Don't panic if this fails to compile; just return an error.
        let shading_language = instance.shading_language();
        let accum_filename = match shading_language {
            ShadingLanguage::Cl => ACCUM_CL_SHADER_FILENAME,
            ShadingLanguage::Glsl => ACCUM_COMPUTE_SHADER_FILENAME,
        };

        let mut accum_path = options.shader_path.to_owned();
        accum_path.push(accum_filename);
        let mut accum_file = match File::open(&accum_path) {
            Err(error) => return Err(InitError::ShaderUnreadable(error)),
            Ok(file) => file,
        };

        let mut compute_preamble_source = String::new();
        match shading_language {
            ShadingLanguage::Cl => {}
            ShadingLanguage::Glsl => {
                let mut compute_preamble_path = options.shader_path.to_owned();
                compute_preamble_path.push(COMPUTE_PREAMBLE_FILENAME);
                let mut compute_preamble_file = match File::open(&compute_preamble_path) {
                    Err(error) => return Err(InitError::ShaderUnreadable(error)),
                    Ok(file) => file,
                };

                if compute_preamble_file.read_to_string(&mut compute_preamble_source).is_err() {
                    return Err(InitError::CompileFailed("Compute shader",
                                                        "Invalid UTF-8".to_string()))
                }
            }
        }

        let mut accum_source = String::new();
        if accum_file.read_to_string(&mut accum_source).is_err() {
            return Err(InitError::CompileFailed("Compute shader", "Invalid UTF-8".to_string()))
        }

        let accum_source_r8 = format!("{}\n#define IMAGE_FORMAT r8\n{}",
                                      compute_preamble_source,
                                      accum_source);
        let accum_source_rgba8 = format!("{}\n#define IMAGE_FORMAT rgba8\n{}",
                                         compute_preamble_source,
                                         accum_source);

        let accum_program_r8 = try!(device.create_program(&accum_source_r8)
                                          .map_err(InitError::ComputeError));
        let accum_program_rgba8 = try!(device.create_program(&accum_source_rgba8)
                                             .map_err(InitError::ComputeError));

        Ok(Rasterizer {
            device: device,
            queue: queue,
            shading_language: shading_language,
            draw_program: draw_program,
            accum_program_r8: accum_program_r8,
            accum_program_rgba8: accum_program_rgba8,
            draw_vertex_array: draw_vertex_array,
            draw_position_attribute: draw_position_attribute,
            draw_glyph_index_attribute: draw_glyph_index_attribute,
            draw_atlas_size_uniform: draw_atlas_size_uniform,
            draw_glyph_descriptors_uniform: draw_glyph_descriptors_uniform,
            draw_image_descriptors_uniform: draw_image_descriptors_uniform,
            draw_query: draw_query,
            options: options,
        })
    }

    /// Draws the supplied font atlas into the texture image at the given location.
    ///
    /// * `image` is the texture image that this rasterizer will draw into.
    ///
    /// * `rect` is the pixel boundaries of the atlas inside that image.
    ///
    /// * `atlas` is the glyph atlas to render.
    ///
    /// * `outlines` specifies the outlines for the font associated with that atlas.
    ///
    /// * `coverage_buffer` is a coverage buffer to use (see `CoverageBuffer`). This can be reused
    ///   from call to call. It must be at least as large as the atlas.
    pub fn draw_atlas(&self,
                      image: &Image,
                      rect: &Rect<u32>,
                      atlas: &Atlas,
                      outlines: &Outlines,
                      coverage_buffer: &CoverageBuffer)
                      -> Result<DrawAtlasProfilingEvents, RasterError> {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, coverage_buffer.framebuffer());
            gl::Viewport(0, 0, rect.size.width as GLint, rect.size.height as GLint);

            // TODO(pcwalton): Scissor to the image rect to clear faster?
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            gl::BindVertexArray(self.draw_vertex_array);
            gl::UseProgram(self.draw_program);

            // Set up the buffer layout.
            gl::BindBuffer(gl::ARRAY_BUFFER, outlines.vertices_buffer());
            gl::VertexAttribIPointer(self.draw_position_attribute as GLuint,
                                     2,
                                     gl::SHORT,
                                     mem::size_of::<Vertex>() as GLint,
                                     0 as *const GLvoid);
            gl::VertexAttribIPointer(self.draw_glyph_index_attribute as GLuint,
                                     1,
                                     gl::UNSIGNED_SHORT,
                                     mem::size_of::<Vertex>() as GLint,
                                     mem::size_of::<(i16, i16)>() as *const GLvoid);
            gl::EnableVertexAttribArray(self.draw_position_attribute as GLuint);
            gl::EnableVertexAttribArray(self.draw_glyph_index_attribute as GLuint);

            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, outlines.indices_buffer());

            // Don't bind the atlas uniform buffers (binding point 2) here; the batches will do
            // that on their own.
            gl::BindBufferBase(gl::UNIFORM_BUFFER, 1, outlines.descriptors_buffer());
            gl::UniformBlockBinding(self.draw_program, self.draw_glyph_descriptors_uniform, 1);
            gl::UniformBlockBinding(self.draw_program, self.draw_image_descriptors_uniform, 2);

            gl::Uniform2ui(self.draw_atlas_size_uniform, rect.size.width, rect.size.height);

            gl::PatchParameteri(gl::PATCH_VERTICES, 3);

            // Use blending on our floating point framebuffer to accumulate coverage.
            gl::Enable(gl::BLEND);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::ONE, gl::ONE);

            // Enable backface culling. See comments in `draw.tcs.glsl` for more information
            // regarding why this is necessary.
            gl::CullFace(gl::BACK);
            gl::FrontFace(gl::CCW);
            gl::Enable(gl::CULL_FACE);

            // If we're using a geometry shader for debugging, we draw fake triangles. Otherwise,
            // we use patches.
            let primitive = if self.options.force_geometry_shader {
                gl::TRIANGLES
            } else {
                gl::PATCHES
            };

            // Now draw the glyph ranges.
            gl::BeginQuery(gl::TIME_ELAPSED, self.draw_query);
            atlas.draw(primitive);
            gl::EndQuery(gl::TIME_ELAPSED);

            gl::Disable(gl::CULL_FACE);
            gl::Disable(gl::BLEND);

            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);

            // FIXME(pcwalton): We should have some better synchronization here if we're using
            // OpenCL, but I don't know how to do that portably (i.e. on Mac…) Just using
            // `glFlush()` seems to work in practice.
            gl::Flush();

            if self.shading_language == ShadingLanguage::Glsl {
                gl::MemoryBarrier(gl::ALL_BARRIER_BITS);
            }
        }

        let accum_uniforms = [
            (0, Uniform::Image(image)),
            (1, Uniform::Image(coverage_buffer.image())),
            (2, Uniform::UVec4([rect.origin.x, rect.origin.y, rect.max_x(), rect.max_y()])),
            (3, Uniform::U32(atlas.shelf_height())),
        ];

        let accum_program = match image.format() {
            Ok(Format::R8) => &self.accum_program_r8,
            Ok(Format::RGBA8) => &self.accum_program_rgba8,
            Ok(_) => return Err(RasterError::UnsupportedImageFormat),
            Err(err) => return Err(RasterError::ComputeError(err)),
        };

        let accum_event = try!(self.queue.submit_compute(accum_program,
                                                         &[atlas.shelf_columns()],
                                                         &accum_uniforms,
                                                         &[]).map_err(RasterError::ComputeError));

        Ok(DrawAtlasProfilingEvents {
            draw: self.draw_query,
            accum: accum_event,
        })
    }

    /// Returns the GPU compute device that this rasterizer is using.
    #[inline]
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Returns the GPU compute queue that this rasterizer is using.
    #[inline]
    pub fn queue(&self) -> &Queue {
        &self.queue
    }
}

fn compile_gl_shader(shader_type: GLuint,
                     description: &'static str,
                     filename: &str,
                     shader_path: &Path)
                     -> Result<GLuint, InitError> {
    unsafe {
        let mut path = shader_path.to_owned();
        path.push(filename);

        let mut file = match File::open(&path) {
            Err(error) => return Err(InitError::ShaderUnreadable(error)),
            Ok(file) => file,
        };

        let mut source = String::new();
        if file.read_to_string(&mut source).is_err() {
            return Err(InitError::CompileFailed(description, "Invalid UTF-8".to_string()))
        }

        let shader = gl::CreateShader(shader_type);
        gl::ShaderSource(shader, 1, &(source.as_ptr() as *const GLchar), &(source.len() as GLint));
        gl::CompileShader(shader);
        match check_gl_object_status(shader,
                                     gl::COMPILE_STATUS,
                                     gl::GetShaderiv,
                                     gl::GetShaderInfoLog) {
            Ok(_) => Ok(shader),
            Err(info_log) => Err(InitError::CompileFailed(description, info_log)),
        }
    }
}

fn check_gl_object_status(object: GLuint,
                          parameter: GLenum,
                          get_status: unsafe fn(GLuint, GLenum, *mut GLint),
                          get_log: unsafe fn(GLuint, GLsizei, *mut GLsizei, *mut GLchar))
                          -> Result<(), String> {
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

        match String::from_utf8(info_log) {
            Ok(string) => Err(string),
            Err(_) => Err("(not UTF-8)".to_owned()),
        }
    }
}

/// Options that control Pathfinder's behavior.
#[derive(Clone, Debug)]
pub struct RasterizerOptions {
    /// The path to the shaders.
    ///
    /// If not specified, then the current directory is used. This is probably not what you want.
    /// The corresponding environment variable is `PATHFINDER_SHADER_PATH`.
    pub shader_path: PathBuf,
    /// If true, then a geometry shader is used instead of a tessellation shader.
    ///
    /// This will probably negatively impact performance. This should be considered a debugging
    /// feature only.
    ///
    /// The default is false. The corresponding environment variable is
    /// `PATHFINDER_FORCE_GEOMETRY_SHADER`.
    pub force_geometry_shader: bool,
}

impl Default for RasterizerOptions {
    fn default() -> RasterizerOptions {
        RasterizerOptions {
            shader_path: PathBuf::from("."),
            force_geometry_shader: false,
        }
    }
}

impl RasterizerOptions {
    /// Takes rasterization options from environment variables.
    ///
    /// See the fields of `RasterizerOptions` for info on the settings, including the environment
    /// variables that control them.
    ///
    /// Boolean variables may be set to true by setting the corresponding variable to `"on"`,
    /// `"yes"`, or `1`; they may be set to false with `"off"`, `"no"`, or `0`.
    ///
    /// Environment variables not set cause their associated settings to take on default values.
    pub fn from_env() -> Result<RasterizerOptions, InitError> {
        let shader_path = match env::var("PATHFINDER_SHADER_PATH") {
            Ok(ref string) => PathBuf::from(string),
            Err(_) => PathBuf::from("."),
        };

        let force_geometry_shader = match env::var("PATHFINDER_FORCE_GEOMETRY_SHADER") {
            Ok(ref string) if string.eq_ignore_ascii_case("on") ||
                string.eq_ignore_ascii_case("yes") ||
                string.eq_ignore_ascii_case("1") => true,
            Ok(ref string) if string.eq_ignore_ascii_case("off") ||
                string.eq_ignore_ascii_case("no") ||
                string.eq_ignore_ascii_case("0") => false,
            Err(_) => false,
            Ok(_) => return Err(InitError::InvalidSetting),
        };

        Ok(RasterizerOptions {
            shader_path: shader_path,
            force_geometry_shader: force_geometry_shader,
        })
    }
}

