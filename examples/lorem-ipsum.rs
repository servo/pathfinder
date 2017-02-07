/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

#![feature(alloc_system)]

extern crate alloc_system;
extern crate compute_shader;
extern crate euclid;
extern crate gl;
extern crate glfw;
extern crate image;
extern crate memmap;
extern crate pathfinder;

use compute_shader::buffer;
use compute_shader::image::{ExternalImage, Format, Image};
use compute_shader::instance::Instance;
use euclid::{Point2D, Rect, Size2D};
use gl::types::{GLchar, GLint, GLsizei, GLsizeiptr, GLuint, GLvoid};
use glfw::{Action, Context, Key, OpenGlProfileHint, WindowEvent, WindowHint, WindowMode};
use memmap::{Mmap, Protection};
use pathfinder::atlas::AtlasBuilder;
use pathfinder::charmap::CodepointRanges;
use pathfinder::coverage::CoverageBuffer;
use pathfinder::glyph_range::GlyphRanges;
use pathfinder::otf::Font;
use pathfinder::outline::{OutlineBuffers, OutlineBuilder};
use pathfinder::rasterizer::{DrawAtlasProfilingEvents, Rasterizer, RasterizerOptions};
use pathfinder::shaper;
use std::char;
use std::env;
use std::fs::File;
use std::io::Read;
use std::mem;
use std::os::raw::c_void;
use std::path::Path;
use std::process;

const ATLAS_SIZE: u32 = 2048;
const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
const SCROLL_SPEED: f64 = 6.0;

const INITIAL_POINT_SIZE: f32 = 24.0;
const MIN_POINT_SIZE: f32 = 6.0;
const MAX_POINT_SIZE: f32 = 256.0;

const FPS_DISPLAY_POINT_SIZE: f32 = 24.0;
const FPS_PADDING: i32 = 6;

static FPS_BACKGROUND_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.7];
static FPS_FOREGROUND_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
static TEXT_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

static ATLAS_DUMP_FILENAME: &'static str = "lorem-ipsum-atlas.png";

