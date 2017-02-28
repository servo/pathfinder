/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

extern crate clap;
extern crate compute_shader;
extern crate euclid;
extern crate gl;
extern crate glfw;
extern crate image;
extern crate memmap;
extern crate pathfinder;

use clap::{App, Arg};
use compute_shader::buffer;
use compute_shader::image::{Color, ExternalImage, Format, Image};
use compute_shader::instance::{Instance, ShadingLanguage};
use euclid::{Point2D, Rect, Size2D};
use gl::types::{GLchar, GLint, GLsizei, GLsizeiptr, GLuint, GLvoid};
use glfw::{Action, Context, Key, OpenGlProfileHint, SwapInterval, Window, WindowEvent};
use glfw::{Glfw, WindowHint, WindowMode};
use memmap::{Mmap, Protection};
use pathfinder::atlas::{AtlasBuilder, AtlasOptions, GlyphRasterizationOptions};
use pathfinder::charmap::CodepointRanges;
use pathfinder::coverage::{CoverageBuffer, CoverageBufferOptions};
use pathfinder::error::RasterError;
use pathfinder::font::Font;
use pathfinder::rasterizer::{DrawAtlasProfilingEvents, Rasterizer, RasterizerOptions};
use pathfinder::typesetter::{GlyphStore, PositionedGlyph, Typesetter};
use std::char;
use std::env;
use std::f32;
use std::fs::File;
use std::io::Read;
use std::mem;
use std::os::raw::c_void;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

const ATLAS_SIZE: u32 = 2048;
const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
const SCROLL_SPEED: f64 = 6.0;

const SUBPIXEL_GRANULARITY_COUNT: u8 = 4;
const SUBPIXEL_GRANULARITY: f32 = 1.0 / SUBPIXEL_GRANULARITY_COUNT as f32;

const INITIAL_POINT_SIZE: f32 = 24.0;
const MIN_POINT_SIZE: f32 = 6.0;
const MAX_POINT_SIZE: f32 = 512.0;

const FPS_DISPLAY_POINT_SIZE: f32 = 24.0;
const FPS_PADDING: i32 = 6;

static PATHFINDER_SHADER_PATH: &'static str = "resources/shaders/";
static EXAMPLE_SHADER_PATH: &'static str = "resources/examples/lorem-ipsum/";
static DEFAULT_TEXT_PATH: &'static str = "resources/examples/lorem-ipsum/default.txt";

static FPS_BACKGROUND_COLOR: [f32; 3] = [0.3, 0.3, 0.3];
static FPS_FOREGROUND_COLOR: [f32; 3] = [1.0, 1.0, 1.0];
static BACKGROUND_COLOR:     [f32; 3] = [1.0, 1.0, 1.0];
static TEXT_COLOR:           [f32; 3] = [0.0, 0.0, 0.0];

static ATLAS_DUMP_FILENAME: &'static str = "lorem-ipsum-atlas.png";

static RECT_INDICES: [u16; 6] = [0, 1, 3, 1, 2, 3];

fn main() {
    DemoApp::new(AppOptions::parse()).run()
}

struct DemoApp {
    options: AppOptions,
    renderer: Renderer,
    glfw: Glfw,
    window: Window,
    events: Receiver<(f64, WindowEvent)>,
    point_size: f32,
    translation: Point2D<i32>,
    device_pixel_size: Size2D<u32>,
}

impl DemoApp {
    fn new(options: AppOptions) -> DemoApp {
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
        glfw.set_swap_interval(SwapInterval::Sync(1));

        gl::load_with(|symbol| window.get_proc_address(symbol) as *const c_void);

        let (width, height) = window.get_framebuffer_size();
        let device_pixel_size = Size2D::new(width as u32, height as u32);

        let renderer = Renderer::new(options.subpixel_aa);

        DemoApp {
            renderer: renderer,
            glfw: glfw,
            window: window,
            events: events,
            device_pixel_size: device_pixel_size,
            point_size: INITIAL_POINT_SIZE,
            translation: Point2D::zero(),
            options: options,
        }
    }

