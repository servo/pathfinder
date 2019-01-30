// pathfinder/demo3/src/main.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[macro_use]
extern crate serde_derive;

use crate::debug_text::DebugRenderer;
use crate::device::{Buffer, BufferTarget, BufferUploadMode, Framebuffer, Program, Texture};
use crate::device::{Uniform, VertexAttr};
use clap::{App, Arg};
use euclid::{Point2D, Rect, Size2D};
use gl::types::{GLfloat, GLint, GLuint};
use jemallocator;
use pathfinder_geometry::point::Point4DF32;
use pathfinder_geometry::transform3d::{Perspective, Transform3DF32};
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
use std::f32::consts::FRAC_PI_4;
use std::time::Instant;
use std::path::PathBuf;
use usvg::{Options as UsvgOptions, Tree};

mod debug_text;
mod device;

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

static QUAD_VERTEX_POSITIONS: [u8; 8] = [0, 0, 1, 0, 1, 1, 0, 1];

// TODO(pcwalton): Replace with `mem::size_of` calls?
const FILL_INSTANCE_SIZE: GLint = 8;
const SOLID_TILE_INSTANCE_SIZE: GLint = 6;
const MASK_TILE_INSTANCE_SIZE: GLint = 8;

const MASK_FRAMEBUFFER_WIDTH: u32 = TILE_WIDTH * 256;
const MASK_FRAMEBUFFER_HEIGHT: u32 = TILE_HEIGHT * 256;

const MAIN_FRAMEBUFFER_WIDTH: u32 = 1067;
const MAIN_FRAMEBUFFER_HEIGHT: u32 = 800;

const FILL_COLORS_TEXTURE_WIDTH: u32 = 256;
const FILL_COLORS_TEXTURE_HEIGHT: u32 = 256;

const MOUSELOOK_ROTATION_SPEED: f32 = 0.01;
const CAMERA_VELOCITY: f32 = 0.03;

fn main() {
    let options = Options::get();

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

    let mut camera_position = Point4DF32::new(1.1, 1.0, 3.0, 1.0);
    let mut camera_velocity = Point4DF32::new(0.0, 0.0, 0.0, 1.0);
    let (mut camera_yaw, mut camera_pitch) = (0.0, 0.0);

    let window_size = Size2D::new(drawable_width, drawable_height);
    renderer.debug_renderer.set_framebuffer_size(&window_size);

    let base_scene = load_scene(&options, &window_size);
    let mut dump_transformed_scene = false;

    let mut events = vec![];

    while !exit {
        let mut scene = base_scene.clone();

        let mut start_time = Instant::now();

        if options.run_in_3d {
            let rotation = Transform3DF32::from_rotation(-camera_yaw, -camera_pitch, 0.0);
            camera_position = camera_position + rotation.transform_point(camera_velocity);

            let mut transform =
                Transform3DF32::from_perspective(FRAC_PI_4, 4.0 / 3.0, 0.0001, 100.0);
            transform = transform.post_mul(&Transform3DF32::from_rotation(camera_yaw,
                                                                          camera_pitch,
                                                                          0.0));
            transform =
                transform.post_mul(&Transform3DF32::from_translation(-camera_position.x(),
                                                                     -camera_position.y(),
                                                                     -camera_position.z()));
            transform =
                transform.post_mul(&Transform3DF32::from_scale(1.0 / 800.0, 1.0 / 800.0, 1.0));

            let perspective = Perspective::new(&transform, &window_size);

            match options.jobs {
                Some(1) => scene.apply_perspective_sequentially(&perspective),
                _ => scene.apply_perspective(&perspective),
            }
        } else {
            scene.prepare();
        }

        let elapsed_prepare_time = Instant::now() - start_time;

        if dump_transformed_scene {
            println!("{:?}", scene);
            dump_transformed_scene = false;
        }

        // Tile the scene.

        start_time = Instant::now();

        let built_scene = build_scene(&scene, &options);

        let elapsed_tile_time = Instant::now() - start_time;

        // Draw the scene.

        unsafe {
            gl::ClearColor(0.7, 0.7, 0.7, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            renderer.render_scene(&built_scene);

            renderer.debug_renderer.draw(elapsed_prepare_time, elapsed_tile_time);
        }

        window.gl_swap_window();

        let mut event_handled = false;
        while !event_handled {
            if camera_velocity.is_zero() {
                events.push(sdl_event_pump.wait_event());
            }
            for event in sdl_event_pump.poll_iter() {
                events.push(event);
            }

            for event in events.drain(..) {
                match event {
                    Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                        exit = true;
                    }
                    Event::MouseMotion { xrel, yrel, .. } => {
                        camera_yaw += xrel as f32 * MOUSELOOK_ROTATION_SPEED;
                        camera_pitch -= yrel as f32 * MOUSELOOK_ROTATION_SPEED;
                    }
                    Event::KeyDown { keycode: Some(Keycode::W), .. } => {
                        camera_velocity.set_z(-CAMERA_VELOCITY)
                    }
                    Event::KeyDown { keycode: Some(Keycode::S), .. } => {
                        camera_velocity.set_z(CAMERA_VELOCITY)
                    }
                    Event::KeyDown { keycode: Some(Keycode::A), .. } => {
                        camera_velocity.set_x(-CAMERA_VELOCITY)
                    }
                    Event::KeyDown { keycode: Some(Keycode::D), .. } => {
                        camera_velocity.set_x(CAMERA_VELOCITY)
                    }
                    Event::KeyDown { keycode: Some(Keycode::T), .. } => {
                        dump_transformed_scene = true;
                    }
                    Event::KeyUp { keycode: Some(Keycode::W), .. } |
                    Event::KeyUp { keycode: Some(Keycode::S), .. } => {
                        camera_velocity.set_z(0.0);
                    }
                    Event::KeyUp { keycode: Some(Keycode::A), .. } |
                    Event::KeyUp { keycode: Some(Keycode::D), .. } => {
                        camera_velocity.set_x(0.0);
                    }
                    _ => continue,
                }

                event_handled = true;
            }

            // FIXME(pcwalton): This is so ugly!
            if !camera_velocity.is_zero() {
                event_handled = true;
            }
        }
    }
}

