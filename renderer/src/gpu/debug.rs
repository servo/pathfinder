// pathfinder/renderer/src/gpu/debug.rs
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

use crate::gpu_data::Stats;
use crate::paint::ColorU;
use pathfinder_geometry::basic::point::Point2DI32;
use pathfinder_geometry::basic::rect::RectI32;
use pathfinder_gpu::{BlendState, BufferTarget, BufferUploadMode, Device, Primitive, RenderState};
use pathfinder_gpu::{Resources, UniformData, VertexAttrType};
use pathfinder_simd::default::F32x4;
use serde_json;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::BufReader;
use std::ops::{Add, Div};
use std::time::Duration;

const SAMPLE_BUFFER_SIZE: usize = 60;

const DEBUG_TEXTURE_VERTEX_SIZE: usize = 8;
const DEBUG_SOLID_VERTEX_SIZE:   usize = 4;

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
    fn load(resources: &Resources) -> DebugFont {
        let mut path = resources.resources_directory.clone();
        path.push(FONT_JSON_FILENAME);

        serde_json::from_reader(BufReader::new(File::open(path).unwrap())).unwrap()
    }
}

pub struct DebugUI<D> where D: Device {
    framebuffer_size: Point2DI32,

    texture_program: DebugTextureProgram<D>,
    texture_vertex_array: DebugTextureVertexArray<D>,
    font: DebugFont,
    solid_program: DebugSolidProgram<D>,
    solid_vertex_array: DebugSolidVertexArray<D>,

    font_texture: D::Texture,
    corner_fill_texture: D::Texture,
    corner_outline_texture: D::Texture,

    cpu_samples: SampleBuffer<CPUSample>,
    gpu_samples: SampleBuffer<GPUSample>,
}