    fn run(&mut self) {
        let file = Mmap::open_path(&self.options.font_path, Protection::Read).unwrap();
        let mut buffer = vec![];
        let font = unsafe {
            Font::from_collection_index(file.as_slice(),
                                        self.options.font_index,
                                        &mut buffer).unwrap()
        };

        let page_width = self.device_pixel_size.width as f32 *
            font.units_per_em() as f32 / INITIAL_POINT_SIZE;

        let mut typesetter = Typesetter::new(page_width, &font, font.units_per_em() as f32);
        typesetter.add_text(&font, font.units_per_em() as f32, &self.options.text);

        let glyph_stores = GlyphStores::new(&typesetter, &font);

        let mut dirty = true;
        while !self.window.should_close() {
            if dirty {
                self.redraw(&glyph_stores, &typesetter, &font);
                dirty = false
            }

            self.glfw.wait_events();
            let events: Vec<_> = glfw::flush_messages(&self.events).map(|(_, e)| e).collect();
            for event in events {
                dirty = self.handle_window_event(event) || dirty
            }
        }
    }

    fn redraw(&mut self, glyph_stores: &GlyphStores, typesetter: &Typesetter, font: &Font) {
        let redraw_result = self.renderer.redraw(self.point_size,
                                                 &font,
                                                 &glyph_stores.main,
                                                 typesetter,
                                                 &self.device_pixel_size,
                                                 &self.translation);

        let (draw_time, accum_time);
        match redraw_result.events {
            Some(events) => {
                let mut draw_nanos = 0u64;
                unsafe {
                    gl::Flush();
                    gl::GetQueryObjectui64v(events.draw, gl::QUERY_RESULT, &mut draw_nanos);
                }

                draw_time = draw_nanos as f64;
                accum_time = events.accum.time_elapsed().unwrap() as f64;
            }
            None => {
                draw_time = 0.0;
                accum_time = 0.0;
            }
        }

        let timing = self.renderer.get_timing_in_ms();

        self.renderer.draw_fps(&font,
                               &glyph_stores.fps,
                               &self.device_pixel_size,
                               draw_time,
                               accum_time,
                               timing,
                               redraw_result.glyphs_drawn);

        self.window.swap_buffers();
    }

    // Returns true if the window needs to be redrawn.
    fn handle_window_event(&mut self, event: WindowEvent) -> bool {
        match event {
            WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                self.window.set_should_close(true);
                false
            }
            WindowEvent::Key(Key::S, _, Action::Press, _) => {
                self.renderer.take_screenshot();
                println!("wrote screenshot to: {}", ATLAS_DUMP_FILENAME);
                false
            }
            WindowEvent::Scroll(x, y) => {
                if self.window.get_key(Key::LeftAlt) == Action::Press ||
                        self.window.get_key(Key::RightAlt) == Action::Press {
                    let old_point_size = self.point_size;
                    self.point_size = old_point_size + y as f32;

                    if self.point_size < MIN_POINT_SIZE {
                        self.point_size = MIN_POINT_SIZE
                    } else if self.point_size > MAX_POINT_SIZE {
                        self.point_size = MAX_POINT_SIZE
                    }

                    let mut center =
                        Point2D::new(self.translation.x as f32 -
                                     self.device_pixel_size.width as f32 *
                                     0.5,
                                     self.translation.y as f32 -
                                     self.device_pixel_size.height as f32 *
                                     0.5);
                    center.x = center.x * self.point_size / old_point_size;
                    center.y = center.y * self.point_size / old_point_size;

                    self.translation.x =
                        (center.x + self.device_pixel_size.width as f32 * 0.5).round() as i32;
                    self.translation.y =
                        (center.y + self.device_pixel_size.height as f32 * 0.5).round() as i32;
                } else {
                    self.translation.x += (x * SCROLL_SPEED).round() as i32;
                    self.translation.y += (y * SCROLL_SPEED).round() as i32;
                }

                true
            }
            WindowEvent::Size(_, _) | WindowEvent::FramebufferSize(_, _) => {
                let (width, height) = self.window.get_framebuffer_size();
                self.device_pixel_size = Size2D::new(width as u32, height as u32);
                true
            }
            _ => false,
        }
    }
}