fn main() {
    let mut glfw = glfw::init(glfw::LOG_ERRORS).unwrap();
    glfw.window_hint(WindowHint::ContextVersion(3, 3));
    glfw.window_hint(WindowHint::OpenGlForwardCompat(true));
    glfw.window_hint(WindowHint::OpenGlProfile(OpenGlProfileHint::Core));
    let context = glfw.create_window(WIDTH, HEIGHT, "lorem-ipsum", WindowMode::Windowed);

    let (mut window, events) = context.expect("Couldn't create a window!");
    window.make_current();
    window.set_key_polling(true);
    window.set_scroll_polling(true);
    window.set_size_polling(true);
    window.set_framebuffer_size_polling(true);

    gl::load_with(|symbol| window.get_proc_address(symbol) as *const c_void);

    let (width, height) = window.get_framebuffer_size();
    let mut device_pixel_size = Size2D::new(width as u32, height as u32);

    let mut args = env::args();
    args.next();
    let font_path = args.next().unwrap_or_else(|| usage());

    let mut text = "".to_string();
    match args.next() {
        Some(path) => drop(File::open(path).unwrap().read_to_string(&mut text).unwrap()),
        None => text.push_str(TEXT),
    }
    text = text.replace(&['\n', '\r', '\t'][..], " ");

    // Make sure the characters include `[A-Za-z0-9 ./,]`, for the FPS display.
    let mut chars: Vec<char> = text.chars().collect();
    chars.extend(" ./,:()".chars());
    chars.extend(('A' as u32..('Z' as u32 + 1)).flat_map(char::from_u32));
    chars.extend(('a' as u32..('z' as u32 + 1)).flat_map(char::from_u32));
    chars.extend(('0' as u32..('9' as u32 + 1)).flat_map(char::from_u32));
    chars.sort();
    let codepoint_ranges = CodepointRanges::from_sorted_chars(&chars);

    let file = Mmap::open_path(font_path, Protection::Read).unwrap();
    let (font, shaped_glyph_positions, glyph_ranges);
    unsafe {
        font = Font::new(file.as_slice()).unwrap();
        glyph_ranges = font.glyph_ranges_for_codepoint_ranges(&codepoint_ranges.ranges).unwrap();
        shaped_glyph_positions = shaper::shape_text(&font, &glyph_ranges, &text)
    }

    let paragraph_width = (device_pixel_size.width as f32 * font.units_per_em() as f32 /
                           INITIAL_POINT_SIZE) as u32;

    // Do some basic line breaking.
    let mut glyph_positions = vec![];
    let line_spacing = font.units_per_em() as u32;
    let (mut current_x, mut current_y) = (0, line_spacing);
    for glyph_position in &shaped_glyph_positions {
        current_x += glyph_position.advance as u32;
        if current_x > paragraph_width {
            current_x = 0;
            current_y += line_spacing;
        }
        glyph_positions.push(GlyphPos {
            x: current_x,
            y: current_y,
            glyph_id: glyph_position.glyph_id,
        });
    }

    let renderer = Renderer::new();
    let mut point_size = INITIAL_POINT_SIZE;
    let mut translation = Point2D::new(0, 0);
    let mut dirty = true;

    let mut outline_builder = OutlineBuilder::new();
    let mut glyph_count = 0;
    for glyph_id in glyph_ranges.iter() {
        outline_builder.add_glyph(&font, glyph_id).unwrap();
        glyph_count += 1
    }

    let outline_buffers = outline_builder.create_buffers().unwrap();

    let mut fps_atlas = renderer.create_fps_atlas(&font,
                                                  &outline_builder,
                                                  &outline_buffers,
                                                  glyph_count);

    while !window.should_close() {
        if dirty {
            let events = renderer.redraw(point_size,
                                         &font,
                                         &outline_builder,
                                         &outline_buffers,
                                         glyph_count,
                                         &glyph_positions,
                                         &device_pixel_size,
                                         &translation);

            let mut draw_time = 0u64;
            unsafe {
                gl::GetQueryObjectui64v(events.draw, gl::QUERY_RESULT, &mut draw_time);
            }

            let draw_time = draw_time as f64;
            let accum_time = events.accum.time_elapsed().unwrap() as f64;
            let timing = renderer.get_timing_in_ms();

            renderer.draw_fps(&font,
                              &mut fps_atlas,
                              &outline_builder,
                              &device_pixel_size,
                              &glyph_ranges,
                              draw_time,
                              accum_time,
                              timing,
                              glyph_count);

            window.swap_buffers();

            dirty = false
        }


        glfw.wait_events();
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    window.set_should_close(true)
                }
                WindowEvent::Key(Key::S, _, Action::Press, _) => {
                    renderer.take_screenshot();
                    println!("wrote screenshot to: {}", ATLAS_DUMP_FILENAME);
                }
                WindowEvent::Scroll(x, y) => {
                    if window.get_key(Key::LeftAlt) == Action::Press ||
                            window.get_key(Key::RightAlt) == Action::Press {
                        let old_point_size = point_size;
                        point_size = old_point_size + y as f32;

                        if point_size < MIN_POINT_SIZE {
                            point_size = MIN_POINT_SIZE
                        } else if point_size > MAX_POINT_SIZE {
                            point_size = MAX_POINT_SIZE
                        }

                        let mut center = Point2D::new(
                            translation.x as f32 - device_pixel_size.width as f32 * 0.5,
                            translation.y as f32 - device_pixel_size.height as f32 * 0.5);
                        center.x = center.x * point_size / old_point_size;
                        center.y = center.y * point_size / old_point_size;

                        translation.x = (center.x + device_pixel_size.width as f32 * 0.5).round()
                            as i32;
                        translation.y = (center.y + device_pixel_size.height as f32 * 0.5).round()
                            as i32;
                    } else {
                        translation.x += (x * SCROLL_SPEED).round() as i32;
                        translation.y += (y * SCROLL_SPEED).round() as i32;
                    }

                    dirty = true
                }
                WindowEvent::Size(_, _) | WindowEvent::FramebufferSize(_, _) => {
                    let (width, height) = window.get_framebuffer_size();
                    device_pixel_size = Size2D::new(width as u32, height as u32);
                    dirty = true
                }
                _ => {}
            }
        }
    }
}

struct Renderer {
    rasterizer: Rasterizer,

    composite_program: GLuint,
    composite_atlas_uniform: GLint,
    composite_transform_uniform: GLint,
    composite_translation_uniform: GLint,
    composite_color_uniform: GLint,

