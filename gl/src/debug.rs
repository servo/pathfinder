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

use crate::device::{Buffer, BufferTarget, BufferUploadMode, Device, Program, Texture};
use crate::device::{Uniform, VertexAttr};
use gl::types::{GLfloat, GLint, GLsizei, GLuint};
use gl;
use pathfinder_geometry::basic::point::Point2DI32;
use pathfinder_geometry::basic::rect::RectI32;
use pathfinder_renderer::gpu_data::Stats;
use pathfinder_renderer::paint::ColorU;
use serde_json;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::BufReader;
use std::ops::{Add, Div};
use std::ptr;
use std::time::Duration;

const SAMPLE_BUFFER_SIZE: usize = 60;

const DEBUG_TEXTURE_VERTEX_SIZE: GLint = 8;
const DEBUG_SOLID_VERTEX_SIZE:   GLint = 4;

pub const PADDING: i32 = 12;

pub static TEXT_COLOR:   ColorU = ColorU { r: 255, g: 255, b: 255, a: 255      };
pub static WINDOW_COLOR: ColorU = ColorU { r: 0,   g: 0,   b: 0,   a: 255 - 90 };

static INVERTED_TEXT_COLOR: ColorU = ColorU { r: 0,   g: 0,   b: 0,   a: 255      };

const PERF_WINDOW_WIDTH: i32 = 375;
const PERF_WINDOW_HEIGHT: i32 = LINE_HEIGHT * 6 + PADDING + 2;
const FONT_ASCENT: i32 = 28;
const LINE_HEIGHT: i32 = 42;

static FONT_JSON_FILENAME: &'static str = "debug-font.json";
static FONT_PNG_NAME: &'static str = "debug-font";

static CORNER_FILL_PNG_NAME: &'static str = "debug-corner-fill";
static CORNER_OUTLINE_PNG_NAME: &'static str = "debug-corner-outline";

static QUAD_INDICES:              [u32; 6] = [0, 1, 3, 1, 2, 3];
static RECT_LINE_INDICES:         [u32; 8] = [0, 1, 1, 2, 2, 3, 3, 0];
static OUTLINE_RECT_LINE_INDICES: [u32; 8] = [0, 1, 2, 3, 4, 5, 6, 7];

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
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    #[serde(rename = "originX")]
    origin_x: i32,
    #[serde(rename = "originY")]
    origin_y: i32,
    advance: i32,
}

impl DebugFont {
    fn load(device: &Device) -> DebugFont {
        let mut path = device.resources_directory.clone();
        path.push(FONT_JSON_FILENAME);

        serde_json::from_reader(BufReader::new(File::open(path).unwrap())).unwrap()
    }
}

pub struct DebugUI {
    framebuffer_size: Point2DI32,

    texture_program: DebugTextureProgram,
    texture_vertex_array: DebugTextureVertexArray,
    font: DebugFont,
    solid_program: DebugSolidProgram,
    solid_vertex_array: DebugSolidVertexArray,

    font_texture: Texture,
    corner_fill_texture: Texture,
    corner_outline_texture: Texture,

    cpu_samples: SampleBuffer<CPUSample>,
    gpu_samples: SampleBuffer<GPUSample>,
}

impl DebugUI {
    pub fn new(device: &Device, framebuffer_size: Point2DI32) -> DebugUI {
        let texture_program = DebugTextureProgram::new(device);
        let texture_vertex_array = DebugTextureVertexArray::new(&texture_program);
        let font = DebugFont::load(device);

        let solid_program = DebugSolidProgram::new(device);
        let solid_vertex_array = DebugSolidVertexArray::new(&solid_program);

        let font_texture = device.create_texture_from_png(FONT_PNG_NAME);
        let corner_fill_texture = device.create_texture_from_png(CORNER_FILL_PNG_NAME);
        let corner_outline_texture = device.create_texture_from_png(CORNER_OUTLINE_PNG_NAME);

        DebugUI {
            framebuffer_size,

            texture_program,
            texture_vertex_array,
            font,
            solid_program,
            solid_vertex_array,

            font_texture,
            corner_fill_texture,
            corner_outline_texture,

            cpu_samples: SampleBuffer::new(),
            gpu_samples: SampleBuffer::new(),
        }
    }

    pub fn framebuffer_size(&self) -> Point2DI32 {
        self.framebuffer_size
    }

