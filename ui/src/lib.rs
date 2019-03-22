// pathfinder/ui/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A minimal immediate mode UI, for debugging.
//!
//! This can be used in your own applications as an ultra-minimal lightweight
//! alternative to dear imgui, Conrod, etc.

#[macro_use]
extern crate serde_derive;

use hashbrown::HashMap;
use pathfinder_geometry::basic::point::{Point2DF32, Point2DI32};
use pathfinder_geometry::basic::rect::RectI32;
use pathfinder_geometry::color::ColorU;
use pathfinder_gpu::resources::ResourceLoader;
use pathfinder_gpu::{BlendState, BufferTarget, BufferUploadMode, Device, Primitive, RenderState};
use pathfinder_gpu::{UniformData, VertexAttrType};
use pathfinder_simd::default::F32x4;
use serde_json;
use std::mem;

pub const PADDING: i32 = 12;

pub const LINE_HEIGHT: i32 = 42;
pub const FONT_ASCENT: i32 = 28;

pub const BUTTON_WIDTH: i32 = PADDING * 2 + ICON_SIZE;
pub const BUTTON_HEIGHT: i32 = PADDING * 2 + ICON_SIZE;
pub const BUTTON_TEXT_OFFSET: i32 = PADDING + 36;

pub const TOOLTIP_HEIGHT: i32 = FONT_ASCENT + PADDING * 2;

const DEBUG_TEXTURE_VERTEX_SIZE: usize = 8;
const DEBUG_SOLID_VERTEX_SIZE:   usize = 4;

const ICON_SIZE: i32 = 48;

const SWITCH_SEGMENT_SIZE: i32 = 96;

pub static TEXT_COLOR:   ColorU = ColorU { r: 255, g: 255, b: 255, a: 255      };
pub static WINDOW_COLOR: ColorU = ColorU { r: 0,   g: 0,   b: 0,   a: 255 - 90 };

static BUTTON_ICON_COLOR: ColorU = ColorU { r: 255, g: 255, b: 255, a: 255 };
static OUTLINE_COLOR:     ColorU = ColorU { r: 255, g: 255, b: 255, a: 192 };

static INVERTED_TEXT_COLOR: ColorU = ColorU { r: 0,   g: 0,   b: 0,   a: 255      };

static FONT_JSON_VIRTUAL_PATH: &'static str = "debug-fonts/regular.json";
static FONT_PNG_NAME: &'static str = "debug-font";

static CORNER_FILL_PNG_NAME: &'static str = "debug-corner-fill";
static CORNER_OUTLINE_PNG_NAME: &'static str = "debug-corner-outline";

static QUAD_INDICES:              [u32; 6] = [0, 1, 3, 1, 2, 3];
static RECT_LINE_INDICES:         [u32; 8] = [0, 1, 1, 2, 2, 3, 3, 0];
static OUTLINE_RECT_LINE_INDICES: [u32; 8] = [0, 1, 2, 3, 4, 5, 6, 7];

pub struct UI<D> where D: Device {
    pub event_queue: UIEventQueue,
    pub mouse_position: Point2DF32,

    framebuffer_size: Point2DI32,

    texture_program: DebugTextureProgram<D>,
    texture_vertex_array: DebugTextureVertexArray<D>,
    solid_program: DebugSolidProgram<D>,
    solid_vertex_array: DebugSolidVertexArray<D>,
    font: DebugFont,

    font_texture: D::Texture,
    corner_fill_texture: D::Texture,
    corner_outline_texture: D::Texture,
}

impl<D> UI<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader, framebuffer_size: Point2DI32) -> UI<D> {
        let texture_program = DebugTextureProgram::new(device, resources);
        let texture_vertex_array = DebugTextureVertexArray::new(device, &texture_program);
        let font = DebugFont::load(resources);

        let solid_program = DebugSolidProgram::new(device, resources);
        let solid_vertex_array = DebugSolidVertexArray::new(device, &solid_program);

        let font_texture = device.create_texture_from_png(resources, FONT_PNG_NAME);
        let corner_fill_texture = device.create_texture_from_png(resources, CORNER_FILL_PNG_NAME);
        let corner_outline_texture = device.create_texture_from_png(resources,
                                                                    CORNER_OUTLINE_PNG_NAME);