struct AppOptions {
    font_path: String,
    font_index: u32,
    text: String,
    subpixel_aa: bool,
}

impl AppOptions {
    fn parse() -> AppOptions {
        let index_arg = Arg::with_name("index").short("i")
                                               .long("index")
                                               .help("Select an index within a font collection")
                                               .takes_value(true);
        let subpixel_antialiasing_arg =
            Arg::with_name("subpixel-aa").short("s")
                                         .long("subpixel-aa")
                                         .help("Enable subpixel antialiasing");
        let font_arg =
            Arg::with_name("FONT-FILE").help("Select the font file (`.ttf`, `.otf`, etc.)")
                                       .required(true)
                                       .index(1);
        let text_arg = Arg::with_name("TEXT-FILE").help("Select a file containing text to display")
                                                  .index(2);
        let matches = App::new("lorem-ipsum").arg(index_arg)
                                             .arg(subpixel_antialiasing_arg)
                                             .arg(font_arg)
                                             .arg(text_arg)
                                             .get_matches();

        let mut text = "".to_string();
        let path = matches.value_of("TEXT-FILE").unwrap_or(DEFAULT_TEXT_PATH);
        File::open(path).unwrap().read_to_string(&mut text).unwrap();
        text = text.replace(&['\n', '\r', '\t'][..], " ");

        let font_index = match matches.value_of("index") {
            Some(index) => index.parse().unwrap(),
            None => 0,
        };

        let font_path = matches.value_of("FONT-FILE").unwrap();
        let subpixel_aa = matches.is_present("subpixel-aa");

        AppOptions {
            text: text,
            font_index: font_index,
            font_path: font_path.to_string(),
            subpixel_aa: subpixel_aa,
        }
    }
}

struct GlyphStores {
    main: GlyphStore,
    fps: GlyphStore,
}

impl GlyphStores {
    fn new(typesetter: &Typesetter, font: &Font) -> GlyphStores {
        let main_glyph_store = typesetter.create_glyph_store(&font).unwrap();

        let mut fps_chars: Vec<char> = vec![];
        fps_chars.extend(" ./,:()".chars());
        fps_chars.extend(('A' as u32..('Z' as u32 + 1)).flat_map(char::from_u32));
        fps_chars.extend(('a' as u32..('z' as u32 + 1)).flat_map(char::from_u32));
        fps_chars.extend(('0' as u32..('9' as u32 + 1)).flat_map(char::from_u32));
        fps_chars.sort();
        let fps_codepoint_ranges = CodepointRanges::from_sorted_chars(&fps_chars);
        let fps_glyph_store = GlyphStore::from_codepoints(&fps_codepoint_ranges, &font).unwrap();

        GlyphStores {
            main: main_glyph_store,
            fps: fps_glyph_store,
        }
    }
}

struct Renderer {
    rasterizer: Rasterizer,

    composite_program: GLuint,
    composite_atlas_uniform: GLint,
    composite_transform_uniform: GLint,
    composite_translation_uniform: GLint,
    composite_foreground_color_uniform: GLint,
    composite_background_color_uniform: GLint,

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

    shading_language: ShadingLanguage,

    subpixel_aa: bool,
}