impl<D> DebugUI<D> where D: Device {
    pub fn new(device: &D, resources: &Resources, framebuffer_size: Point2DI32) -> DebugUI<D> {
        let texture_program = DebugTextureProgram::new(device, resources);
        let texture_vertex_array = DebugTextureVertexArray::new(device, &texture_program);
        let font = DebugFont::load(resources);

        let solid_program = DebugSolidProgram::new(device, resources);
        let solid_vertex_array = DebugSolidVertexArray::new(device, &solid_program);

        let font_texture = device.create_texture_from_png(resources, FONT_PNG_NAME);
        let corner_fill_texture = device.create_texture_from_png(resources, CORNER_FILL_PNG_NAME);
        let corner_outline_texture = device.create_texture_from_png(resources,
                                                                    CORNER_OUTLINE_PNG_NAME);

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

    pub fn draw(&self, device: &D) {
        // Draw performance window.
        let bottom = self.framebuffer_size.y() - PADDING;
        let window_rect = RectI32::new(
            Point2DI32::new(self.framebuffer_size.x() - PADDING - PERF_WINDOW_WIDTH,
                            bottom - PERF_WINDOW_HEIGHT),
            Point2DI32::new(PERF_WINDOW_WIDTH, PERF_WINDOW_HEIGHT));
        self.draw_solid_rounded_rect(device, window_rect, WINDOW_COLOR);
        let origin = window_rect.origin() + Point2DI32::new(PADDING, PADDING + FONT_ASCENT);

        let mean_cpu_sample = self.cpu_samples.mean();
        self.draw_text(device,
                       &format!("Objects: {}", mean_cpu_sample.stats.object_count),
                       origin,
                       false);
        self.draw_text(device,
                       &format!("Solid Tiles: {}", mean_cpu_sample.stats.solid_tile_count),
                       origin + Point2DI32::new(0, LINE_HEIGHT * 1),
                       false);
        self.draw_text(device,
                       &format!("Mask Tiles: {}", mean_cpu_sample.stats.mask_tile_count),
                       origin + Point2DI32::new(0, LINE_HEIGHT * 2),
                       false);
        self.draw_text(device,
                       &format!("Fills: {}", mean_cpu_sample.stats.fill_count),
                       origin + Point2DI32::new(0, LINE_HEIGHT * 3),
                       false);

        self.draw_text(device,
                       &format!("CPU Time: {:.3} ms", duration_to_ms(mean_cpu_sample.elapsed)),
                       origin + Point2DI32::new(0, LINE_HEIGHT * 4),
                       false);

        let mean_gpu_sample = self.gpu_samples.mean();
        self.draw_text(device,
                       &format!("GPU Time: {:.3} ms", duration_to_ms(mean_gpu_sample.elapsed)),
                       origin + Point2DI32::new(0, LINE_HEIGHT * 5),
                       false);
    }

    pub fn draw_solid_rect(&self, device: &D, rect: RectI32, color: ColorU) {
        self.draw_rect(device, rect, color, true);
    }

    pub fn draw_rect_outline(&self, device: &D, rect: RectI32, color: ColorU) {
        self.draw_rect(device, rect, color, false);
    }

    fn draw_rect(&self, device: &D, rect: RectI32, color: ColorU, filled: bool) {
        let vertex_data = [
            DebugSolidVertex::new(rect.origin()),
            DebugSolidVertex::new(rect.upper_right()),
            DebugSolidVertex::new(rect.lower_right()),
            DebugSolidVertex::new(rect.lower_left()),
        ];

        if filled {
            self.draw_solid_rects_with_vertex_data(device,
                                                   &vertex_data,
                                                   &QUAD_INDICES,
                                                   color,
                                                   true);
        } else {
            self.draw_solid_rects_with_vertex_data(device,
                                                   &vertex_data,
                                                   &RECT_LINE_INDICES,
                                                   color,
                                                   false);
        }
    }

    fn draw_solid_rects_with_vertex_data(&self,
                                         device: &D,
                                         vertex_data: &[DebugSolidVertex],
                                         index_data: &[u32],
                                         color: ColorU,
                                         filled: bool) {
        device.bind_vertex_array(&self.solid_vertex_array.vertex_array);

        device.upload_to_buffer(&self.solid_vertex_array.vertex_buffer,
                                vertex_data,
                                BufferTarget::Vertex,
                                BufferUploadMode::Dynamic);
        device.upload_to_buffer(&self.solid_vertex_array.index_buffer,
                                index_data,
                                BufferTarget::Index,
                                BufferUploadMode::Dynamic);

        device.use_program(&self.solid_program.program);
        device.set_uniform(&self.solid_program.framebuffer_size_uniform,
                           UniformData::Vec2(self.framebuffer_size.0.to_f32x4()));
        set_color_uniform(device, &self.solid_program.color_uniform, color);

        let primitive = if filled { Primitive::Triangles } else { Primitive::Lines };
        device.draw_elements(primitive, index_data.len() as u32, &RenderState {
            blend: BlendState::RGBOneAlphaOneMinusSrcAlpha,
            ..RenderState::default()
        });
    }

    pub fn draw_text(&self, device: &D, string: &str, origin: Point2DI32, invert: bool) {
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
        self.draw_texture_with_vertex_data(device,
                                           &vertex_data,
                                           &index_data,
                                           &self.font_texture,
                                           color);
    }

    pub fn draw_texture(&self,
                        device: &D,
                        origin: Point2DI32,
                        texture: &D::Texture,
                        color: ColorU) {
        let position_rect = RectI32::new(origin, device.texture_size(&texture));
        let tex_coord_rect = RectI32::new(Point2DI32::default(), position_rect.size());
        let vertex_data = [
            DebugTextureVertex::new(position_rect.origin(),      tex_coord_rect.origin()),
            DebugTextureVertex::new(position_rect.upper_right(), tex_coord_rect.upper_right()),
            DebugTextureVertex::new(position_rect.lower_right(), tex_coord_rect.lower_right()),
            DebugTextureVertex::new(position_rect.lower_left(),  tex_coord_rect.lower_left()),
        ];

        self.draw_texture_with_vertex_data(device, &vertex_data, &QUAD_INDICES, texture, color);
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

    pub fn draw_solid_rounded_rect(&self, device: &D, rect: RectI32, color: ColorU) {
        let corner_texture = self.corner_texture(true);
        let corner_rects = CornerRects::new(device, rect, corner_texture);
        self.draw_rounded_rect_corners(device, color, corner_texture, &corner_rects);

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

        self.draw_solid_rects_with_vertex_data(device,
                                               &vertex_data,
                                               &index_data[0..18],
                                               color,
                                               true);
    }

    pub fn draw_rounded_rect_outline(&self, device: &D, rect: RectI32, color: ColorU) {
        let corner_texture = self.corner_texture(false);
        let corner_rects = CornerRects::new(device, rect, corner_texture);
        self.draw_rounded_rect_corners(device, color, corner_texture, &corner_rects);

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
        self.draw_solid_rects_with_vertex_data(device, &vertex_data, index_data, color, false);
    }

    fn draw_rounded_rect_corners(&self,
                                 device: &D,
                                 color: ColorU,
                                 texture: &D::Texture,
                                 corner_rects: &CornerRects) {
        let corner_size = device.texture_size(&texture);
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

        self.draw_texture_with_vertex_data(device, &vertex_data, &index_data, texture, color);
    }

    fn corner_texture(&self, filled: bool) -> &D::Texture {
        if filled { &self.corner_fill_texture } else { &self.corner_outline_texture }
    }

    fn draw_texture_with_vertex_data(&self,
                                     device: &D,
                                     vertex_data: &[DebugTextureVertex],
                                     index_data: &[u32],
                                     texture: &D::Texture,
                                     color: ColorU) {
        device.upload_to_buffer(&self.texture_vertex_array.vertex_buffer,
                                vertex_data,
                                BufferTarget::Vertex,
                                BufferUploadMode::Dynamic);
        device.upload_to_buffer(&self.texture_vertex_array.index_buffer,
                                index_data,
                                BufferTarget::Index,
                                BufferUploadMode::Dynamic);

        device.bind_vertex_array(&self.texture_vertex_array.vertex_array);
        device.use_program(&self.texture_program.program);
        device.set_uniform(&self.texture_program.framebuffer_size_uniform,
                           UniformData::Vec2(self.framebuffer_size.0.to_f32x4()));
        device.set_uniform(&self.texture_program.texture_size_uniform,
                           UniformData::Vec2(device.texture_size(&texture).0.to_f32x4()));
        set_color_uniform(device, &self.texture_program.color_uniform, color);
        device.bind_texture(texture, 0);
        device.set_uniform(&self.texture_program.texture_uniform, UniformData::TextureUnit(0));

        device.draw_elements(Primitive::Triangles, index_data.len() as u32, &RenderState {
            blend: BlendState::RGBOneAlphaOneMinusSrcAlpha,
            ..RenderState::default()
        });
    }
}

struct DebugTextureVertexArray<D> where D: Device {
    vertex_array: D::VertexArray,
    vertex_buffer: D::Buffer,
    index_buffer: D::Buffer,
}

impl<D> DebugTextureVertexArray<D> where D: Device {
    fn new(device: &D, debug_texture_program: &DebugTextureProgram<D>)
           -> DebugTextureVertexArray<D> {
        let (vertex_buffer, index_buffer) = (device.create_buffer(), device.create_buffer());
        let vertex_array = device.create_vertex_array();

        let position_attr = device.get_vertex_attr(&debug_texture_program.program, "Position");
        let tex_coord_attr = device.get_vertex_attr(&debug_texture_program.program, "TexCoord");

        device.bind_vertex_array(&vertex_array);
        device.use_program(&debug_texture_program.program);
        device.bind_buffer(&vertex_buffer, BufferTarget::Vertex);
        device.bind_buffer(&index_buffer, BufferTarget::Index);
        device.configure_float_vertex_attr(&position_attr,
                                           2,
                                           VertexAttrType::U16,
                                           false,
                                           DEBUG_TEXTURE_VERTEX_SIZE,
                                           0,
                                           0);
        device.configure_float_vertex_attr(&tex_coord_attr,
                                           2,
                                           VertexAttrType::U16,
                                           false,
                                           DEBUG_TEXTURE_VERTEX_SIZE,
                                           4,
                                           0);

        DebugTextureVertexArray { vertex_array, vertex_buffer, index_buffer }
    }
}

struct DebugSolidVertexArray<D> where D: Device {
    vertex_array: D::VertexArray,
    vertex_buffer: D::Buffer,
    index_buffer: D::Buffer,
}

impl<D> DebugSolidVertexArray<D> where D: Device {
    fn new(device: &D, debug_solid_program: &DebugSolidProgram<D>) -> DebugSolidVertexArray<D> {
        let (vertex_buffer, index_buffer) = (device.create_buffer(), device.create_buffer());
        let vertex_array = device.create_vertex_array();

        let position_attr = device.get_vertex_attr(&debug_solid_program.program, "Position");
        device.bind_vertex_array(&vertex_array);
        device.use_program(&debug_solid_program.program);
        device.bind_buffer(&vertex_buffer, BufferTarget::Vertex);
        device.bind_buffer(&index_buffer, BufferTarget::Index);
        device.configure_float_vertex_attr(&position_attr,
                                           2,
                                           VertexAttrType::U16,
                                           false,
                                           DEBUG_SOLID_VERTEX_SIZE,
                                           0,
                                           0);

        DebugSolidVertexArray { vertex_array, vertex_buffer, index_buffer }
    }
}

struct DebugTextureProgram<D> where D: Device {
    program: D::Program,
    framebuffer_size_uniform: D::Uniform,
    texture_size_uniform: D::Uniform,
    texture_uniform: D::Uniform,
    color_uniform: D::Uniform,
}

impl<D> DebugTextureProgram<D> where D: Device {
    fn new(device: &D, resources: &Resources) -> DebugTextureProgram<D> {
        let program = device.create_program(resources, "debug_texture");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let texture_size_uniform = device.get_uniform(&program, "TextureSize");
        let texture_uniform = device.get_uniform(&program, "Texture");
        let color_uniform = device.get_uniform(&program, "Color");
        DebugTextureProgram {
            program,
            framebuffer_size_uniform,
            texture_size_uniform,
            texture_uniform,
            color_uniform,
        }
    }
}

struct DebugSolidProgram<D> where D: Device {
    program: D::Program,
    framebuffer_size_uniform: D::Uniform,
    color_uniform: D::Uniform,
}

impl<D> DebugSolidProgram<D> where D: Device {
    fn new(device: &D, resources: &Resources) -> DebugSolidProgram<D> {
        let program = device.create_program(resources, "debug_solid");
        let framebuffer_size_uniform = device.get_uniform(&program, "FramebufferSize");
        let color_uniform = device.get_uniform(&program, "Color");
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

fn set_color_uniform<D>(device: &D, uniform: &D::Uniform, color: ColorU) where D: Device {
    let color = F32x4::new(color.r as f32, color.g as f32, color.b as f32, color.a as f32);
    device.set_uniform(uniform, UniformData::Vec4(color * F32x4::splat(1.0 / 255.0)));
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
    fn new<D>(device: &D, rect: RectI32, texture: &D::Texture) -> CornerRects where D: Device {
        let size = device.texture_size(texture);
        CornerRects {
            upper_left:  RectI32::new(rect.origin(),                                     size),
            upper_right: RectI32::new(rect.upper_right() - Point2DI32::new(size.x(), 0), size),
            lower_left:  RectI32::new(rect.lower_left()  - Point2DI32::new(0, size.y()), size),
            lower_right: RectI32::new(rect.lower_right() - size,                         size),
        }
    }
}
