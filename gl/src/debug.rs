// pathfinder/gl/src/debug.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A debug overlay.
//!
//! We don't render the demo UI text using Pathfinder itself so that we can use the debug UI to
//! debug Pathfinder if it's totally busted.
//!
//! The debug font atlas was generated using: https://evanw.github.io/font-texture-generator/

use crate::device::{Buffer, BufferTarget, BufferUploadMode, Program, Texture, Uniform, VertexAttr};
use euclid::{Point2D, Rect, Size2D};
use gl::types::{GLfloat, GLint, GLsizei, GLuint};
use gl;
use pathfinder_renderer::paint::ColorU;
use serde_json;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::ptr;
use std::time::Duration;

const DEBUG_FONT_VERTEX_SIZE:  GLint = 8;
const DEBUG_SOLID_VERTEX_SIZE: GLint = 4;

const WINDOW_WIDTH: i16 = 400;
const WINDOW_HEIGHT: i16 = LINE_HEIGHT * 2 + PADDING + 2;
const PADDING: i16 = 12;
const FONT_ASCENT: i16 = 28;
const LINE_HEIGHT: i16 = 42;

static WINDOW_COLOR: ColorU = ColorU { r: 30, g: 30, b: 30, a: 255 - 30 };

static JSON_PATH: &'static str = "resources/debug-font.json";
static PNG_NAME: &'static str = "debug-font";

static QUAD_INDICES: [u32; 6] = [0, 1, 3, 1, 2, 3];

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct DebugFont {
    name: String,
    size: i32,
    bold: bool,
    italic: bool,
    width: u32,
    height: u32,
    characters: HashMap<char, DebugCharacter>,
}

#[derive(Deserialize)]
struct DebugCharacter {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    #[serde(rename = "originX")]
    origin_x: i16,
    #[serde(rename = "originY")]
    origin_y: i16,
    advance: i16,
}

impl DebugFont {
    fn load() -> DebugFont {
        serde_json::from_reader(BufReader::new(File::open(JSON_PATH).unwrap())).unwrap()
    }
}

pub struct DebugRenderer {
    framebuffer_size: Size2D<u32>,
    font_program: DebugFontProgram,
    font_vertex_array: DebugFontVertexArray,
    font_texture: Texture,
    font: DebugFont,
    solid_program: DebugSolidProgram,
    solid_vertex_array: DebugSolidVertexArray,
}

impl DebugRenderer {
    pub fn new(framebuffer_size: &Size2D<u32>) -> DebugRenderer {
        let font_program = DebugFontProgram::new();
        let font_vertex_array = DebugFontVertexArray::new(&font_program);
        let font_texture = Texture::from_png(PNG_NAME);
        let font = DebugFont::load();

        let solid_program = DebugSolidProgram::new();
        let solid_vertex_array = DebugSolidVertexArray::new(&solid_program);
        solid_vertex_array.index_buffer.upload(&QUAD_INDICES,
                                               BufferTarget::Index,
                                               BufferUploadMode::Static);

        DebugRenderer {
            framebuffer_size: *framebuffer_size,
            font_program,
            font_vertex_array,
            font_texture,
            font,
            solid_program,
            solid_vertex_array,
        }
    }

    pub fn set_framebuffer_size(&mut self, window_size: &Size2D<u32>) {
        self.framebuffer_size = *window_size;
    }

    pub fn draw(&self, tile_time: Duration, rendering_time: Option<Duration>) {
        let window_rect =
            Rect::new(Point2D::new(self.framebuffer_size.width as i16 - PADDING - WINDOW_WIDTH,
                                   self.framebuffer_size.height as i16 - PADDING - WINDOW_HEIGHT),
                      Size2D::new(WINDOW_WIDTH, WINDOW_HEIGHT));
        self.draw_solid_rect(&window_rect, WINDOW_COLOR);
        self.draw_text(&format!("Tiling: {:.3} ms", duration_ms(tile_time)),
                       &Point2D::new(window_rect.origin.x + PADDING,
                                     window_rect.origin.y + PADDING + FONT_ASCENT));
        if let Some(rendering_time) = rendering_time {
            self.draw_text(&format!("Rendering: {:.3} ms", duration_ms(rendering_time)),
                           &Point2D::new(
                               window_rect.origin.x + PADDING,
                               window_rect.origin.y + PADDING + FONT_ASCENT + LINE_HEIGHT));
        }
    }