    pub fn set_framebuffer_size(&mut self, window_size: Point2DI32) {
        self.framebuffer_size = window_size;
    }

    pub fn add_sample(&mut self,
                      stats: Stats,
                      tile_time: Duration,
                      rendering_time: Option<Duration>) {
        self.cpu_samples.push(CPUSample { stats, elapsed: tile_time });
        if let Some(rendering_time) = rendering_time {
            self.gpu_samples.push(GPUSample { elapsed: rendering_time });
        }
    }

    pub fn draw(&self) {
        // Draw performance window.
        let bottom = self.framebuffer_size.y() - PADDING;
        let window_rect = RectI32::new(
            Point2DI32::new(self.framebuffer_size.x() - PADDING - PERF_WINDOW_WIDTH,
                            bottom - PERF_WINDOW_HEIGHT),
            Point2DI32::new(PERF_WINDOW_WIDTH, PERF_WINDOW_HEIGHT));
        self.draw_solid_rounded_rect(window_rect, WINDOW_COLOR);
        let origin = window_rect.origin() + Point2DI32::new(PADDING, PADDING + FONT_ASCENT);

        let mean_cpu_sample = self.cpu_samples.mean();
        self.draw_text(&format!("Objects: {}", mean_cpu_sample.stats.object_count), origin, false);
        self.draw_text(&format!("Solid Tiles: {}", mean_cpu_sample.stats.solid_tile_count),
                       origin + Point2DI32::new(0, LINE_HEIGHT * 1),
                       false);
        self.draw_text(&format!("Mask Tiles: {}", mean_cpu_sample.stats.mask_tile_count),
                       origin + Point2DI32::new(0, LINE_HEIGHT * 2),
                       false);
        self.draw_text(&format!("Fills: {}", mean_cpu_sample.stats.fill_count),
                       origin + Point2DI32::new(0, LINE_HEIGHT * 3),
                       false);

        self.draw_text(&format!("CPU Time: {:.3} ms", duration_to_ms(mean_cpu_sample.elapsed)),
                       origin + Point2DI32::new(0, LINE_HEIGHT * 4),
                       false);

        let mean_gpu_sample = self.gpu_samples.mean();
        self.draw_text(&format!("GPU Time: {:.3} ms", duration_to_ms(mean_gpu_sample.elapsed)),
                       origin + Point2DI32::new(0, LINE_HEIGHT * 5),
                       false);
    }

    pub fn draw_solid_rect(&self, rect: RectI32, color: ColorU) {
        self.draw_rect(rect, color, true);
    }

    pub fn draw_rect_outline(&self, rect: RectI32, color: ColorU) {
        self.draw_rect(rect, color, false);
    }

    fn draw_rect(&self, rect: RectI32, color: ColorU, filled: bool) {
        let vertex_data = [
            DebugSolidVertex::new(rect.origin()),
            DebugSolidVertex::new(rect.upper_right()),
            DebugSolidVertex::new(rect.lower_right()),
            DebugSolidVertex::new(rect.lower_left()),
        ];

        if filled {
            self.draw_solid_rects_with_vertex_data(&vertex_data, &QUAD_INDICES, color, true);
        } else {
            self.draw_solid_rects_with_vertex_data(&vertex_data, &RECT_LINE_INDICES, color, false);
        }
    }

    fn draw_solid_rects_with_vertex_data(&self,
                                         vertex_data: &[DebugSolidVertex],
                                         index_data: &[u32],
                                         color: ColorU,
                                         filled: bool) {
        unsafe {
            gl::BindVertexArray(self.solid_vertex_array.gl_vertex_array);
        }

        self.solid_vertex_array
            .vertex_buffer
            .upload(vertex_data, BufferTarget::Vertex, BufferUploadMode::Dynamic);
        self.solid_vertex_array
            .index_buffer
            .upload(index_data, BufferTarget::Index, BufferUploadMode::Dynamic);

        unsafe {
            gl::UseProgram(self.solid_program.program.gl_program);
            gl::Uniform2f(self.solid_program.framebuffer_size_uniform.location,
                          self.framebuffer_size.x() as GLfloat,
                          self.framebuffer_size.y() as GLfloat);
            set_color_uniform(&self.solid_program.color_uniform, color);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
            gl::Enable(gl::BLEND);
            let primitive = if filled { gl::TRIANGLES } else { gl::LINES };
            gl::DrawElements(primitive, index_data.len() as GLint, gl::UNSIGNED_INT, ptr::null());
            gl::Disable(gl::BLEND);
        }
    }

