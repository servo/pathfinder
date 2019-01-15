// pathfinder/demo3/src/main.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use clap::{App, Arg};
use euclid::Size2D;
use gl::types::{GLchar, GLfloat, GLint, GLsizei, GLsizeiptr, GLuint, GLvoid};
use jemallocator;
use pathfinder_geometry::point::Point2DF32;
use pathfinder_geometry::transform::Transform2DF32;
use pathfinder_renderer::builder::SceneBuilder;
use pathfinder_renderer::gpu_data::{Batch, BuiltScene, SolidTileScenePrimitive};
use pathfinder_renderer::paint::ObjectShader;
use pathfinder_renderer::scene::Scene;
use pathfinder_renderer::tiles::{TILE_HEIGHT, TILE_WIDTH};
use pathfinder_renderer::z_buffer::ZBuffer;
use pathfinder_svg::SceneExt;
use rayon::ThreadPoolBuilder;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::video::GLProfile;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::mem;
use std::path::PathBuf;
use std::ptr;
use std::time::{Duration, Instant};
use usvg::{Options as UsvgOptions, Tree};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

static QUAD_VERTEX_POSITIONS: [u8; 8] = [0, 0, 1, 0, 1, 1, 0, 1];

// TODO(pcwalton): Replace with `mem::size_of` calls?
const FILL_INSTANCE_SIZE: GLint = 8;
const SOLID_TILE_INSTANCE_SIZE: GLint = 6;
const MASK_TILE_INSTANCE_SIZE: GLint = 8;

const MASK_FRAMEBUFFER_WIDTH: u32 = TILE_WIDTH * 256;
const MASK_FRAMEBUFFER_HEIGHT: u32 = TILE_HEIGHT * 256;

const MAIN_FRAMEBUFFER_WIDTH: u32 = 800;
const MAIN_FRAMEBUFFER_HEIGHT: u32 = 800;

const FILL_COLORS_TEXTURE_WIDTH: u32 = 256;
const FILL_COLORS_TEXTURE_HEIGHT: u32 = 256;

fn main() {
    let options = Options::get();
    let base_scene = load_scene(&options);

    let sdl_context = sdl2::init().unwrap();
    let sdl_video = sdl_context.video().unwrap();

    let gl_attributes = sdl_video.gl_attr();
    gl_attributes.set_context_profile(GLProfile::Core);
    gl_attributes.set_context_version(3, 3);

    let window =
        sdl_video.window("Pathfinder Demo", MAIN_FRAMEBUFFER_WIDTH, MAIN_FRAMEBUFFER_HEIGHT)
                 .opengl()
                 .allow_highdpi()
                 .build()
                 .unwrap();

    let _gl_context = window.gl_create_context().unwrap();
    gl::load_with(|name| sdl_video.gl_get_proc_address(name) as *const _);

    let mut sdl_event_pump = sdl_context.event_pump().unwrap();
    let mut exit = false;

    let (drawable_width, drawable_height) = window.drawable_size();
    let mut renderer = Renderer::new(&Size2D::new(drawable_width, drawable_height));

    let mut scale = 1.0;

    while !exit {
        let mut scene = base_scene.clone();
        scene.transform(&Transform2DF32::from_scale(&Point2DF32::new(scale, scale)));
        scale -= 0.1;

        let built_scene = build_scene(&scene, &options);

        unsafe {
            gl::ClearColor(1.0, 1.0, 1.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            renderer.render_scene(&built_scene);
        }

        window.gl_swap_window();

        for event in sdl_event_pump.poll_iter() {
            match event {
                Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    exit = true;
                }
                _ => {}
            }
        }
    }
}

struct Options {
    jobs: Option<usize>,
    input_path: PathBuf,
}