    fn draw_solid_rect(&self, rect: &Rect<i16>, color: ColorU) {
        let vertex_data = [
            DebugSolidVertex::new(rect.origin),
            DebugSolidVertex::new(rect.top_right()),
            DebugSolidVertex::new(rect.bottom_right()),
            DebugSolidVertex::new(rect.bottom_left()),
        ];
        self.solid_vertex_array
            .vertex_buffer
            .upload(&vertex_data, BufferTarget::Vertex, BufferUploadMode::Dynamic);

        unsafe {
            gl::BindVertexArray(self.solid_vertex_array.gl_vertex_array);
            gl::UseProgram(self.solid_program.program.gl_program);
            gl::Uniform2f(self.solid_program.framebuffer_size_uniform.location,
                          self.framebuffer_size.width as GLfloat,
                          self.framebuffer_size.height as GLfloat);
            gl::Uniform4f(self.solid_program.color_uniform.location,
                          color.r as f32 * (1.0 / 255.0),
                          color.g as f32 * (1.0 / 255.0),
                          color.b as f32 * (1.0 / 255.0),
                          color.a as f32 * (1.0 / 255.0));
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
            gl::Enable(gl::BLEND);
            gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, ptr::null());
            gl::Disable(gl::BLEND);
        }
    }

    fn draw_text(&self, string: &str, origin: &Point2D<i16>) {
        let mut next = *origin;
        let char_count = string.chars().count();
        let mut vertex_data = Vec::with_capacity(char_count * 4);
        let mut index_data = Vec::with_capacity(char_count * 6);
        for mut character in string.chars() {
            if !self.font.characters.contains_key(&character) {
                character = '?';
            }
            let info = &self.font.characters[&character];
            let position_rect =
                Rect::new(Point2D::new(next.x - info.origin_x, next.y - info.origin_y),
                          Size2D::new(info.width as i16, info.height as i16));
            let tex_coord_rect = Rect::new(Point2D::new(info.x, info.y),
                                           Size2D::new(info.width, info.height));
            let first_vertex_index = vertex_data.len();
            vertex_data.extend_from_slice(&[
                DebugFontVertex::new(position_rect.origin,         tex_coord_rect.origin),
                DebugFontVertex::new(position_rect.top_right(),    tex_coord_rect.top_right()),
                DebugFontVertex::new(position_rect.bottom_right(), tex_coord_rect.bottom_right()),
                DebugFontVertex::new(position_rect.bottom_left(),  tex_coord_rect.bottom_left()),
            ]);
            index_data.extend(QUAD_INDICES.iter().map(|&i| i + first_vertex_index as u32));
            next.x += info.advance;
        }

        self.font_vertex_array
            .vertex_buffer
            .upload(&vertex_data, BufferTarget::Vertex, BufferUploadMode::Dynamic);
        self.font_vertex_array
            .index_buffer
            .upload(&index_data, BufferTarget::Index, BufferUploadMode::Dynamic);

        unsafe {
            gl::BindVertexArray(self.font_vertex_array.gl_vertex_array);
            gl::UseProgram(self.font_program.program.gl_program);
            gl::Uniform2f(self.font_program.framebuffer_size_uniform.location,
                          self.framebuffer_size.width as GLfloat,
                          self.framebuffer_size.height as GLfloat);
            gl::Uniform2f(self.font_program.font_texture_size_uniform.location,
                          self.font_texture.size.width as GLfloat,
                          self.font_texture.size.height as GLfloat);
            self.font_texture.bind(0);
            gl::Uniform1i(self.font_program.font_texture_uniform.location, 0);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
            gl::Enable(gl::BLEND);
            gl::DrawElements(gl::TRIANGLES,
                             index_data.len() as GLsizei,
                             gl::UNSIGNED_INT,
                             ptr::null());
            gl::Disable(gl::BLEND);
        }
    }
}

struct DebugFontVertexArray {
    gl_vertex_array: GLuint,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
}