    pub fn draw_text(&self, string: &str, origin: Point2DI32, invert: bool) {
        let mut next = origin;
        let char_count = string.chars().count();
        let mut vertex_data = Vec::with_capacity(char_count * 4);
        let mut index_data = Vec::with_capacity(char_count * 6);
        for mut character in string.chars() {
            if !self.font.characters.contains_key(&character) {
                character = '?';
            }

            let info = &self.font.characters[&character];
            let position_rect =
                RectI32::new(Point2DI32::new(next.x() - info.origin_x, next.y() - info.origin_y),
                             Point2DI32::new(info.width as i32, info.height as i32));
            let tex_coord_rect = RectI32::new(Point2DI32::new(info.x, info.y),
                                              Point2DI32::new(info.width, info.height));
            let first_vertex_index = vertex_data.len();
            vertex_data.extend_from_slice(&[
                DebugTextureVertex::new(position_rect.origin(),      tex_coord_rect.origin()),
                DebugTextureVertex::new(position_rect.upper_right(), tex_coord_rect.upper_right()),
                DebugTextureVertex::new(position_rect.lower_right(), tex_coord_rect.lower_right()),
                DebugTextureVertex::new(position_rect.lower_left(),  tex_coord_rect.lower_left()),
            ]);
            index_data.extend(QUAD_INDICES.iter().map(|&i| i + first_vertex_index as u32));

            let next_x = next.x() + info.advance;
            next.set_x(next_x);
        }

        let color = if invert { INVERTED_TEXT_COLOR } else { TEXT_COLOR };
        self.draw_texture_with_vertex_data(&vertex_data, &index_data, &self.font_texture, color);
    }

    pub fn draw_texture(&self, origin: Point2DI32, texture: &Texture, color: ColorU) {
        let position_rect = RectI32::new(origin, texture.size);
        let tex_coord_rect = RectI32::new(Point2DI32::default(), texture.size);
        let vertex_data = [
            DebugTextureVertex::new(position_rect.origin(),      tex_coord_rect.origin()),
            DebugTextureVertex::new(position_rect.upper_right(), tex_coord_rect.upper_right()),
            DebugTextureVertex::new(position_rect.lower_right(), tex_coord_rect.lower_right()),
            DebugTextureVertex::new(position_rect.lower_left(),  tex_coord_rect.lower_left()),
        ];

        self.draw_texture_with_vertex_data(&vertex_data, &QUAD_INDICES, texture, color);
    }

    pub fn measure_text(&self, string: &str) -> i32 {
        let mut next = 0;
        for mut character in string.chars() {
            if !self.font.characters.contains_key(&character) {
                character = '?';
            }

            let info = &self.font.characters[&character];
            next += info.advance;
        }
        next
    }

    pub fn draw_solid_rounded_rect(&self, rect: RectI32, color: ColorU) {
        let corner_texture = self.corner_texture(true);
        let corner_rects = CornerRects::new(rect, corner_texture);
        self.draw_rounded_rect_corners(color, corner_texture, &corner_rects);

        let solid_rect_mid   = RectI32::from_points(corner_rects.upper_left.upper_right(),
                                                    corner_rects.lower_right.lower_left());
        let solid_rect_left  = RectI32::from_points(corner_rects.upper_left.lower_left(),
                                                    corner_rects.lower_left.upper_right());
        let solid_rect_right = RectI32::from_points(corner_rects.upper_right.lower_left(),
                                                    corner_rects.lower_right.upper_right());
        let vertex_data = vec![
            DebugSolidVertex::new(solid_rect_mid.origin()),
            DebugSolidVertex::new(solid_rect_mid.upper_right()),
            DebugSolidVertex::new(solid_rect_mid.lower_right()),
            DebugSolidVertex::new(solid_rect_mid.lower_left()),

            DebugSolidVertex::new(solid_rect_left.origin()),
            DebugSolidVertex::new(solid_rect_left.upper_right()),
            DebugSolidVertex::new(solid_rect_left.lower_right()),
            DebugSolidVertex::new(solid_rect_left.lower_left()),

            DebugSolidVertex::new(solid_rect_right.origin()),
            DebugSolidVertex::new(solid_rect_right.upper_right()),
            DebugSolidVertex::new(solid_rect_right.lower_right()),
            DebugSolidVertex::new(solid_rect_right.lower_left()),
        ];

        let mut index_data = Vec::with_capacity(18);
        index_data.extend(QUAD_INDICES.iter().map(|&index| index + 0));
        index_data.extend(QUAD_INDICES.iter().map(|&index| index + 4));
        index_data.extend(QUAD_INDICES.iter().map(|&index| index + 8));

        self.draw_solid_rects_with_vertex_data(&vertex_data, &index_data[0..18], color, true);
    }