impl Options {
    fn get() -> Options {
        let matches = App::new("tile-svg")
            .arg(
                Arg::with_name("jobs")
                    .short("j")
                    .long("jobs")
                    .value_name("THREADS")
                    .takes_value(true)
                    .help("Number of threads to use"),
            )
            .arg(
                Arg::with_name("INPUT")
                    .help("Path to the SVG file to render")
                    .required(true)
                    .index(1),
            )
            .get_matches();
        let jobs: Option<usize> = matches
            .value_of("jobs")
            .map(|string| string.parse().unwrap());
        let input_path = PathBuf::from(matches.value_of("INPUT").unwrap());

        // Set up Rayon.
        let mut thread_pool_builder = ThreadPoolBuilder::new();
        if let Some(jobs) = jobs {
            thread_pool_builder = thread_pool_builder.num_threads(jobs);
        }
        thread_pool_builder.build_global().unwrap();

        Options { jobs, input_path }
    }
}

fn load_scene(options: &Options) -> Scene {
    // Build scene.
    let usvg = Tree::from_file(&options.input_path, &UsvgOptions::default()).unwrap();
    let scene = Scene::from_tree(usvg);

    println!(
        "Scene bounds: {:?} View box: {:?}",
        scene.bounds, scene.view_box
    );
    println!(
        "{} objects, {} paints",
        scene.objects.len(),
        scene.paints.len()
    );

    scene
}

fn build_scene(scene: &Scene, options: &Options) -> BuiltScene {
    let (mut elapsed_object_build_time, mut elapsed_scene_build_time) = (0.0, 0.0);

    let z_buffer = ZBuffer::new(&scene.view_box);

    let start_time = Instant::now();
    let built_objects = match options.jobs {
        Some(1) => scene.build_objects_sequentially(&z_buffer),
        _ => scene.build_objects(&z_buffer),
    };
    elapsed_object_build_time += duration_to_ms(&(Instant::now() - start_time));

    let start_time = Instant::now();
    let mut built_scene = BuiltScene::new(&scene.view_box);
    built_scene.shaders = scene.build_shaders();
    let mut scene_builder = SceneBuilder::new(built_objects, z_buffer, &scene.view_box);
    built_scene.solid_tiles = scene_builder.build_solid_tiles();
    while let Some(batch) = scene_builder.build_batch() {
        built_scene.batches.push(batch);
    }
    elapsed_scene_build_time += duration_to_ms(&(Instant::now() - start_time));

    let total_elapsed_time = elapsed_object_build_time + elapsed_scene_build_time;

    println!(
        "{:.3}ms ({:.3}ms objects, {:.3}ms scene) elapsed",
        total_elapsed_time, elapsed_object_build_time, elapsed_scene_build_time
    );

    println!("{} solid tiles", built_scene.solid_tiles.len());
    for (batch_index, batch) in built_scene.batches.iter().enumerate() {
        println!(
            "Batch {}: {} fills, {} mask tiles",
            batch_index,
            batch.fills.len(),
            batch.mask_tiles.len()
        );
    }

    built_scene
}

fn duration_to_ms(duration: &Duration) -> f64 {
    duration.as_secs() as f64 * 1000.0 + f64::from(duration.subsec_micros()) / 1000.0
}

struct Renderer {
    fill_program: FillProgram,
    solid_tile_program: SolidTileProgram,
    mask_tile_program: MaskTileProgram,
    area_lut_texture: Texture,
    #[allow(dead_code)]
    quad_vertex_positions_buffer: Buffer,
    fill_vertex_array: FillVertexArray,
    mask_tile_vertex_array: MaskTileVertexArray,
    solid_tile_vertex_array: SolidTileVertexArray,
    mask_framebuffer: Framebuffer,
    fill_colors_texture: Texture,

    main_framebuffer_size: Size2D<u32>,
}