impl Renderer {
    fn new(subpixel_aa: bool) -> Renderer {
        let instance = Instance::new().unwrap();
        let device = instance.open_device().unwrap();
        let queue = device.create_queue().unwrap();

        let mut rasterizer_options = RasterizerOptions::from_env().unwrap();
        if env::var("PATHFINDER_SHADER_PATH").is_err() {
            rasterizer_options.shader_path = PathBuf::from(PATHFINDER_SHADER_PATH)
        }

        let rasterizer = Rasterizer::new(&instance, device, queue, rasterizer_options).unwrap();

        let (composite_program, composite_position_attribute, composite_tex_coord_attribute);
        let (composite_atlas_uniform, composite_transform_uniform);
        let (composite_translation_uniform, composite_foreground_color_uniform);
        let composite_background_color_uniform;
        let (main_composite_vertex_array, fps_composite_vertex_array);
        let (solid_color_program, solid_color_position_attribute, solid_color_color_uniform);
        let (mut solid_color_vertex_buffer, mut solid_color_index_buffer) = (0, 0);
        let mut solid_color_vertex_array = 0;
        unsafe {
            composite_program = create_program("composite");
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
            composite_foreground_color_uniform =
                gl::GetUniformLocation(composite_program,
                                       "uForegroundColor\0".as_ptr() as *const GLchar);
            composite_background_color_uniform =
                gl::GetUniformLocation(composite_program,
                                       "uBackgroundColor\0".as_ptr() as *const GLchar);

            solid_color_program = create_program("solid_color");
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
                                        gl::FLOAT,
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

        // FIXME(pcwalton): Dynamically resizing atlas.
        let atlas_size = Size2D::new(ATLAS_SIZE, ATLAS_SIZE);
        let coverage_buffer_options = CoverageBufferOptions {
            size: atlas_size,
            subpixel_antialiasing: subpixel_aa,
            ..CoverageBufferOptions::default()
        };

        let main_coverage_buffer = CoverageBuffer::new(rasterizer.device(),
                                                       &coverage_buffer_options).unwrap();
        let fps_coverage_buffer = CoverageBuffer::new(rasterizer.device(),
                                                      &coverage_buffer_options).unwrap();

        let (main_compute_image, main_gl_texture) = create_image(&rasterizer, &atlas_size);
        let (fps_compute_image, fps_gl_texture) = create_image(&rasterizer, &atlas_size);

        let mut query = 0;
        unsafe {
            gl::GenQueries(1, &mut query);
        }

        let shading_language = instance.shading_language();

        Renderer {
            rasterizer: rasterizer,

            composite_program: composite_program,
            composite_atlas_uniform: composite_atlas_uniform,
            composite_transform_uniform: composite_transform_uniform,
            composite_translation_uniform: composite_translation_uniform,
            composite_foreground_color_uniform: composite_foreground_color_uniform,
            composite_background_color_uniform: composite_background_color_uniform,

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

            shading_language: shading_language,

            subpixel_aa: subpixel_aa,
        }
    }

    fn redraw(&self,
              point_size: f32,
              font: &Font,
              glyph_store: &GlyphStore,
              typesetter: &Typesetter,
              device_pixel_size: &Size2D<u32>,
              translation: &Point2D<i32>)
              -> RedrawResult {
        let shelf_height = font.shelf_height(point_size);
        let atlas_options = AtlasOptions {
            available_width: ATLAS_SIZE,
            shelf_height: shelf_height,
            subpixel_antialiasing: self.subpixel_aa,
            ..AtlasOptions::default()
        };

        let mut atlas_builder = AtlasBuilder::new(&atlas_options);

        let (positioned_glyphs, cached_glyphs) = self.determine_visible_glyphs(&mut atlas_builder,
                                                                               font,
                                                                               glyph_store,
                                                                               typesetter,
                                                                               device_pixel_size,
                                                                               translation,
                                                                               point_size);

        let atlas = atlas_builder.create_atlas().unwrap();
        let rect = Rect::new(Point2D::new(0, 0), self.atlas_size);

        let events = match self.rasterizer.draw_atlas(&self.main_compute_image,
                                                      &rect,
                                                      &atlas,
                                                      &glyph_store.outlines,
                                                      &self.main_coverage_buffer) {
            Ok(events) => Some(events),
            Err(RasterError::NoGlyphsToDraw) => None,
            Err(error) => panic!("Failed to rasterize atlas: {:?}", error),
        };

        self.rasterizer.queue().flush().unwrap();

        unsafe {
            if self.shading_language == ShadingLanguage::Glsl {
                gl::MemoryBarrier(gl::SHADER_IMAGE_ACCESS_BARRIER_BIT |
                                  gl::TEXTURE_FETCH_BARRIER_BIT);
            }

            gl::Viewport(0,
                         0,
                         device_pixel_size.width as GLint,
                         device_pixel_size.height as GLint);
            gl::ClearColor(BACKGROUND_COLOR[0], BACKGROUND_COLOR[1], BACKGROUND_COLOR[2], 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        if events.is_some() {
            self.draw_glyphs(glyph_store,
                             &self.main_composite_vertex_array,
                             &positioned_glyphs,
                             &cached_glyphs,
                             device_pixel_size,
                             translation,
                             self.main_gl_texture,
                             point_size,
                             &TEXT_COLOR,
                             &BACKGROUND_COLOR)
        }

        RedrawResult {
            events: events,
            glyphs_drawn: cached_glyphs.len() as u32,
        }
    }

    fn determine_visible_glyphs(&self,
                                atlas_builder: &mut AtlasBuilder,
                                font: &Font,
                                glyph_store: &GlyphStore,
                                typesetter: &Typesetter,
                                device_pixel_size: &Size2D<u32>,
                                translation: &Point2D<i32>,
                                point_size: f32)
                                -> (Vec<PositionedGlyph>, Vec<CachedGlyph>) {
        let viewport = Rect::new(-translation.cast().unwrap(), device_pixel_size.cast().unwrap());

        let scale = point_size / font.units_per_em() as f32;
        let positioned_glyphs = typesetter.positioned_glyphs_in_rect(&viewport,
                                                                     glyph_store,
                                                                     font.units_per_em() as f32,
                                                                     scale,
                                                                     SUBPIXEL_GRANULARITY);

        let mut glyphs: Vec<_> = positioned_glyphs.iter().map(|positioned_glyph| {
            (positioned_glyph.glyph_index,
             (positioned_glyph.subpixel_x / SUBPIXEL_GRANULARITY).round() as u8)
        }).collect();

        glyphs.sort();
        glyphs.dedup();

        let cached_glyphs = glyphs.iter().map(|&(glyph_index, subpixel)| {
            let subpixel_offset = (subpixel as f32) / (SUBPIXEL_GRANULARITY as f32);
            let origin = atlas_builder.pack_glyph(&glyph_store.outlines,
                                                  glyph_index,
                                                  &GlyphRasterizationOptions {
                                                      point_size: point_size,
                                                      horizontal_offset: subpixel_offset,
                                                      ..GlyphRasterizationOptions::default()
                                                  }).unwrap();
            CachedGlyph {
                x: origin.x,
                y: origin.y,
                glyph_index: glyph_index,
                subpixel: subpixel,
            }
        }).collect();

        (positioned_glyphs, cached_glyphs)
    }

    fn get_timing_in_ms(&self) -> f64 {
        unsafe {
            let mut result = 0;
            gl::GetQueryObjectui64v(self.query, gl::QUERY_RESULT, &mut result);
            (result as f64) / (1_000_000.0)
        }
    }

    fn draw_glyphs(&self,
                   glyph_store: &GlyphStore,
                   vertex_array: &CompositeVertexArray,
                   positioned_glyphs: &[PositionedGlyph],
                   cached_glyphs: &[CachedGlyph],
                   device_pixel_size: &Size2D<u32>,
                   translation: &Point2D<i32>,
                   texture: GLuint,
                   point_size: f32,
                   foreground_color: &[f32],
                   background_color: &[f32]) {
        unsafe {
            gl::UseProgram(self.composite_program);
            gl::BindVertexArray(vertex_array.vertex_array);
            gl::BindBuffer(gl::ARRAY_BUFFER, vertex_array.vertex_buffer);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, vertex_array.index_buffer);

            let vertex_count = self.upload_quads_for_text(glyph_store,
                                                          positioned_glyphs,
                                                          cached_glyphs,
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

            gl::Uniform3fv(self.composite_foreground_color_uniform, 1, foreground_color.as_ptr());
            gl::Uniform3fv(self.composite_background_color_uniform, 1, background_color.as_ptr());

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
                             glyph_store: &GlyphStore,
                             positioned_glyphs: &[PositionedGlyph],
                             cached_glyphs: &[CachedGlyph],
                             point_size: f32)
                             -> usize {
        let (mut vertices, mut indices) = (vec![], vec![]);
        for positioned_glyph in positioned_glyphs {
            let glyph_index = positioned_glyph.glyph_index;
            let glyph_rect = glyph_store.outlines.glyph_subpixel_bounds(glyph_index, point_size);

            let subpixel = (positioned_glyph.subpixel_x / SUBPIXEL_GRANULARITY).round() as u8;

            let glyph_rect_i = glyph_rect.round_out();
            let glyph_size_i = glyph_rect_i.size();

            let cached_glyph_index = cached_glyphs.binary_search_by(|cached_glyph| {
                (cached_glyph.glyph_index, cached_glyph.subpixel).cmp(&(glyph_index, subpixel))
            }).expect("Didn't cache the glyph properly!");
            let cached_glyph = cached_glyphs[cached_glyph_index];

            let uv_tl: Point2D<u32> = Point2D::new(cached_glyph.x,
                                                   cached_glyph.y).floor().cast().unwrap();
            let uv_br = uv_tl + glyph_size_i.cast().unwrap();

            let left_pos = positioned_glyph.bounds.origin.x;
            let top_pos = positioned_glyph.bounds.origin.y;
            let right_pos = positioned_glyph.bounds.origin.x + glyph_size_i.width as f32;
            let bottom_pos = positioned_glyph.bounds.origin.y + glyph_size_i.height as f32;

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

    fn draw_fps(&self,
                font: &Font,
                fps_glyph_store: &GlyphStore,
                device_pixel_size: &Size2D<u32>,
                draw_time: f64,
                accum_time: f64,
                composite_time: f64,
                glyphs_drawn: u32) {
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

            gl::Uniform3fv(self.solid_color_color_uniform, 1, FPS_BACKGROUND_COLOR.as_ptr());

            gl::Enable(gl::BLEND);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

            gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_SHORT, 0 as *const GLvoid);
        }

        let fps_text = format!("draw: {:.3}ms ({:.3}us/glyph), \
                                accum: {:.3}ms ({:.3}us/glyph), \
                                composite: {:.3}ms ({:.3}us/glyph)",
                               draw_time / 1_000_000.0,
                               draw_time / (1000.0 * glyphs_drawn as f64),
                               accum_time / 1_000_000.0,
                               accum_time / (1000.0 * glyphs_drawn as f64),
                               composite_time,
                               (composite_time * 1000.0) / (glyphs_drawn as f64));

        // TODO(pcwalton): Subpixel positioning for the FPS display.
        let mut fps_typesetter = Typesetter::new(f32::INFINITY, &font, font.units_per_em() as f32);
        fps_typesetter.add_text(&font, font.units_per_em() as f32, &fps_text);

        let shelf_height = font.shelf_height(FPS_DISPLAY_POINT_SIZE);
        let atlas_options = AtlasOptions {
            available_width: ATLAS_SIZE,
            shelf_height: shelf_height,
            subpixel_antialiasing: self.subpixel_aa,
            ..AtlasOptions::default()
        };

        let mut fps_atlas_builder = AtlasBuilder::new(&atlas_options);

        let mut fps_glyphs = vec![];
        for &fps_glyph_index in &fps_glyph_store.all_glyph_indices {
            for subpixel in 0..SUBPIXEL_GRANULARITY_COUNT {
                let subpixel_increment = SUBPIXEL_GRANULARITY * subpixel as f32;
                let options = GlyphRasterizationOptions {
                    point_size: FPS_DISPLAY_POINT_SIZE,
                    horizontal_offset: subpixel_increment,
                    ..GlyphRasterizationOptions::default()
                };

                let origin = fps_atlas_builder.pack_glyph(&fps_glyph_store.outlines,
                                                          fps_glyph_index,
                                                          &options).unwrap();
                fps_glyphs.push(CachedGlyph {
                    x: origin.x,
                    y: origin.y,
                    glyph_index: fps_glyph_index,
                    subpixel: subpixel,
                })
            }
        }

        let fps_atlas = fps_atlas_builder.create_atlas().unwrap();
        let rect = Rect::new(Point2D::new(0, 0), self.atlas_size);
        self.rasterizer.draw_atlas(&self.fps_compute_image,
                                   &rect,
                                   &fps_atlas,
                                   &fps_glyph_store.outlines,
                                   &self.fps_coverage_buffer).unwrap();
        self.rasterizer.queue().flush().unwrap();

        let fps_pixels_per_unit = FPS_DISPLAY_POINT_SIZE / font.units_per_em() as f32;
        let fps_line_spacing = ((font.ascender() as f32 - font.descender() as f32 +
                                 font.line_gap() as f32) * fps_pixels_per_unit).round() as i32;

        let fps_left = FPS_PADDING;
        let fps_top = device_pixel_size.height as i32 - FPS_PADDING - fps_line_spacing;

        let fps_viewport = Rect::new(Point2D::zero(), device_pixel_size.cast().unwrap());
        let fps_scale = FPS_DISPLAY_POINT_SIZE / font.units_per_em() as f32;

        let fps_positioned_glyphs =
            fps_typesetter.positioned_glyphs_in_rect(&fps_viewport,
                                                     fps_glyph_store,
                                                     font.units_per_em() as f32,
                                                     fps_scale,
                                                     SUBPIXEL_GRANULARITY);

        self.draw_glyphs(&fps_glyph_store,
                         &self.fps_composite_vertex_array,
                         &fps_positioned_glyphs,
                         &fps_glyphs,
                         device_pixel_size,
                         &Point2D::new(fps_left, fps_top),
                         self.fps_gl_texture,
                         FPS_DISPLAY_POINT_SIZE,
                         &FPS_FOREGROUND_COLOR,
                         &FPS_BACKGROUND_COLOR);
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
    x: f32,
    y: f32,
    u: u32,
    v: u32,
}

impl Vertex {
    fn new(x: f32, y: f32, u: u32, v: u32) -> Vertex {
        Vertex {
            x: x,
            y: y,
            u: u,
            v: v,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct CachedGlyph {
    x: f32,
    y: f32,
    glyph_index: u16,
    subpixel: u8,
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

struct RedrawResult {
    events: Option<DrawAtlasProfilingEvents>,
    glyphs_drawn: u32,
}

fn create_program(name: &str) -> GLuint {
    unsafe {
        let (mut vertex_shader_source, mut fragment_shader_source) = (vec![], vec![]);
        File::open(&format!("{}/{}.vs.glsl",
                            EXAMPLE_SHADER_PATH,
                            name)).unwrap().read_to_end(&mut vertex_shader_source).unwrap();
        File::open(&format!("{}/{}.fs.glsl",
                            EXAMPLE_SHADER_PATH,
                            name)).unwrap().read_to_end(&mut fragment_shader_source).unwrap();

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
    let compute_image = rasterizer.device().create_image(Format::RGBA8,
                                                         buffer::Protection::ReadWrite,
                                                         &atlas_size).unwrap();

    rasterizer.queue().submit_clear(&compute_image, &Color::UInt(0, 0, 0, 0), &[]).unwrap();

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