impl DebugFontVertexArray {
    fn new(debug_font_program: &DebugFontProgram) -> DebugFontVertexArray {
        let vertex_buffer = Buffer::new();
        let index_buffer = Buffer::new();
        let mut gl_vertex_array = 0;
        unsafe {
            let position_attr = VertexAttr::new(&debug_font_program.program, "Position");
            let tex_coord_attr = VertexAttr::new(&debug_font_program.program, "TexCoord");

            gl::GenVertexArrays(1, &mut gl_vertex_array);
            gl::BindVertexArray(gl_vertex_array);
            gl::UseProgram(debug_font_program.program.gl_program);
            gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer.gl_buffer);
            position_attr.configure_float(2,
                                          gl::UNSIGNED_SHORT,
                                          false,
                                          DEBUG_FONT_VERTEX_SIZE,
                                          0,
                                          0);
            tex_coord_attr.configure_float(2,
                                           gl::UNSIGNED_SHORT,
                                           false,
                                           DEBUG_FONT_VERTEX_SIZE,
                                           4,
                                           0);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, index_buffer.gl_buffer);
        }

        DebugFontVertexArray { gl_vertex_array, vertex_buffer, index_buffer }
    }
}

impl Drop for DebugFontVertexArray {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &mut self.gl_vertex_array);
        }
    }
}

struct DebugSolidVertexArray {
    gl_vertex_array: GLuint,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
}

impl DebugSolidVertexArray {
    fn new(debug_solid_program: &DebugSolidProgram) -> DebugSolidVertexArray {
        let vertex_buffer = Buffer::new();
        let index_buffer = Buffer::new();
        let mut gl_vertex_array = 0;
        unsafe {
            let position_attr = VertexAttr::new(&debug_solid_program.program, "Position");

            gl::GenVertexArrays(1, &mut gl_vertex_array);
            gl::BindVertexArray(gl_vertex_array);
            gl::UseProgram(debug_solid_program.program.gl_program);
            gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer.gl_buffer);
            position_attr.configure_float(2,
                                          gl::UNSIGNED_SHORT,
                                          false,
                                          DEBUG_SOLID_VERTEX_SIZE,
                                          0,
                                          0);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, index_buffer.gl_buffer);
        }

        DebugSolidVertexArray { gl_vertex_array, vertex_buffer, index_buffer }
    }
}

impl Drop for DebugSolidVertexArray {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &mut self.gl_vertex_array);
        }
    }
}

struct DebugFontProgram {
    program: Program,
    framebuffer_size_uniform: Uniform,
    font_texture_size_uniform: Uniform,
    font_texture_uniform: Uniform,
}

impl DebugFontProgram {
    fn new() -> DebugFontProgram {
        let program = Program::new("debug_font");
        let framebuffer_size_uniform = Uniform::new(&program, "FramebufferSize");
        let font_texture_size_uniform = Uniform::new(&program, "FontTextureSize");
        let font_texture_uniform = Uniform::new(&program, "FontTexture");
        DebugFontProgram {
            program,
            framebuffer_size_uniform,
            font_texture_size_uniform,
            font_texture_uniform,
        }
    }
}

struct DebugSolidProgram {
    program: Program,
    framebuffer_size_uniform: Uniform,
    color_uniform: Uniform,
}

impl DebugSolidProgram {
    fn new() -> DebugSolidProgram {
        let program = Program::new("debug_solid");
        let framebuffer_size_uniform = Uniform::new(&program, "FramebufferSize");
        let color_uniform = Uniform::new(&program, "Color");
        DebugSolidProgram { program, framebuffer_size_uniform, color_uniform }
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct DebugFontVertex {
    position_x: i16,
    position_y: i16,
    tex_coord_x: u16,
    tex_coord_y: u16,
}

impl DebugFontVertex {
    fn new(position: Point2D<i16>, tex_coord: Point2D<u16>) -> DebugFontVertex {
        DebugFontVertex {
            position_x: position.x,
            position_y: position.y,
            tex_coord_x: tex_coord.x,
            tex_coord_y: tex_coord.y,
        }
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct DebugSolidVertex {
    position_x: i16,
    position_y: i16,
}

impl DebugSolidVertex {
    fn new(position: Point2D<i16>) -> DebugSolidVertex {
        DebugSolidVertex { position_x: position.x, position_y: position.y }
    }
}

fn duration_ms(time: Duration) -> f64 {
    time.as_secs() as f64 * 1000.0 + time.subsec_nanos() as f64 / 1000000.0
}