impl Renderer {
    fn new(main_framebuffer_size: &Size2D<u32>) -> Renderer {
        let fill_program = FillProgram::new();
        let solid_tile_program = SolidTileProgram::new();
        let mask_tile_program = MaskTileProgram::new();

        let area_lut_texture = Texture::from_png("area-lut");

        let quad_vertex_positions_buffer = Buffer::new();
        quad_vertex_positions_buffer.upload(&QUAD_VERTEX_POSITIONS, BufferUploadMode::Static);

        let fill_vertex_array = FillVertexArray::new(&fill_program, &quad_vertex_positions_buffer);
        let mask_tile_vertex_array = MaskTileVertexArray::new(&mask_tile_program,
                                                              &quad_vertex_positions_buffer);
        let solid_tile_vertex_array = SolidTileVertexArray::new(&solid_tile_program,
                                                                &quad_vertex_positions_buffer);

        let mask_framebuffer = Framebuffer::new(&Size2D::new(MASK_FRAMEBUFFER_WIDTH,
                                                             MASK_FRAMEBUFFER_HEIGHT));

        let fill_colors_texture = Texture::new_rgba(&Size2D::new(FILL_COLORS_TEXTURE_WIDTH,
                                                                 FILL_COLORS_TEXTURE_HEIGHT));

        Renderer {
            fill_program,
            solid_tile_program,
            mask_tile_program,
            area_lut_texture,
            quad_vertex_positions_buffer,
            fill_vertex_array,
            mask_tile_vertex_array,
            solid_tile_vertex_array,
            mask_framebuffer,
            fill_colors_texture,

            main_framebuffer_size: *main_framebuffer_size,
        }
    }

    fn render_scene(&mut self, built_scene: &BuiltScene) {
        self.upload_shaders(&built_scene.shaders);

        self.upload_solid_tiles(&built_scene.solid_tiles);
        self.draw_solid_tiles(&built_scene.solid_tiles);

        for batch in &built_scene.batches {
            self.upload_batch(batch);
            self.draw_batch_fills(batch);
            self.draw_batch_mask_tiles(batch);
        }
    }

    fn upload_shaders(&mut self, shaders: &[ObjectShader]) {
        let size = Size2D::new(FILL_COLORS_TEXTURE_WIDTH, FILL_COLORS_TEXTURE_HEIGHT);
        let mut fill_colors = vec![0; size.width as usize * size.height as usize * 4];
        for (shader_index, shader) in shaders.iter().enumerate() {
            fill_colors[shader_index * 4 + 0] = shader.fill_color.r;
            fill_colors[shader_index * 4 + 1] = shader.fill_color.g;
            fill_colors[shader_index * 4 + 2] = shader.fill_color.b;
            fill_colors[shader_index * 4 + 3] = shader.fill_color.a;
        }
        self.fill_colors_texture.upload_rgba(&size, &fill_colors);
    }

    fn upload_solid_tiles(&mut self, solid_tiles: &[SolidTileScenePrimitive]) {
        self.solid_tile_vertex_array.vertex_buffer.upload(solid_tiles, BufferUploadMode::Dynamic);
    }

    fn upload_batch(&mut self, batch: &Batch) {
        self.fill_vertex_array.vertex_buffer.upload(&batch.fills, BufferUploadMode::Dynamic);
        self.mask_tile_vertex_array.vertex_buffer.upload(&batch.mask_tiles,
                                                         BufferUploadMode::Dynamic);
    }