        UI {
            event_queue: UIEventQueue::new(),
            mouse_position: Point2DF32::default(),

            framebuffer_size,

            texture_program,
            texture_vertex_array,
            font,
            solid_program,
            solid_vertex_array,

            font_texture,
            corner_fill_texture,
            corner_outline_texture,
        }
    }

    pub fn framebuffer_size(&self) -> Point2DI32 {
        self.framebuffer_size
    }

    pub fn set_framebuffer_size(&mut self, window_size: Point2DI32) {
        self.framebuffer_size = window_size;
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

    #[inline]
    pub fn measure_switch(&self, segment_count: u8) -> i32 {
        SWITCH_SEGMENT_SIZE * segment_count as i32 + (segment_count - 1) as i32
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

    // TODO(pcwalton): `LineSegmentI32`.
    fn draw_line(&self, device: &D, from: Point2DI32, to: Point2DI32, color: ColorU) {
        let vertex_data = vec![DebugSolidVertex::new(from), DebugSolidVertex::new(to)];
        self.draw_solid_rects_with_vertex_data(device, &vertex_data, &[0, 1], color, false);

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

    pub fn draw_button(&mut self, device: &D, origin: Point2DI32, texture: &D::Texture) -> bool {
        let button_rect = RectI32::new(origin, Point2DI32::new(BUTTON_WIDTH, BUTTON_HEIGHT));
        self.draw_solid_rounded_rect(device, button_rect, WINDOW_COLOR);
        self.draw_rounded_rect_outline(device, button_rect, OUTLINE_COLOR);
        self.draw_texture(device,
                          origin + Point2DI32::new(PADDING, PADDING),
                          texture,
                          BUTTON_ICON_COLOR);
        self.event_queue.handle_mouse_down_in_rect(button_rect).is_some()
    }

    pub fn draw_text_switch(&mut self,
                            device: &D,
                            mut origin: Point2DI32,
                            segment_labels: &[&str],
                            mut value: u8)
                            -> u8 {
        value = self.draw_switch(device, origin, value, segment_labels.len() as u8);

        origin = origin + Point2DI32::new(0, BUTTON_TEXT_OFFSET);
        for (segment_index, segment_label) in segment_labels.iter().enumerate() {
            let label_width = self.measure_text(segment_label);
            let offset = SWITCH_SEGMENT_SIZE / 2 - label_width / 2;
            self.draw_text(device,
                           segment_label,
                           origin + Point2DI32::new(offset, 0),
                           segment_index as u8 == value);
            origin += Point2DI32::new(SWITCH_SEGMENT_SIZE + 1, 0);
        }

        value
    }

    pub fn draw_image_switch(&mut self,
                             device: &D,
                             origin: Point2DI32,
                             segment_textures: &[&D::Texture],
                             mut value: u8)
                             -> u8 {
        value = self.draw_switch(device, origin, value, segment_textures.len() as u8);

        for (segment_index, segment_texture) in segment_textures.iter().enumerate() {
            let offset = (SWITCH_SEGMENT_SIZE * segment_index as i32) +
                         (SWITCH_SEGMENT_SIZE / 2) -
                         (device.texture_size(segment_texture).x() / 2);
            let color = if value == segment_index as u8 { WINDOW_COLOR } else {TEXT_COLOR };
            self.draw_texture(device,
                              origin + Point2DI32::new(offset, PADDING),
                              segment_texture,
                              color);
        }

        value
    }

    fn draw_switch(&mut self, device: &D, origin: Point2DI32, mut value: u8, segment_count: u8)
                   -> u8 {
        let widget_width = self.measure_switch(segment_count);
        let widget_rect = RectI32::new(origin, Point2DI32::new(widget_width, BUTTON_HEIGHT));
        if let Some(position) = self.event_queue.handle_mouse_down_in_rect(widget_rect) {
            let segment = (position.x() / (SWITCH_SEGMENT_SIZE + 1)) as u8;
            value = segment.min(segment_count - 1);
        }

        self.draw_solid_rounded_rect(device, widget_rect, WINDOW_COLOR);
        self.draw_rounded_rect_outline(device, widget_rect, OUTLINE_COLOR);

        let highlight_size = Point2DI32::new(SWITCH_SEGMENT_SIZE, BUTTON_HEIGHT);
        let x_offset = value as i32 * SWITCH_SEGMENT_SIZE + (value as i32 - 1);
        self.draw_solid_rounded_rect(device,
                                     RectI32::new(origin + Point2DI32::new(x_offset, 0),
                                                  highlight_size),
                                     TEXT_COLOR);

        let mut segment_origin = origin + Point2DI32::new(SWITCH_SEGMENT_SIZE + 1, 0);
        for next_segment_index in 1..segment_count {
            let prev_segment_index = next_segment_index - 1;
            if value != prev_segment_index && value != next_segment_index {
                self.draw_line(device,
                               segment_origin,
                               segment_origin + Point2DI32::new(0, BUTTON_HEIGHT),
                               TEXT_COLOR);
            }
            segment_origin = segment_origin + Point2DI32::new(SWITCH_SEGMENT_SIZE + 1, 0);
        }

        value
    }

    pub fn draw_tooltip(&self, device: &D, string: &str, rect: RectI32) {
        if !rect.to_f32().contains_point(self.mouse_position) {
            return;
        }

        let text_size = self.measure_text(string);
        let window_size = Point2DI32::new(text_size + PADDING * 2, TOOLTIP_HEIGHT);
        let origin = rect.origin() - Point2DI32::new(0, window_size.y() + PADDING);

        self.draw_solid_rounded_rect(device, RectI32::new(origin, window_size), WINDOW_COLOR);
        self.draw_text(device,
                       string,
                       origin + Point2DI32::new(PADDING, PADDING + FONT_ASCENT),
                       false);
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
    fn new(device: &D, resources: &dyn ResourceLoader) -> DebugTextureProgram<D> {
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

struct DebugSolidProgram<D> where D: Device {
    program: D::Program,
    framebuffer_size_uniform: D::Uniform,
    color_uniform: D::Uniform,
}

impl<D> DebugSolidProgram<D> where D: Device {
    fn new(device: &D, resources: &dyn ResourceLoader) -> DebugSolidProgram<D> {
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

fn set_color_uniform<D>(device: &D, uniform: &D::Uniform, color: ColorU) where D: Device {
    let color = F32x4::new(color.r as f32, color.g as f32, color.b as f32, color.a as f32);
    device.set_uniform(uniform, UniformData::Vec4(color * F32x4::splat(1.0 / 255.0)));
}

#[derive(Clone, Copy)]
pub enum UIEvent {
    MouseDown(MousePosition),
    MouseDragged(MousePosition),
}

pub struct UIEventQueue {
    events: Vec<UIEvent>,
}

impl UIEventQueue {
    fn new() -> UIEventQueue {
        UIEventQueue { events: vec![] }
    }

    pub fn push(&mut self, event: UIEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<UIEvent> {
        mem::replace(&mut self.events, vec![])
    }

    fn handle_mouse_down_in_rect(&mut self, rect: RectI32) -> Option<Point2DI32> {
        let (mut remaining_events, mut result) = (vec![], None);
        for event in self.events.drain(..) {
            match event {
                UIEvent::MouseDown(position) if rect.contains_point(position.absolute) => {
                    result = Some(position.absolute - rect.origin());
                }
                event => remaining_events.push(event),
            }
        }
        self.events = remaining_events;
        result
    }

    pub fn handle_mouse_down_or_dragged_in_rect(&mut self, rect: RectI32) -> Option<Point2DI32> {
        let (mut remaining_events, mut result) = (vec![], None);
        for event in self.events.drain(..) {
            match event {
                UIEvent::MouseDown(position) | UIEvent::MouseDragged(position) if
                        rect.contains_point(position.absolute) => {
                    result = Some(position.absolute - rect.origin());
                }
                event => remaining_events.push(event),
            }
        }
        self.events = remaining_events;
        result
    }
}

#[derive(Clone, Copy)]
pub struct MousePosition {
    pub absolute: Point2DI32,
    pub relative: Point2DI32,
}

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
    #[inline]
    fn load(resources: &dyn ResourceLoader) -> DebugFont {
        serde_json::from_slice(&resources.slurp(FONT_JSON_VIRTUAL_PATH).unwrap()).unwrap()
    }
}