    main_composite_vertex_array: CompositeVertexArray,
    fps_composite_vertex_array: CompositeVertexArray,

    solid_color_program: GLuint,
    solid_color_color_uniform: GLint,

    solid_color_vertex_array: GLuint,
    solid_color_vertex_buffer: GLuint,
    solid_color_index_buffer: GLuint,

    atlas_size: Size2D<u32>,

    main_coverage_buffer: CoverageBuffer,
    fps_coverage_buffer: CoverageBuffer,
    main_compute_image: Image,
    main_gl_texture: GLuint,
    fps_compute_image: Image,
    fps_gl_texture: GLuint,

    query: GLuint,
}

impl Renderer {
    fn new() -> Renderer {
        let instance = Instance::new().unwrap();
        let device = instance.open_device().unwrap();
        let queue = device.create_queue().unwrap();

        let rasterizer_options = RasterizerOptions::from_env().unwrap();
        let rasterizer = Rasterizer::new(&instance, device, queue, rasterizer_options).unwrap();

        let (composite_program, composite_position_attribute, composite_tex_coord_attribute);
        let (composite_atlas_uniform, composite_transform_uniform);
        let (composite_translation_uniform, composite_color_uniform);
        let (main_composite_vertex_array, fps_composite_vertex_array);
        let (solid_color_program, solid_color_position_attribute, solid_color_color_uniform);
        let (mut solid_color_vertex_buffer, mut solid_color_index_buffer) = (0, 0);
        let mut solid_color_vertex_array = 0;
        unsafe {
            composite_program = create_program(COMPOSITE_VERTEX_SHADER, COMPOSITE_FRAGMENT_SHADER);
            composite_position_attribute =
                gl::GetAttribLocation(composite_program, "aPosition\0".as_ptr() as *const GLchar);
            composite_tex_coord_attribute =
                gl::GetAttribLocation(composite_program, "aTexCoord\0".as_ptr() as *const GLchar);
            composite_atlas_uniform =
                gl::GetUniformLocation(composite_program, "uAtlas\0".as_ptr() as *const GLchar);
            composite_transform_uniform =
                gl::GetUniformLocation(composite_program,
                                       "uTransform\0".as_ptr() as *const GLchar);
            composite_translation_uniform =
                gl::GetUniformLocation(composite_program,
                                       "uTranslation\0".as_ptr() as *const GLchar);
            composite_color_uniform =
                gl::GetUniformLocation(composite_program, "uColor\0".as_ptr() as *const GLchar);

            solid_color_program = create_program(SOLID_COLOR_VERTEX_SHADER,
                                                 SOLID_COLOR_FRAGMENT_SHADER);
            solid_color_position_attribute =
                gl::GetAttribLocation(solid_color_program,
                                      "aPosition\0".as_ptr() as *const GLchar);
            solid_color_color_uniform =
                gl::GetUniformLocation(solid_color_program, "uColor\0".as_ptr() as *const GLchar);

            gl::UseProgram(composite_program);

            main_composite_vertex_array = CompositeVertexArray::new();
            fps_composite_vertex_array = CompositeVertexArray::new();
            for vertex_array in &[&main_composite_vertex_array, &fps_composite_vertex_array] {
                gl::BindVertexArray(vertex_array.vertex_array);

                gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, vertex_array.index_buffer);
                gl::BindBuffer(gl::ARRAY_BUFFER, vertex_array.vertex_buffer);

                gl::VertexAttribPointer(composite_position_attribute as GLuint,
                                        2,
                                        gl::INT,
                                        gl::FALSE,
                                        mem::size_of::<Vertex>() as GLsizei,
                                        0 as *const GLvoid);
                gl::VertexAttribPointer(composite_tex_coord_attribute as GLuint,
                                        2,
                                        gl::UNSIGNED_INT,
                                        gl::FALSE,
                                        mem::size_of::<Vertex>() as GLsizei,
                                        (mem::size_of::<f32>() * 2) as *const GLvoid);
                gl::EnableVertexAttribArray(composite_position_attribute as GLuint);
                gl::EnableVertexAttribArray(composite_tex_coord_attribute as GLuint);
            }

            gl::UseProgram(solid_color_program);

            gl::GenVertexArrays(1, &mut solid_color_vertex_array);
            gl::BindVertexArray(solid_color_vertex_array);

            gl::GenBuffers(1, &mut solid_color_vertex_buffer);
            gl::GenBuffers(1, &mut solid_color_index_buffer);

            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, solid_color_index_buffer);
            gl::BindBuffer(gl::ARRAY_BUFFER, solid_color_vertex_buffer);