    fn draw_batch_fills(&mut self, batch: &Batch) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.mask_framebuffer.gl_framebuffer);
            gl::Viewport(0, 0, MASK_FRAMEBUFFER_WIDTH as GLint, MASK_FRAMEBUFFER_HEIGHT as GLint);
            // TODO(pcwalton): Only clear the appropriate portion?
            gl::ClearColor(0.0, 0.0, 0.0, 0.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            gl::BindVertexArray(self.fill_vertex_array.gl_vertex_array);
            gl::UseProgram(self.fill_program.program.gl_program);
            gl::Uniform2f(self.fill_program.framebuffer_size_uniform.location,
                          MASK_FRAMEBUFFER_WIDTH as GLfloat,
                          MASK_FRAMEBUFFER_HEIGHT as GLfloat);
            gl::Uniform2f(self.fill_program.tile_size_uniform.location,
                          TILE_WIDTH as GLfloat,
                          TILE_HEIGHT as GLfloat);
            self.area_lut_texture.bind(0);
            gl::Uniform1i(self.fill_program.area_lut_uniform.location, 0);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::ONE, gl::ONE);
            gl::Enable(gl::BLEND);
            gl::DrawArraysInstanced(gl::TRIANGLE_FAN, 0, 4, batch.fills.len() as GLint);
            gl::Disable(gl::BLEND);
        }
    }

    fn draw_batch_mask_tiles(&mut self, batch: &Batch) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::Viewport(0,
                         0,
                         self.main_framebuffer_size.width as GLint,
                         self.main_framebuffer_size.height as GLint);

            gl::BindVertexArray(self.mask_tile_vertex_array.gl_vertex_array);
            gl::UseProgram(self.mask_tile_program.program.gl_program);
            gl::Uniform2f(self.mask_tile_program.framebuffer_size_uniform.location,
                          self.main_framebuffer_size.width as GLfloat,
                          self.main_framebuffer_size.height as GLfloat);
            gl::Uniform2f(self.mask_tile_program.tile_size_uniform.location,
                          TILE_WIDTH as GLfloat,
                          TILE_HEIGHT as GLfloat);
            self.mask_framebuffer.texture.bind(0);
            gl::Uniform1i(self.mask_tile_program.stencil_texture_uniform.location, 0);
            gl::Uniform2f(self.mask_tile_program.stencil_texture_size_uniform.location,
                          MASK_FRAMEBUFFER_WIDTH as GLfloat,
                          MASK_FRAMEBUFFER_HEIGHT as GLfloat);
            self.fill_colors_texture.bind(1);
            gl::Uniform1i(self.mask_tile_program.fill_colors_texture_uniform.location, 1);
            gl::Uniform2f(self.mask_tile_program.fill_colors_texture_size_uniform.location,
                          FILL_COLORS_TEXTURE_WIDTH as GLfloat,
                          FILL_COLORS_TEXTURE_HEIGHT as GLfloat);
            // FIXME(pcwalton): Fill this in properly!
            gl::Uniform2f(self.mask_tile_program.view_box_origin_uniform.location, 0.0, 0.0);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFuncSeparate(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA, gl::ONE, gl::ONE);
            gl::Enable(gl::BLEND);
            gl::DrawArraysInstanced(gl::TRIANGLE_FAN, 0, 4, batch.mask_tiles.len() as GLint);
            gl::Disable(gl::BLEND);
        }
    }

    fn draw_solid_tiles(&mut self, solid_tiles: &[SolidTileScenePrimitive]) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::Viewport(0,
                         0,
                         self.main_framebuffer_size.width as GLint,
                         self.main_framebuffer_size.height as GLint);

            gl::BindVertexArray(self.solid_tile_vertex_array.gl_vertex_array);
            gl::UseProgram(self.solid_tile_program.program.gl_program);
            gl::Uniform2f(self.solid_tile_program.framebuffer_size_uniform.location,
                          self.main_framebuffer_size.width as GLfloat,
                          self.main_framebuffer_size.height as GLfloat);
            gl::Uniform2f(self.solid_tile_program.tile_size_uniform.location,
                          TILE_WIDTH as GLfloat,
                          TILE_HEIGHT as GLfloat);
            self.fill_colors_texture.bind(0);
            gl::Uniform1i(self.solid_tile_program.fill_colors_texture_uniform.location, 0);
            gl::Uniform2f(self.solid_tile_program.fill_colors_texture_size_uniform.location,
                          FILL_COLORS_TEXTURE_WIDTH as GLfloat,
                          FILL_COLORS_TEXTURE_HEIGHT as GLfloat);
            // FIXME(pcwalton): Fill this in properly!
            gl::Uniform2f(self.solid_tile_program.view_box_origin_uniform.location, 0.0, 0.0);
            gl::Disable(gl::BLEND);
            gl::DrawArraysInstanced(gl::TRIANGLE_FAN, 0, 4, solid_tiles.len() as GLint);
        }
    }
}

struct FillVertexArray {
    gl_vertex_array: GLuint,
    vertex_buffer: Buffer,
}