    pub fn draw_rounded_rect_outline(&self, rect: RectI32, color: ColorU) {
        let corner_texture = self.corner_texture(false);
        let corner_rects = CornerRects::new(rect, corner_texture);
        self.draw_rounded_rect_corners(color, corner_texture, &corner_rects);

        let vertex_data = vec![
            DebugSolidVertex::new(corner_rects.upper_left.upper_right()),
            DebugSolidVertex::new(corner_rects.upper_right.origin()),
            DebugSolidVertex::new(corner_rects.upper_right.lower_right()),
            DebugSolidVertex::new(corner_rects.lower_right.upper_right()),
            DebugSolidVertex::new(corner_rects.lower_left.lower_right()),
            DebugSolidVertex::new(corner_rects.lower_right.lower_left()),
            DebugSolidVertex::new(corner_rects.upper_left.lower_left()),
            DebugSolidVertex::new(corner_rects.lower_left.origin()),
        ];

        let index_data = &OUTLINE_RECT_LINE_INDICES;
        self.draw_solid_rects_with_vertex_data(&vertex_data, index_data, color, false);
    }

    fn draw_rounded_rect_corners(&self,
                                 color: ColorU,
                                 texture: &Texture,
                                 corner_rects: &CornerRects) {
        let corner_size = texture.size;
        let tex_coord_rect = RectI32::new(Point2DI32::default(), corner_size);

        let vertex_data = vec![
            DebugTextureVertex::new(
                corner_rects.upper_left.origin(),       tex_coord_rect.origin()),
            DebugTextureVertex::new(
                corner_rects.upper_left.upper_right(),  tex_coord_rect.upper_right()),
            DebugTextureVertex::new(
                corner_rects.upper_left.lower_right(),  tex_coord_rect.lower_right()),
            DebugTextureVertex::new(
                corner_rects.upper_left.lower_left(),   tex_coord_rect.lower_left()),

            DebugTextureVertex::new(
                corner_rects.upper_right.origin(),      tex_coord_rect.lower_left()),
            DebugTextureVertex::new(
                corner_rects.upper_right.upper_right(), tex_coord_rect.origin()),
            DebugTextureVertex::new(
                corner_rects.upper_right.lower_right(), tex_coord_rect.upper_right()),
            DebugTextureVertex::new(
                corner_rects.upper_right.lower_left(),  tex_coord_rect.lower_right()),

            DebugTextureVertex::new(
                corner_rects.lower_left.origin(),       tex_coord_rect.upper_right()),
            DebugTextureVertex::new(
                corner_rects.lower_left.upper_right(),  tex_coord_rect.lower_right()),
            DebugTextureVertex::new(
                corner_rects.lower_left.lower_right(),  tex_coord_rect.lower_left()),
            DebugTextureVertex::new(
                corner_rects.lower_left.lower_left(),   tex_coord_rect.origin()),

            DebugTextureVertex::new(
                corner_rects.lower_right.origin(),      tex_coord_rect.lower_right()),
            DebugTextureVertex::new(
                corner_rects.lower_right.upper_right(), tex_coord_rect.lower_left()),
            DebugTextureVertex::new(
                corner_rects.lower_right.lower_right(), tex_coord_rect.origin()),
            DebugTextureVertex::new(
                corner_rects.lower_right.lower_left(),  tex_coord_rect.upper_right()),
        ];

        let mut index_data = Vec::with_capacity(24);
        index_data.extend(QUAD_INDICES.iter().map(|&index| index + 0));
        index_data.extend(QUAD_INDICES.iter().map(|&index| index + 4));
        index_data.extend(QUAD_INDICES.iter().map(|&index| index + 8));
        index_data.extend(QUAD_INDICES.iter().map(|&index| index + 12));

        self.draw_texture_with_vertex_data(&vertex_data, &index_data, texture, color);
    }

