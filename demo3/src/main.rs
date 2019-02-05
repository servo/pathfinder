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
use jemallocator;
use pathfinder_geometry::basic::point::{Point2DF32, Point3DF32};
use pathfinder_geometry::basic::rect::RectF32;
use pathfinder_geometry::basic::transform3d::{Perspective, Transform3DF32};
use pathfinder_gl::renderer::Renderer;
use pathfinder_renderer::builder::SceneBuilder;
use pathfinder_renderer::gpu_data::BuiltScene;
use pathfinder_renderer::scene::{BuildTransform, Scene};
use pathfinder_renderer::z_buffer::ZBuffer;
use pathfinder_svg::SceneExt;
use rayon::ThreadPoolBuilder;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::video::GLProfile;
use std::f32::consts::FRAC_PI_4;
use std::panic;
use std::path::PathBuf;
use std::process;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use usvg::{Options as UsvgOptions, Tree};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

const MAIN_FRAMEBUFFER_WIDTH: u32 = 1067;
const MAIN_FRAMEBUFFER_HEIGHT: u32 = 800;

const MOUSELOOK_ROTATION_SPEED: f32 = 0.007;
const CAMERA_VELOCITY: f32 = 25.0;

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

    let mut camera_position = Point3DF32::new(500.0, 500.0, 3000.0, 1.0);
    let mut camera_velocity = Point3DF32::new(0.0, 0.0, 0.0, 1.0);
    let (mut camera_yaw, mut camera_pitch) = (0.0, 0.0);

    let window_size = Size2D::new(drawable_width, drawable_height);
    renderer.debug_renderer.set_framebuffer_size(&window_size);

    let base_scene = load_scene(&options, &window_size);
    let scene_thread_proxy = SceneThreadProxy::new(base_scene, options.clone());

    let mut events = vec![];
    let mut first_frame = true;

    while !exit {
        // Update the scene.
        let perspective = if options.run_in_3d {
            let rotation = Transform3DF32::from_rotation(-camera_yaw, -camera_pitch, 0.0);
            camera_position = camera_position + rotation.transform_point(camera_velocity);

            let mut transform =
                Transform3DF32::from_perspective(FRAC_PI_4, 4.0 / 3.0, 0.025, 100.0);

            transform = transform.post_mul(&Transform3DF32::from_scale(1.0 / 800.0,
                                                                       1.0 / 800.0,
                                                                       1.0 / 800.0));
            transform = transform.post_mul(&Transform3DF32::from_rotation(camera_yaw,
                                                                          camera_pitch,
                                                                          0.0));
            transform =
                transform.post_mul(&Transform3DF32::from_translation(-camera_position.x(),
                                                                     -camera_position.y(),
                                                                     -camera_position.z()));

            Some(Perspective::new(&transform, &window_size))
        } else {
            None
        };

        scene_thread_proxy.sender.send(MainToSceneMsg::Build(perspective)).unwrap();

        // Draw the scene.
        if !first_frame {
            if let Ok(SceneToMainMsg::Render {
                built_scene,
                tile_time
            }) = scene_thread_proxy.receiver.recv() {
                unsafe {
                    gl::ClearColor(0.7, 0.7, 0.7, 1.0);
                    gl::Clear(gl::COLOR_BUFFER_BIT);
                    renderer.render_scene(&built_scene);

                    let rendering_time = renderer.shift_timer_query();
                    renderer.debug_renderer.draw(tile_time, rendering_time);
                }
            }
        }

        window.gl_swap_window();
        first_frame = false;

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

struct SceneThreadProxy {
    sender: Sender<MainToSceneMsg>,
    receiver: Receiver<SceneToMainMsg>,
}

impl SceneThreadProxy {
    fn new(scene: Scene, options: Options) -> SceneThreadProxy {
        let (main_to_scene_sender, main_to_scene_receiver) = mpsc::channel();
        let (scene_to_main_sender, scene_to_main_receiver) = mpsc::channel();
        SceneThread::new(scene, scene_to_main_sender, main_to_scene_receiver, options);
        SceneThreadProxy { sender: main_to_scene_sender, receiver: scene_to_main_receiver }
    }
}

struct SceneThread {
    scene: Scene,
    sender: Sender<SceneToMainMsg>,
    receiver: Receiver<MainToSceneMsg>,
    options: Options,
}

impl SceneThread {
    fn new(scene: Scene,
           sender: Sender<SceneToMainMsg>,
           receiver: Receiver<MainToSceneMsg>,
           options: Options) {
        thread::spawn(move || (SceneThread { scene, sender, receiver, options }).run());
    }

    fn run(self) {
        while let Ok(msg) = self.receiver.recv() {
            match msg {
                MainToSceneMsg::Build(perspective) => {
                    let start_time = Instant::now();
                    let built_scene = build_scene(&self.scene, perspective, &self.options);
                    let tile_time = Instant::now() - start_time;
                    self.sender.send(SceneToMainMsg::Render { built_scene, tile_time }).unwrap();
                }
            }
        }
    }
}

enum MainToSceneMsg {
    Build(Option<Perspective>),
}

enum SceneToMainMsg {
    Render { built_scene: BuiltScene, tile_time: Duration }
}

#[derive(Clone)]
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
    scene.view_box =
        RectF32::new(Point2DF32::default(),
                     Point2DF32::new(window_size.width as f32, window_size.height as f32));

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

fn build_scene(scene: &Scene, perspective: Option<Perspective>, options: &Options) -> BuiltScene {
    let z_buffer = ZBuffer::new(scene.view_box);

    let build_transform = match perspective {
        None => BuildTransform::None,
        Some(perspective) => BuildTransform::Perspective(perspective),
    };

    let built_objects = panic::catch_unwind(|| {
         match options.jobs {
            Some(1) => scene.build_objects_sequentially(&build_transform, &z_buffer),
            _ => scene.build_objects(&build_transform, &z_buffer),
        }
    });

    let built_objects = match built_objects {
        Ok(built_objects) => built_objects,
        Err(_) => {
            eprintln!("Scene building crashed! Dumping scene:");
            println!("{:?}", scene);
            process::exit(1);
        }
    };

    let mut built_scene = BuiltScene::new(scene.view_box);
    built_scene.shaders = scene.build_shaders();

    let mut scene_builder = SceneBuilder::new(built_objects, z_buffer, scene.view_box);
    built_scene.solid_tiles = scene_builder.build_solid_tiles();
    while let Some(batch) = scene_builder.build_batch() {
        built_scene.batches.push(batch);
    }
    built_scene
}