impl FillVertexArray {
    fn new(fill_program: &FillProgram, quad_vertex_positions_buffer: &Buffer) -> FillVertexArray {
        let vertex_buffer = Buffer::new();
        let mut gl_vertex_array = 0;
        unsafe {
            let tess_coord_attr = VertexAttr::new(&fill_program.program, "TessCoord");
            let from_px_attr = VertexAttr::new(&fill_program.program, "FromPx");
            let to_px_attr = VertexAttr::new(&fill_program.program, "ToPx");
            let from_subpx_attr = VertexAttr::new(&fill_program.program, "FromSubpx");
            let to_subpx_attr = VertexAttr::new(&fill_program.program, "ToSubpx");
            let tile_index_attr = VertexAttr::new(&fill_program.program, "TileIndex");

            gl::GenVertexArrays(1, &mut gl_vertex_array);
            gl::BindVertexArray(gl_vertex_array);
            gl::UseProgram(fill_program.program.gl_program);
            gl::BindBuffer(gl::ARRAY_BUFFER, quad_vertex_positions_buffer.gl_buffer);
            tess_coord_attr.configure_float(2, gl::UNSIGNED_BYTE, false, 0, 0, 0);
            gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer.gl_buffer);
            from_px_attr.configure_int(1, gl::UNSIGNED_BYTE, FILL_INSTANCE_SIZE, 0, 1);
            to_px_attr.configure_int(1, gl::UNSIGNED_BYTE, FILL_INSTANCE_SIZE, 1, 1);
            from_subpx_attr.configure_float(2, gl::UNSIGNED_BYTE, true, FILL_INSTANCE_SIZE, 2, 1);
            to_subpx_attr.configure_float(2, gl::UNSIGNED_BYTE, true, FILL_INSTANCE_SIZE, 4, 1);
            tile_index_attr.configure_int(1, gl::UNSIGNED_SHORT, FILL_INSTANCE_SIZE, 6, 1);
        }

        FillVertexArray { gl_vertex_array, vertex_buffer }
    }
}

struct MaskTileVertexArray {
    gl_vertex_array: GLuint,
    vertex_buffer: Buffer,
}

impl MaskTileVertexArray {
    fn new(mask_tile_program: &MaskTileProgram, quad_vertex_positions_buffer: &Buffer)
           -> MaskTileVertexArray {
        let vertex_buffer = Buffer::new();
        let mut gl_vertex_array = 0;
        unsafe {
            let tess_coord_attr = VertexAttr::new(&mask_tile_program.program, "TessCoord");
            let tile_origin_attr = VertexAttr::new(&mask_tile_program.program, "TileOrigin");
            let backdrop_attr = VertexAttr::new(&mask_tile_program.program, "Backdrop");
            let object_attr = VertexAttr::new(&mask_tile_program.program, "Object");

            gl::GenVertexArrays(1, &mut gl_vertex_array);
            gl::BindVertexArray(gl_vertex_array);
            gl::UseProgram(mask_tile_program.program.gl_program);
            gl::BindBuffer(gl::ARRAY_BUFFER, quad_vertex_positions_buffer.gl_buffer);
            tess_coord_attr.configure_float(2, gl::UNSIGNED_BYTE, false, 0, 0, 0);
            gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer.gl_buffer);
            tile_origin_attr.configure_float(2, gl::SHORT, false, MASK_TILE_INSTANCE_SIZE, 0, 1);
            backdrop_attr.configure_int(1, gl::SHORT, MASK_TILE_INSTANCE_SIZE, 4, 1);
            object_attr.configure_int(2, gl::UNSIGNED_SHORT, MASK_TILE_INSTANCE_SIZE, 6, 1);
        }

        MaskTileVertexArray { gl_vertex_array, vertex_buffer }
    }
}

struct SolidTileVertexArray {
    gl_vertex_array: GLuint,
    vertex_buffer: Buffer,
}

impl SolidTileVertexArray {
    fn new(solid_tile_program: &SolidTileProgram, quad_vertex_positions_buffer: &Buffer)
           -> SolidTileVertexArray {
        let vertex_buffer = Buffer::new();
        let mut gl_vertex_array = 0;
        unsafe {
            let tess_coord_attr = VertexAttr::new(&solid_tile_program.program, "TessCoord");
            let tile_origin_attr = VertexAttr::new(&solid_tile_program.program, "TileOrigin");
            let object_attr = VertexAttr::new(&solid_tile_program.program, "Object");

            gl::GenVertexArrays(1, &mut gl_vertex_array);
            gl::BindVertexArray(gl_vertex_array);
            gl::UseProgram(solid_tile_program.program.gl_program);
            gl::BindBuffer(gl::ARRAY_BUFFER, quad_vertex_positions_buffer.gl_buffer);
            tess_coord_attr.configure_float(2, gl::UNSIGNED_BYTE, false, 0, 0, 0);
            gl::BindBuffer(gl::ARRAY_BUFFER, vertex_buffer.gl_buffer);
            tile_origin_attr.configure_float(2, gl::SHORT, false, SOLID_TILE_INSTANCE_SIZE, 0, 1);
            object_attr.configure_int(1, gl::UNSIGNED_SHORT, SOLID_TILE_INSTANCE_SIZE, 4, 1);
        }

        SolidTileVertexArray { gl_vertex_array, vertex_buffer }
    }
}