            gl::VertexAttribPointer(solid_color_position_attribute as GLuint,
                                    2,
                                    gl::FLOAT,
                                    gl::FALSE,
                                    mem::size_of::<i32>() as GLsizei * 2,
                                    0 as *const GLvoid);
            gl::EnableVertexAttribArray(solid_color_position_attribute as GLuint);

            gl::BufferData(gl::ELEMENT_ARRAY_BUFFER,
                           (RECT_INDICES.len() * mem::size_of::<u32>()) as GLsizeiptr,
                           RECT_INDICES.as_ptr() as *const GLvoid,
                           gl::STATIC_DRAW);
        }

        // FIXME(pcwalton)
        let atlas_size = Size2D::new(ATLAS_SIZE, ATLAS_SIZE);

        let main_coverage_buffer = CoverageBuffer::new(&rasterizer.device, &atlas_size).unwrap();
        let fps_coverage_buffer = CoverageBuffer::new(&rasterizer.device, &atlas_size).unwrap();

        let (main_compute_image, main_gl_texture) = create_image(&rasterizer, &atlas_size);
        let (fps_compute_image, fps_gl_texture) = create_image(&rasterizer, &atlas_size);

        let mut query = 0;
        unsafe {
            gl::GenQueries(1, &mut query);
        }

        Renderer {
            rasterizer: rasterizer,

            composite_program: composite_program,
            composite_atlas_uniform: composite_atlas_uniform,
            composite_transform_uniform: composite_transform_uniform,
            composite_translation_uniform: composite_translation_uniform,
            composite_color_uniform: composite_color_uniform,

            main_composite_vertex_array: main_composite_vertex_array,
            fps_composite_vertex_array: fps_composite_vertex_array,

            solid_color_program: solid_color_program,
            solid_color_color_uniform: solid_color_color_uniform,

            solid_color_vertex_array: solid_color_vertex_array,
            solid_color_vertex_buffer: solid_color_vertex_buffer,
            solid_color_index_buffer: solid_color_index_buffer,

            atlas_size: atlas_size,

            main_coverage_buffer: main_coverage_buffer,
            fps_coverage_buffer: fps_coverage_buffer,
            main_compute_image: main_compute_image,
            main_gl_texture: main_gl_texture,
            fps_compute_image: fps_compute_image,
            fps_gl_texture: fps_gl_texture,

            query: query,
        }
    }

    fn redraw(&self,
              point_size: f32,
              font: &Font,
              outline_builder: &OutlineBuilder,
              outline_buffers: &OutlineBuffers,
              glyph_count: usize,
              glyph_positions: &[GlyphPos],
              device_pixel_size: &Size2D<u32>,
              translation: &Point2D<i32>)
              -> DrawAtlasProfilingEvents {
        let shelf_height = font.shelf_height(point_size);
        let mut atlas_builder = AtlasBuilder::new(ATLAS_SIZE, shelf_height);
        for glyph_index in 0..(glyph_count as u32) {
            atlas_builder.pack_glyph(&outline_builder, glyph_index, point_size).unwrap()
        }

        let atlas = atlas_builder.create_atlas(&outline_builder).unwrap();

        let rect = Rect::new(Point2D::new(0, 0), self.atlas_size);

        let events = self.rasterizer.draw_atlas(&self.main_compute_image,
                                                &rect,
                                                &atlas,
                                                outline_buffers,
                                                &self.main_coverage_buffer).unwrap();
        self.rasterizer.queue.flush().unwrap();

        unsafe {
            gl::Viewport(0,
                         0,
                         device_pixel_size.width as GLint,
                         device_pixel_size.height as GLint);
            gl::ClearColor(1.0, 1.0, 1.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        self.draw_glyphs(&font,
                         &mut atlas_builder,
                         outline_builder,
                         &self.main_composite_vertex_array,
                         glyph_positions,
                         device_pixel_size,
                         translation,
                         self.main_gl_texture,
                         point_size,
                         &TEXT_COLOR);

        events
    }

    fn get_timing_in_ms(&self) -> f64 {
        unsafe {
            let mut result = 0;
            gl::GetQueryObjectui64v(self.query, gl::QUERY_RESULT, &mut result);
            (result as f64) / (1_000_000.0)
        }
    }

    fn draw_glyphs(&self,
                   font: &Font,
                   atlas_builder: &mut AtlasBuilder,
                   outline_builder: &OutlineBuilder,
                   vertex_array: &CompositeVertexArray,
                   glyph_positions: &[GlyphPos],
                   device_pixel_size: &Size2D<u32>,
                   translation: &Point2D<i32>,
                   texture: GLuint,
                   point_size: f32,
                   color: &[f32]) {
        unsafe {
            gl::UseProgram(self.composite_program);
            gl::BindVertexArray(vertex_array.vertex_array);
            gl::BindBuffer(gl::ARRAY_BUFFER, vertex_array.vertex_buffer);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, vertex_array.index_buffer);

            let vertex_count = self.upload_quads_for_text(font,
                                                          atlas_builder,
                                                          outline_builder,
                                                          glyph_positions,
                                                          point_size);

            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_RECTANGLE, texture);
            gl::Uniform1i(self.composite_atlas_uniform, 0);

            let matrix = [
                2.0 / device_pixel_size.width as f32, 0.0,
                0.0, -2.0 / device_pixel_size.height as f32,
            ];
            gl::UniformMatrix2fv(self.composite_transform_uniform, 1, gl::FALSE, matrix.as_ptr());

            gl::Uniform2f(self.composite_translation_uniform,
                          -1.0 + 2.0 * translation.x as f32 / device_pixel_size.width as f32,
                          1.0 - 2.0 * translation.y as f32 / device_pixel_size.height as f32);

            gl::Uniform4fv(self.composite_color_uniform, 1, color.as_ptr());

            gl::Enable(gl::BLEND);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

            gl::BeginQuery(gl::TIME_ELAPSED, self.query);

            gl::DrawElements(gl::TRIANGLES,
                             vertex_count as GLsizei,
                             gl::UNSIGNED_SHORT,
                             0 as *const GLvoid);

            gl::EndQuery(gl::TIME_ELAPSED);
        }
    }

    fn upload_quads_for_text(&self,
                             font: &Font,
                             atlas_builder: &mut AtlasBuilder,
                             outline_builder: &OutlineBuilder,
                             glyph_positions: &[GlyphPos],
                             point_size: f32)
                             -> usize {
        let pixels_per_unit = point_size as f32 / font.units_per_em() as f32;

        let (mut vertices, mut indices) = (vec![], vec![]);
        for position in glyph_positions {
            let glyph_index = match atlas_builder.glyph_index_for(position.glyph_id) {
                None => continue,
                Some(glyph_index) => glyph_index,
            };

            let glyph_rect_i = outline_builder.glyph_pixel_bounds_i(glyph_index, point_size);

            let uv_tl: Point2D<u32> = atlas_builder.atlas_origin(glyph_index)
                                                   .floor()
                                                   .cast()
                                                   .unwrap();
            let uv_br = uv_tl + glyph_rect_i.size().cast().unwrap();

            let bearing_pos = (position.x as f32 * pixels_per_unit).round() as i32;
            let baseline_pos = (position.y as f32 * pixels_per_unit).round() as i32;

            let left_pos = bearing_pos + glyph_rect_i.left;
            let top_pos = baseline_pos - glyph_rect_i.top;
            let right_pos = bearing_pos + glyph_rect_i.right;
            let bottom_pos = baseline_pos - glyph_rect_i.bottom;

            let first_index = vertices.len() as u16;

            vertices.push(Vertex::new(left_pos,  top_pos,    uv_tl.x, uv_tl.y));
            vertices.push(Vertex::new(right_pos, top_pos,    uv_br.x, uv_tl.y));
            vertices.push(Vertex::new(right_pos, bottom_pos, uv_br.x, uv_br.y));
            vertices.push(Vertex::new(left_pos,  bottom_pos, uv_tl.x, uv_br.y));

            indices.extend(RECT_INDICES.iter().map(|index| first_index + index));
        }

        unsafe {
            gl::BufferData(gl::ARRAY_BUFFER,
                           (vertices.len() * mem::size_of::<Vertex>()) as GLsizeiptr,
                           vertices.as_ptr() as *const GLvoid,
                           gl::STATIC_DRAW);
            gl::BufferData(gl::ELEMENT_ARRAY_BUFFER,
                           (indices.len() * mem::size_of::<u16>()) as GLsizeiptr,
                           indices.as_ptr() as *const GLvoid,
                           gl::STATIC_DRAW);
        }

        indices.len()
    }

    fn create_fps_atlas(&self,
                        font: &Font,
                        outline_builder: &OutlineBuilder,
                        outline_buffers: &OutlineBuffers,
                        glyph_count: usize)
                        -> AtlasBuilder {
        let shelf_height = font.shelf_height(FPS_DISPLAY_POINT_SIZE);
        let mut atlas_builder = AtlasBuilder::new(ATLAS_SIZE, shelf_height);
        for glyph_index in 0..(glyph_count as u32) {
            atlas_builder.pack_glyph(&outline_builder, glyph_index, FPS_DISPLAY_POINT_SIZE)
                         .unwrap()
        }

        let atlas = atlas_builder.create_atlas(&outline_builder).unwrap();

        let rect = Rect::new(Point2D::new(0, 0), self.atlas_size);

        self.rasterizer.draw_atlas(&self.fps_compute_image,
                                   &rect,
                                   &atlas,
                                   outline_buffers,
                                   &self.fps_coverage_buffer).unwrap();
        
        atlas_builder
    }

    fn draw_fps(&self,
                font: &Font,
                atlas_builder: &mut AtlasBuilder,
                outline_builder: &OutlineBuilder,
                device_pixel_size: &Size2D<u32>,
                glyph_ranges: &GlyphRanges,
                draw_time: f64,
                accum_time: f64,
                composite_time: f64,
                glyph_count: usize) {
        // Draw the background color.
        unsafe {
            gl::BindVertexArray(self.solid_color_vertex_array);
            gl::UseProgram(self.solid_color_program);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.solid_color_vertex_buffer);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.solid_color_index_buffer);

            let tl = Point2D::new(
                -1.0,
                -1.0 + (FPS_DISPLAY_POINT_SIZE + FPS_PADDING as f32 * 2.0) /
                    (device_pixel_size.height as f32) * 2.0);
            let br = Point2D::new(1.0, -1.0);

            let vertices = [(tl.x, tl.y), (br.x, tl.y), (br.x, br.y), (tl.x, br.y)];
            gl::BufferData(gl::ARRAY_BUFFER,
                           (vertices.len() * mem::size_of::<(f32, f32)>()) as GLsizeiptr,
                           vertices.as_ptr() as *const GLvoid,
                           gl::DYNAMIC_DRAW);

            gl::Uniform4fv(self.solid_color_color_uniform, 1, FPS_BACKGROUND_COLOR.as_ptr());

            gl::Enable(gl::BLEND);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

            gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_SHORT, 0 as *const GLvoid);
        }

        let fps_text = format!("draw: {:.3}ms ({:.3}us/glyph), \
                                accum: {:.3}ms ({:.3}us/glyph), \
                                composite: {:.3}ms ({:.3}us/glyph)",
                               draw_time / 1_000_000.0,
                               draw_time / (1000.0 * glyph_count as f64),
                               accum_time / 1_000_000.0,
                               accum_time / (1000.0 * glyph_count as f64),
                               composite_time,
                               (composite_time * 1000.0) / (glyph_count as f64));

        let mut fps_glyphs = vec![];
        let mut current_x = 0;
        for glyph_pos in &shaper::shape_text(&font, &glyph_ranges, &fps_text) {
            current_x += glyph_pos.advance as u32;
            fps_glyphs.push(GlyphPos {
                x: current_x,
                y: 0,
                glyph_id: glyph_pos.glyph_id,
            });
        }

        self.draw_glyphs(font,
                         atlas_builder,
                         outline_builder,
                         &self.fps_composite_vertex_array,
                         &fps_glyphs,
                         device_pixel_size,
                         &Point2D::new(FPS_PADDING, device_pixel_size.height as i32 - FPS_PADDING),
                         self.fps_gl_texture,
                         FPS_DISPLAY_POINT_SIZE,
                         &FPS_FOREGROUND_COLOR);
    }

    fn take_screenshot(&self) {
        unsafe {
            let mut fbo = 0;
            gl::GenFramebuffers(1, &mut fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
            gl::FramebufferTexture2D(gl::FRAMEBUFFER,
                                     gl::COLOR_ATTACHMENT0,
                                     gl::TEXTURE_RECTANGLE,
                                     self.main_gl_texture,
                                     0);

            let length = 4 * self.atlas_size.width as usize * self.atlas_size.height as usize;
            let mut pixels: Vec<u8> = vec![0; length];
            gl::ReadPixels(0, 0,
                           self.atlas_size.width as GLint, self.atlas_size.height as GLint,
                           gl::RGBA,
                           gl::UNSIGNED_BYTE,
                           pixels.as_mut_ptr() as *mut c_void);

            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::DeleteFramebuffers(1, &mut fbo);

            image::save_buffer(&Path::new(ATLAS_DUMP_FILENAME),
                               &pixels,
                               self.atlas_size.width,
                               self.atlas_size.height,
                               image::RGBA(8)).unwrap();
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct Vertex {
    x: i32,
    y: i32,
    u: u32,
    v: u32,
}

impl Vertex {
    fn new(x: i32, y: i32, u: u32, v: u32) -> Vertex {
        Vertex {
            x: x,
            y: y,
            u: u,
            v: v,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct GlyphPos {
    x: u32,
    y: u32,
    glyph_id: u16,
}

#[derive(Debug)]
struct CompositeVertexArray {
    vertex_array: GLuint,
    vertex_buffer: GLuint,
    index_buffer: GLuint,
}

impl CompositeVertexArray {
    fn new() -> CompositeVertexArray {
        let (mut vertex_array, mut vertex_buffer, mut index_buffer) = (0, 0, 0);

        unsafe {
            gl::GenVertexArrays(1, &mut vertex_array);
            gl::GenBuffers(1, &mut vertex_buffer);
            gl::GenBuffers(1, &mut index_buffer);
        }

        CompositeVertexArray {
            vertex_array: vertex_array,
            vertex_buffer: vertex_buffer,
            index_buffer: index_buffer,
        }
    }
}

fn create_program(vertex_shader_source: &str, fragment_shader_source: &str) -> GLuint {
    unsafe {
        let vertex_shader = gl::CreateShader(gl::VERTEX_SHADER);
        let fragment_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
        gl::ShaderSource(vertex_shader,
                         1,
                         &(vertex_shader_source.as_ptr() as *const u8 as *const GLchar),
                         &(vertex_shader_source.len() as GLint));
        gl::ShaderSource(fragment_shader,
                         1,
                         &(fragment_shader_source.as_ptr() as *const u8 as *const GLchar),
                         &(fragment_shader_source.len() as GLint));
        gl::CompileShader(vertex_shader);
        gl::CompileShader(fragment_shader);

        let program = gl::CreateProgram();
        gl::AttachShader(program, vertex_shader);
        gl::AttachShader(program, fragment_shader);
        gl::LinkProgram(program);
        program
    }
}

fn create_image(rasterizer: &Rasterizer, atlas_size: &Size2D<u32>) -> (Image, GLuint) {
    let compute_image = rasterizer.device.create_image(Format::R8,
                                                       buffer::Protection::WriteOnly,
                                                       &atlas_size).unwrap();

    let mut gl_texture = 0;
    unsafe {
        gl::GenTextures(1, &mut gl_texture);
        compute_image.bind_to(&ExternalImage::GlTexture(gl_texture)).unwrap();

        gl::BindTexture(gl::TEXTURE_RECTANGLE, gl_texture);
        gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
        gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);
    }

    (compute_image, gl_texture)
}

fn usage() -> ! {
    println!("usage: lorem-ipsum /path/to/font.ttf [/path/to/text.txt]");
    process::exit(0)
}

static COMPOSITE_VERTEX_SHADER: &'static str = "\
#version 330

uniform mat2 uTransform;
uniform vec2 uTranslation;

in vec2 aPosition;
in vec2 aTexCoord;

out vec2 vTexCoord;

void main() {
    vTexCoord = aTexCoord;
    gl_Position = vec4(uTransform * aPosition + uTranslation, 0.0f, 1.0f);
}
";

static COMPOSITE_FRAGMENT_SHADER: &'static str = "\
#version 330

uniform sampler2DRect uAtlas;
uniform vec4 uColor;

in vec2 vTexCoord;

out vec4 oFragColor;

void main() {
    float value = texture(uAtlas, vTexCoord).r;
    oFragColor = vec4(uColor.rgb, uColor.a * value);
}
";

static SOLID_COLOR_VERTEX_SHADER: &'static str = "\
#version 330

in vec2 aPosition;

void main() {
    gl_Position = vec4(aPosition, 0.0f, 1.0f);
}
";

static SOLID_COLOR_FRAGMENT_SHADER: &'static str = "\
#version 330

uniform vec4 uColor;

out vec4 oFragColor;

void main() {
    oFragColor = uColor;
}
";

static RECT_INDICES: [u16; 6] = [0, 1, 3, 1, 2, 3];

static TEXT: &'static str = "\
Lorem ipsum dolor sit amet, consectetur adipiscing elit. Curabitur scelerisque pellentesque risus quis vehicula. Ut sollicitudin aliquet diam, vel lobortis orci porta in. Sed eu nisi egestas odio tincidunt cursus eget ut lorem. Fusce lacinia ex nec lectus rutrum mollis. Donec in ultrices purus. Integer id suscipit magna. Suspendisse congue pulvinar neque id ultrices. Curabitur nec tellus et est pellentesque posuere. Duis ut metus euismod, feugiat arcu vitae, posuere libero. \
Curabitur nunc urna, rhoncus vitae scelerisque quis, viverra et odio. Suspendisse accumsan pretium mi, nec fringilla metus condimentum id. Duis dignissim quam eu felis lobortis, eget dignissim lectus fermentum. Nunc et massa id orci pellentesque rutrum. Nam imperdiet quam vel ligula efficitur ultricies vel eu tellus. Maecenas luctus risus a erat euismod ultricies. Pellentesque neque mauris, laoreet vitae finibus quis, molestie ut velit. Donec laoreet justo risus. In id mi sed odio placerat interdum ut vitae erat. Fusce quis mollis mauris, sit amet efficitur libero. \
In efficitur tortor nulla, sollicitudin sodales mi tempor in. In egestas ultrices fermentum. Quisque mattis egestas nulla. Interdum et malesuada fames ac ante ipsum primis in faucibus. Etiam in tempus sapien, in dignissim arcu. Quisque diam nulla, rhoncus et tempor nec, facilisis porta purus. Nulla ut eros laoreet, placerat dolor ut, interdum orci. Sed posuere eleifend mollis. Integer at nunc ex. Vestibulum aliquet risus quis lacinia convallis. Fusce et metus viverra, varius nulla in, rutrum justo. Interdum et malesuada fames ac ante ipsum primis in faucibus. Praesent non est vel lectus suscipit malesuada id ut nisl. Aenean sem ipsum, tincidunt non orci non, varius consectetur purus. Aenean sed mollis turpis, sit amet vestibulum risus. Nunc ut hendrerit urna, sit amet lacinia arcu. \
Curabitur laoreet a enim et eleifend. Etiam consectetur pharetra massa, sed elementum quam molestie nec. Integer eu justo lectus. Vestibulum sed vulputate sapien. Curabitur pretium luctus orci et interdum. Quisque ligula nisi, varius id sodales id, volutpat et lorem. Pellentesque ex urna, malesuada at ex non, elementum ultricies nulla. Nunc sodales, turpis at maximus bibendum, neque lorem laoreet felis, eget convallis sem mauris ac quam. Mauris non pretium nulla. Nam semper pulvinar convallis. Suspendisse ultricies odio vitae tortor congue, rutrum finibus nisl malesuada. Interdum et malesuada fames ac ante ipsum primis in faucibus. \
Vestibulum aliquam et lacus sit amet lobortis. In sed ligula quis urna accumsan vehicula sit amet id magna. Cras mollis orci vitae turpis porta, sed gravida nunc aliquam. Phasellus nec facilisis nunc. Suspendisse volutpat leo felis, in iaculis nisi dignissim et. Phasellus at urna purus. Nullam vitae metus ante. Praesent porttitor libero quis velit fermentum rhoncus. Cras vitae rhoncus nulla. In efficitur risus sapien, sed viverra neque scelerisque at. Morbi fringilla odio massa. Donec tincidunt magna diam, eget congue leo tristique eget. Cras et sapien nulla.\
";