struct Options {
    jobs: Option<usize>,
    run_in_3d: bool,
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
                Arg::with_name("3d")
                    .short("3")
                    .long("3d")
                    .help("Run in 3D"),
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
        let run_in_3d = matches.is_present("3d");
        let input_path = PathBuf::from(matches.value_of("INPUT").unwrap());

        // Set up Rayon.
        let mut thread_pool_builder = ThreadPoolBuilder::new();
        if let Some(jobs) = jobs {
            thread_pool_builder = thread_pool_builder.num_threads(jobs);
        }
        thread_pool_builder.build_global().unwrap();

        Options { jobs, run_in_3d, input_path }
    }
}

fn load_scene(options: &Options, window_size: &Size2D<u32>) -> Scene {
    // Build scene.
    let usvg = Tree::from_file(&options.input_path, &UsvgOptions::default()).unwrap();

    let mut scene = Scene::from_tree(usvg);
    scene.view_box = Rect::new(Point2D::zero(), window_size.to_f32());

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
    let z_buffer = ZBuffer::new(&scene.view_box);

    let built_objects = match options.jobs {
        Some(1) => scene.build_objects_sequentially(&z_buffer),
        _ => scene.build_objects(&z_buffer),
    };

    let mut built_scene = BuiltScene::new(&scene.view_box);
    built_scene.shaders = scene.build_shaders();

    let mut scene_builder = SceneBuilder::new(built_objects, z_buffer, &scene.view_box);
    built_scene.solid_tiles = scene_builder.build_solid_tiles();
    while let Some(batch) = scene_builder.build_batch() {
        built_scene.batches.push(batch);
    }
    built_scene
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

    debug_renderer: DebugRenderer,

    main_framebuffer_size: Size2D<u32>,
}

impl Renderer {
    fn new(main_framebuffer_size: &Size2D<u32>) -> Renderer {
        let fill_program = FillProgram::new();
        let solid_tile_program = SolidTileProgram::new();
        let mask_tile_program = MaskTileProgram::new();

        let area_lut_texture = Texture::from_png("area-lut");

        let quad_vertex_positions_buffer = Buffer::new();
        quad_vertex_positions_buffer.upload(&QUAD_VERTEX_POSITIONS,
                                            BufferTarget::Vertex,
                                            BufferUploadMode::Static);

        let fill_vertex_array = FillVertexArray::new(&fill_program, &quad_vertex_positions_buffer);
        let mask_tile_vertex_array = MaskTileVertexArray::new(&mask_tile_program,
                                                              &quad_vertex_positions_buffer);
        let solid_tile_vertex_array = SolidTileVertexArray::new(&solid_tile_program,
                                                                &quad_vertex_positions_buffer);

        let mask_framebuffer = Framebuffer::new(&Size2D::new(MASK_FRAMEBUFFER_WIDTH,
                                                             MASK_FRAMEBUFFER_HEIGHT));

        let fill_colors_texture = Texture::new_rgba(&Size2D::new(FILL_COLORS_TEXTURE_WIDTH,
                                                                 FILL_COLORS_TEXTURE_HEIGHT));

        let debug_renderer = DebugRenderer::new(main_framebuffer_size);

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

            debug_renderer,

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
        self.solid_tile_vertex_array
            .vertex_buffer
            .upload(solid_tiles, BufferTarget::Vertex, BufferUploadMode::Dynamic);
    }

    fn upload_batch(&mut self, batch: &Batch) {
        self.fill_vertex_array
            .vertex_buffer
            .upload(&batch.fills, BufferTarget::Vertex, BufferUploadMode::Dynamic);
        self.mask_tile_vertex_array
            .vertex_buffer
            .upload(&batch.mask_tiles, BufferTarget::Vertex, BufferUploadMode::Dynamic);
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

impl Drop for FillVertexArray {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &mut self.gl_vertex_array);
        }
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

impl Drop for MaskTileVertexArray {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &mut self.gl_vertex_array);
        }
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

impl Drop for SolidTileVertexArray {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &mut self.gl_vertex_array);
        }
    }
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