struct VertexAttr {
    attr: GLuint,
}

impl VertexAttr {
    fn new(program: &Program, name: &str) -> VertexAttr {
        let name = CString::new(format!("a{}", name)).unwrap();
        let attr = unsafe {
            gl::GetAttribLocation(program.gl_program, name.as_ptr() as *const GLchar) as GLuint
        };
        VertexAttr { attr }
    }

    fn configure_float(&self,
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

    fn configure_int(&self,
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

struct Framebuffer {
    gl_framebuffer: GLuint,
    texture: Texture,
}

impl Framebuffer {
    fn new(size: &Size2D<u32>) -> Framebuffer {
        let texture = Texture::new_r16f(size);
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
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(1, &mut self.gl_framebuffer)
        }
    }
}

struct Buffer {
    gl_buffer: GLuint,
}

impl Buffer {
    fn new() -> Buffer {
        unsafe {
            let mut gl_buffer = 0;
            gl::GenBuffers(1, &mut gl_buffer);
            Buffer { gl_buffer }
        }
    }

    fn upload<T>(&self, data: &[T], mode: BufferUploadMode) {
        let mode = match mode {
            BufferUploadMode::Static => gl::STATIC_DRAW,
            BufferUploadMode::Dynamic => gl::DYNAMIC_DRAW,
        };
        unsafe {
            gl::BindBuffer(gl::ARRAY_BUFFER, self.gl_buffer);
            gl::BufferData(gl::ARRAY_BUFFER,
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

enum BufferUploadMode {
    Static,
    Dynamic,
}

struct FillProgram {
    program: Program,
    framebuffer_size_uniform: Uniform,
    tile_size_uniform: Uniform,
    area_lut_uniform: Uniform,
}

impl FillProgram {
    fn new() -> FillProgram {
        let program = Program::new("fill");
        let framebuffer_size_uniform = Uniform::new(&program, "FramebufferSize");
        let tile_size_uniform = Uniform::new(&program, "TileSize");
        let area_lut_uniform = Uniform::new(&program, "AreaLUT");
        FillProgram { program, framebuffer_size_uniform, tile_size_uniform, area_lut_uniform }
    }
}

struct SolidTileProgram {
    program: Program,
    framebuffer_size_uniform: Uniform,
    tile_size_uniform: Uniform,
    fill_colors_texture_uniform: Uniform,
    fill_colors_texture_size_uniform: Uniform,
    view_box_origin_uniform: Uniform,
}

impl SolidTileProgram {
    fn new() -> SolidTileProgram {
        let program = Program::new("solid_tile");
        let framebuffer_size_uniform = Uniform::new(&program, "FramebufferSize");
        let tile_size_uniform = Uniform::new(&program, "TileSize");
        let fill_colors_texture_uniform = Uniform::new(&program, "FillColorsTexture");
        let fill_colors_texture_size_uniform = Uniform::new(&program, "FillColorsTextureSize");
        let view_box_origin_uniform = Uniform::new(&program, "ViewBoxOrigin");
        SolidTileProgram {
            program,
            framebuffer_size_uniform,
            tile_size_uniform,
            fill_colors_texture_uniform,
            fill_colors_texture_size_uniform,
            view_box_origin_uniform,
        }
    }
}

struct MaskTileProgram {
    program: Program,
    framebuffer_size_uniform: Uniform,
    tile_size_uniform: Uniform,
    stencil_texture_uniform: Uniform,
    stencil_texture_size_uniform: Uniform,
    fill_colors_texture_uniform: Uniform,
    fill_colors_texture_size_uniform: Uniform,
    view_box_origin_uniform: Uniform,
}

impl MaskTileProgram {
    fn new() -> MaskTileProgram {
        let program = Program::new("mask_tile");
        let framebuffer_size_uniform = Uniform::new(&program, "FramebufferSize");
        let tile_size_uniform = Uniform::new(&program, "TileSize");
        let stencil_texture_uniform = Uniform::new(&program, "StencilTexture");
        let stencil_texture_size_uniform = Uniform::new(&program, "StencilTextureSize");
        let fill_colors_texture_uniform = Uniform::new(&program, "FillColorsTexture");
        let fill_colors_texture_size_uniform = Uniform::new(&program, "FillColorsTextureSize");
        let view_box_origin_uniform = Uniform::new(&program, "ViewBoxOrigin");
        MaskTileProgram {
            program,
            framebuffer_size_uniform,
            tile_size_uniform,
            stencil_texture_uniform,
            stencil_texture_size_uniform,
            fill_colors_texture_uniform,
            fill_colors_texture_size_uniform,
            view_box_origin_uniform,
        }
    }
}

#[derive(Debug)]
struct Uniform {
    location: GLint,
}

impl Uniform {
    fn new(program: &Program, name: &str) -> Uniform {
        let name = CString::new(format!("u{}", name)).unwrap();
        let location = unsafe {
            gl::GetUniformLocation(program.gl_program, name.as_ptr() as *const GLchar)
        };
        Uniform { location }
    }
}

struct Program {
    gl_program: GLuint,
    #[allow(dead_code)]
    vertex_shader: Shader,
    #[allow(dead_code)]
    fragment_shader: Shader,
}

impl Program {
    fn new(name: &'static str) -> Program {
        let vertex_shader = Shader::new(name, ShaderKind::Vertex);
        let fragment_shader = Shader::new(name, ShaderKind::Fragment);

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

impl Shader {
    fn new(name: &str, kind: ShaderKind) -> Shader {
        let suffix = match kind { ShaderKind::Vertex => 'v', ShaderKind::Fragment => 'f' };
        // FIXME(pcwalton): Put the shaders somewhere else. Maybe compile them in?
        let path = format!("shaders/{}.{}s.glsl", name, suffix);
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

struct Texture {
    gl_texture: GLuint,
}

impl Texture {
    fn new_r16f(size: &Size2D<u32>) -> Texture {
        let mut texture = Texture { gl_texture: 0 };
        unsafe {
            gl::GenTextures(1, &mut texture.gl_texture);
            texture.bind(0);
            gl::TexImage2D(gl::TEXTURE_2D,
                           0,
                           gl::R16F as GLint,
                           size.width as GLsizei,
                           size.height as GLsizei,
                           0,
                           gl::RED,
                           gl::HALF_FLOAT,
                           ptr::null());
        }

        texture.set_parameters();
        texture
    }

    fn new_rgba(size: &Size2D<u32>) -> Texture {
        let mut texture = Texture { gl_texture: 0 };
        unsafe {
            gl::GenTextures(1, &mut texture.gl_texture);
            texture.bind(0);
            gl::TexImage2D(gl::TEXTURE_2D,
                           0,
                           gl::RGBA as GLint,
                           size.width as GLsizei,
                           size.height as GLsizei,
                           0,
                           gl::RGBA,
                           gl::UNSIGNED_BYTE,
                           ptr::null());
        }

        texture.set_parameters();
        texture
    }

    fn from_png(name: &str) -> Texture {
        let path = format!("textures/{}.png", name);
        let image = image::open(&path).unwrap().to_luma();

        let mut texture = Texture { gl_texture: 0 };
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

    fn bind(&self, unit: u32) {
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0 + unit);
            gl::BindTexture(gl::TEXTURE_2D, self.gl_texture);
        }
    }

    fn upload_rgba(&self, size: &Size2D<u32>, data: &[u8]) {
        assert!(data.len() >= size.width as usize * size.height as usize * 4);
        unsafe {
            self.bind(0);
            gl::TexImage2D(gl::TEXTURE_2D,
                           0,
                           gl::RGBA as GLint,
                           size.width as GLsizei,
                           size.height as GLsizei,
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