    fn corner_texture(&self, filled: bool) -> &Texture {
        if filled { &self.corner_fill_texture } else { &self.corner_outline_texture }
    }

    fn draw_texture_with_vertex_data(&self,
                                     vertex_data: &[DebugTextureVertex],
                                     index_data: &[u32],
                                     texture: &Texture,
                                     color: ColorU) {
        self.texture_vertex_array
            .vertex_buffer
            .upload(&vertex_data, BufferTarget::Vertex, BufferUploadMode::Dynamic);
        self.texture_vertex_array
            .index_buffer
            .upload(&index_data, BufferTarget::Index, BufferUploadMode::Dynamic);

        unsafe {
            gl::BindVertexArray(self.texture_vertex_array.gl_vertex_array);
            gl::UseProgram(self.texture_program.program.gl_program);
            gl::Uniform2f(self.texture_program.framebuffer_size_uniform.location,
                          self.framebuffer_size.x() as GLfloat,
                          self.framebuffer_size.y() as GLfloat);
            gl::Uniform2f(self.texture_program.texture_size_uniform.location,
                          texture.size.x() as GLfloat,
                          texture.size.y() as GLfloat);
            set_color_uniform(&self.texture_program.color_uniform, color);
            texture.bind(0);
            gl::Uniform1i(self.texture_program.texture_uniform.location, 0);
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

struct DebugTextureVertexArray {
    gl_vertex_array: GLuint,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
}

impl DebugTextureVertexArray {
    fn new(debug_texture_program: &DebugTextureProgram) -> DebugTextureVertexArray {
        let vertex_buffer = Buffer::new();
        let index_buffer = Buffer::new();
        let mut gl_vertex_array = 0;
        unsafe {
            let position_attr = VertexAttr::new(&debug_texture_program.program, "Position");
            let tex_coord_attr = VertexAttr::new(&debug_texture_program.program, "TexCoord");

            gl::GenVertexArrays(1, &mut gl_vertex_array);
            gl::BindVertexArray(gl_vertex_array);
            gl::UseProgram(debug_texture_program.program.gl_program);
            gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer.gl_buffer);
            position_attr.configure_float(2,
                                          gl::UNSIGNED_SHORT,
                                          false,
                                          DEBUG_TEXTURE_VERTEX_SIZE,
                                          0,
                                          0);
            tex_coord_attr.configure_float(2,
                                           gl::UNSIGNED_SHORT,
                                           false,
                                           DEBUG_TEXTURE_VERTEX_SIZE,
                                           4,
                                           0);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, index_buffer.gl_buffer);
        }

        DebugTextureVertexArray { gl_vertex_array, vertex_buffer, index_buffer }
    }
}

impl Drop for DebugTextureVertexArray {
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

struct DebugTextureProgram {
    program: Program,
    framebuffer_size_uniform: Uniform,
    texture_size_uniform: Uniform,
    texture_uniform: Uniform,
    color_uniform: Uniform,
}

impl DebugTextureProgram {
    fn new(device: &Device) -> DebugTextureProgram {
        let program = device.create_program("debug_texture");
        let framebuffer_size_uniform = Uniform::new(&program, "FramebufferSize");
        let texture_size_uniform = Uniform::new(&program, "TextureSize");
        let texture_uniform = Uniform::new(&program, "Texture");
        let color_uniform = Uniform::new(&program, "Color");
        DebugTextureProgram {
            program,
            framebuffer_size_uniform,
            texture_size_uniform,
            texture_uniform,
            color_uniform,
        }
    }
}

struct DebugSolidProgram {
    program: Program,
    framebuffer_size_uniform: Uniform,
    color_uniform: Uniform,
}

impl DebugSolidProgram {
    fn new(device: &Device) -> DebugSolidProgram {
        let program = device.create_program("debug_solid");
        let framebuffer_size_uniform = Uniform::new(&program, "FramebufferSize");
        let color_uniform = Uniform::new(&program, "Color");
        DebugSolidProgram { program, framebuffer_size_uniform, color_uniform }
    }
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
#[repr(C)]
struct DebugTextureVertex {
    position_x: i16,
    position_y: i16,
    tex_coord_x: u16,
    tex_coord_y: u16,
}

impl DebugTextureVertex {
    fn new(position: Point2DI32, tex_coord: Point2DI32) -> DebugTextureVertex {
        DebugTextureVertex {
            position_x: position.x() as i16,
            position_y: position.y() as i16,
            tex_coord_x: tex_coord.x() as u16,
            tex_coord_y: tex_coord.y() as u16,
        }
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
#[repr(C)]
struct DebugSolidVertex {
    position_x: i16,
    position_y: i16,
}

impl DebugSolidVertex {
    fn new(position: Point2DI32) -> DebugSolidVertex {
        DebugSolidVertex { position_x: position.x() as i16, position_y: position.y() as i16 }
    }
}

struct SampleBuffer<S> where S: Add<S, Output=S> + Div<u32, Output=S> + Clone + Default {
    samples: VecDeque<S>,
}

impl<S> SampleBuffer<S> where S: Add<S, Output=S> + Div<u32, Output=S> + Clone + Default {
    fn new() -> SampleBuffer<S> {
        SampleBuffer { samples: VecDeque::with_capacity(SAMPLE_BUFFER_SIZE) }
    }

    fn push(&mut self, time: S) {
        self.samples.push_back(time);
        while self.samples.len() > SAMPLE_BUFFER_SIZE {
            self.samples.pop_front();
        }
    }

    fn mean(&self) -> S {
        let mut mean = Default::default();
        if self.samples.is_empty() {
            return mean;
        }

        for time in &self.samples {
            mean = mean + (*time).clone();
        }

        mean / self.samples.len() as u32
    }
}

fn set_color_uniform(uniform: &Uniform, color: ColorU) {
    unsafe {
        gl::Uniform4f(uniform.location,
                      color.r as f32 * (1.0 / 255.0),
                      color.g as f32 * (1.0 / 255.0),
                      color.b as f32 * (1.0 / 255.0),
                      color.a as f32 * (1.0 / 255.0));
    }
}

#[derive(Clone, Default)]
struct CPUSample {
    elapsed: Duration,
    stats: Stats,
}

impl Add<CPUSample> for CPUSample {
    type Output = CPUSample;
    fn add(self, other: CPUSample) -> CPUSample {
        CPUSample {
            elapsed: self.elapsed + other.elapsed,
            stats: Stats {
                object_count: self.stats.object_count + other.stats.object_count,
                solid_tile_count: self.stats.solid_tile_count + other.stats.solid_tile_count,
                mask_tile_count: self.stats.mask_tile_count + other.stats.mask_tile_count,
                fill_count: self.stats.fill_count + other.stats.fill_count,
            },
        }
    }
}

impl Div<u32> for CPUSample {
    type Output = CPUSample;
    fn div(self, divisor: u32) -> CPUSample {
        CPUSample {
            elapsed: self.elapsed / divisor,
            stats: Stats {
                object_count: self.stats.object_count / divisor,
                solid_tile_count: self.stats.solid_tile_count / divisor,
                mask_tile_count: self.stats.mask_tile_count / divisor,
                fill_count: self.stats.fill_count / divisor,
            },
        }
    }
}

#[derive(Clone, Default)]
struct GPUSample {
    elapsed: Duration,
}

impl Add<GPUSample> for GPUSample {
    type Output = GPUSample;
    fn add(self, other: GPUSample) -> GPUSample {
        GPUSample { elapsed: self.elapsed + other.elapsed }
    }
}

impl Div<u32> for GPUSample {
    type Output = GPUSample;
    fn div(self, divisor: u32) -> GPUSample {
        GPUSample { elapsed: self.elapsed / divisor }
    }
}

fn duration_to_ms(time: Duration) -> f64 {
    time.as_secs() as f64 * 1000.0 + time.subsec_nanos() as f64 / 1000000.0
}

struct CornerRects {
    upper_left: RectI32,
    upper_right: RectI32,
    lower_left: RectI32,
    lower_right: RectI32,
}

impl CornerRects {
    fn new(rect: RectI32, texture: &Texture) -> CornerRects {
        let size = texture.size;
        CornerRects {
            upper_left:  RectI32::new(rect.origin(),                                     size),
            upper_right: RectI32::new(rect.upper_right() - Point2DI32::new(size.x(), 0), size),
            lower_left:  RectI32::new(rect.lower_left()  - Point2DI32::new(0, size.y()), size),
            lower_right: RectI32::new(rect.lower_right() - size,                         size),
        }
    }
}
