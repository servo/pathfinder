// pathfinder/demo/src/main.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::ui::{DemoUI, UIEvent};
use clap::{App, Arg};
use euclid::Size2D;
use jemallocator;
use pathfinder_geometry::basic::point::{Point2DF32, Point2DI32, Point3DF32};
use pathfinder_geometry::basic::rect::RectF32;
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_geometry::basic::transform3d::{Perspective, Transform3DF32};
use pathfinder_gl::renderer::Renderer;
use pathfinder_renderer::builder::{RenderOptions, RenderTransform, SceneBuilder};
use pathfinder_renderer::gpu_data::BuiltScene;
use pathfinder_renderer::paint::ColorU;
use pathfinder_renderer::post::{DEFRINGING_KERNEL_CORE_GRAPHICS, STEM_DARKENING_FACTORS};
use pathfinder_renderer::scene::Scene;
use pathfinder_renderer::z_buffer::ZBuffer;
use pathfinder_svg::SceneExt;
use rayon::ThreadPoolBuilder;
use sdl2::{EventPump, Sdl, VideoSubsystem};
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::video::{GLContext, GLProfile, Window};
use std::f32::consts::FRAC_PI_4;
use std::panic;
use std::path::{Path, PathBuf};
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

const BACKGROUND_COLOR: ColorU = ColorU { r: 32, g: 32, b: 32, a: 255 };

const APPROX_FONT_SIZE: f32 = 16.0;

const WORLD_SCALE: f32 = 1.0 / 800.0;

mod ui;

fn main() {
    DemoApp::new().run();
}

struct DemoApp {
    window: Window,
    #[allow(dead_code)]
    sdl_context: Sdl,
    #[allow(dead_code)]
    sdl_video: VideoSubsystem,
    sdl_event_pump: EventPump,
    #[allow(dead_code)]
    gl_context: GLContext,

    scale_factor: f32,

    camera: Camera,
    frame_counter: u32,
    events: Vec<Event>,
    exit: bool,
    mouselook_enabled: bool,
    dirty: bool,

    ui: DemoUI,
    scene_thread_proxy: SceneThreadProxy,
    renderer: Renderer,
}

impl DemoApp {
    fn new() -> DemoApp {
        let options = Options::get();

        let sdl_context = sdl2::init().unwrap();
        let sdl_video = sdl_context.video().unwrap();

        let gl_attributes = sdl_video.gl_attr();
        gl_attributes.set_context_profile(GLProfile::Core);
        gl_attributes.set_context_version(3, 3);

        let window =
            sdl_video.window("Pathfinder Demo", MAIN_FRAMEBUFFER_WIDTH, MAIN_FRAMEBUFFER_HEIGHT)
                    .opengl()
                    .resizable()
                    .allow_highdpi()
                    .build()
                    .unwrap();

        let gl_context = window.gl_create_context().unwrap();
        gl::load_with(|name| sdl_video.gl_get_proc_address(name) as *const _);

        let sdl_event_pump = sdl_context.event_pump().unwrap();

        let (window_width, _) = window.size();
        let (drawable_width, drawable_height) = window.drawable_size();
        let drawable_size = Size2D::new(drawable_width, drawable_height);

        let base_scene = load_scene(&options.input_path);
        let scene_thread_proxy = SceneThreadProxy::new(base_scene, options.clone());
        update_drawable_size(&window, &scene_thread_proxy);

        let camera = if options.threed { Camera::three_d() } else { Camera::two_d() };

        DemoApp {
            window,
            sdl_context,
            sdl_video,
            sdl_event_pump,
            gl_context,

            scale_factor: drawable_width as f32 / window_width as f32,

            camera,
            frame_counter: 0,
            events: vec![],
            exit: false,
            mouselook_enabled: false,
            dirty: true,

            ui: DemoUI::new(options),
            scene_thread_proxy,
            renderer: Renderer::new(&drawable_size),
        }
    }

    fn run(&mut self) {
        while !self.exit {
            // Update the scene.
            self.build_scene();

            // Handle events.
            // FIXME(pcwalton): This can cause us to miss UI events if things get backed up...
            let ui_event = self.handle_events();

            // Draw the scene.
            let render_msg = self.scene_thread_proxy.receiver.recv().unwrap();
            self.draw_scene(render_msg, ui_event);
        }
    }

    fn build_scene(&mut self) {
        let (drawable_width, drawable_height) = self.window.drawable_size();
        let drawable_size = Size2D::new(drawable_width, drawable_height);

        let render_transform = match self.camera {
            Camera::ThreeD { ref mut position, velocity, yaw, pitch } => {
                let rotation = Transform3DF32::from_rotation(-yaw, -pitch, 0.0);

                if !velocity.is_zero() {
                    *position = *position + rotation.transform_point(velocity);
                    self.dirty = true;
                }

                let aspect = drawable_size.width as f32 / drawable_size.height as f32;
                let mut transform =
                    Transform3DF32::from_perspective(FRAC_PI_4, aspect, 0.025, 100.0);

                transform = transform.post_mul(&Transform3DF32::from_scale(WORLD_SCALE,
                                                                           WORLD_SCALE,
                                                                           WORLD_SCALE));
                transform = transform.post_mul(&Transform3DF32::from_rotation(yaw, pitch, 0.0));
                let translation = position.scale(-1.0);
                transform = transform.post_mul(&Transform3DF32::from_translation(translation.x(),
                                                                                translation.y(),
                                                                                translation.z()));

                RenderTransform::Perspective(Perspective::new(&transform, &drawable_size))
            }
            Camera::TwoD { ref position } => {
                let mut transform = Transform2DF32::from_rotation(self.ui.rotation());
                transform = transform.post_mul(&Transform2DF32::from_translation(position));
                RenderTransform::Transform2D(transform)
            }
        };

        let count = if self.frame_counter == 0 { 2 } else { 1 };
        for _ in 0..count {
            self.scene_thread_proxy.sender.send(MainToSceneMsg::Build(BuildOptions {
                render_transform: render_transform.clone(),
                stem_darkening_font_size: if self.ui.stem_darkening_effect_enabled {
                    Some(APPROX_FONT_SIZE * self.scale_factor)
                } else {
                    None
                },
            })).unwrap();
        }

        if count == 2 {
            self.dirty = true;
        }
    }

    fn handle_events(&mut self) -> UIEvent {
        let mut ui_event = UIEvent::None;

        if !self.dirty {
            self.events.push(self.sdl_event_pump.wait_event());
        } else {
            self.dirty = false;
        }

        for event in self.sdl_event_pump.poll_iter() {
            self.events.push(event);
        }

        for event in self.events.drain(..) {
            match event {
                Event::Quit { .. } |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    self.exit = true;
                    self.dirty = true;
                }
                Event::Window { win_event: WindowEvent::SizeChanged(..), .. } => {
                    let drawable_size = update_drawable_size(&self.window,
                                                             &self.scene_thread_proxy);
                    self.renderer.set_main_framebuffer_size(&drawable_size);
                    self.dirty = true;
                }
                Event::MouseButtonDown { x, y, .. } => {
                    let point = Point2DI32::new(x, y).scale(self.scale_factor as i32);
                    ui_event = UIEvent::MouseDown(point);
                }
                Event::MouseMotion { xrel, yrel, .. } if self.mouselook_enabled => {
                    if let Camera::ThreeD { ref mut yaw, ref mut pitch, .. } =
                            self.camera {
                        *yaw += xrel as f32 * MOUSELOOK_ROTATION_SPEED;
                        *pitch -= yrel as f32 * MOUSELOOK_ROTATION_SPEED;
                        self.dirty = true;
                    }
                }
                Event::MouseMotion { x, y, xrel, yrel, mousestate, .. } if mousestate.left() => {
                    let absolute_position = Point2DI32::new(x, y).scale(self.scale_factor as i32);
                    let relative_position =
                        Point2DI32::new(xrel, yrel).scale(self.scale_factor as i32);
                    ui_event = UIEvent::MouseDragged { absolute_position, relative_position };
                    self.dirty = true;
                }
                Event::KeyDown { keycode: Some(Keycode::W), .. } => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        velocity.set_z(-CAMERA_VELOCITY);
                        self.dirty = true;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::S), .. } => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        velocity.set_z(CAMERA_VELOCITY);
                        self.dirty = true;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::A), .. } => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        velocity.set_x(-CAMERA_VELOCITY);
                        self.dirty = true;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::D), .. } => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        velocity.set_x(CAMERA_VELOCITY);
                        self.dirty = true;
                    }
                }
                Event::KeyUp { keycode: Some(Keycode::W), .. } |
                Event::KeyUp { keycode: Some(Keycode::S), .. } => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        velocity.set_z(0.0);
                        self.dirty = true;
                    }
                }
                Event::KeyUp { keycode: Some(Keycode::A), .. } |
                Event::KeyUp { keycode: Some(Keycode::D), .. } => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        velocity.set_x(0.0);
                        self.dirty = true;
                    }
                }
                _ => continue,
            }
        }

        ui_event
    }

    fn draw_scene(&mut self, render_msg: SceneToMainMsg, mut ui_event: UIEvent) {
        let SceneToMainMsg::Render { built_scene, tile_time } = render_msg;

        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::ClearColor(BACKGROUND_COLOR.r as f32 / 255.0,
                           BACKGROUND_COLOR.g as f32 / 255.0,
                           BACKGROUND_COLOR.b as f32 / 255.0,
                           BACKGROUND_COLOR.a as f32 / 255.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            if self.ui.gamma_correction_effect_enabled {
                self.renderer.enable_gamma_correction(BACKGROUND_COLOR);
            } else {
                self.renderer.disable_gamma_correction();
            }

            if self.ui.subpixel_aa_effect_enabled {
                self.renderer.enable_subpixel_aa(&DEFRINGING_KERNEL_CORE_GRAPHICS);
            } else {
                self.renderer.disable_subpixel_aa();
            }

            self.renderer.render_scene(&built_scene);

            let rendering_time = self.renderer.shift_timer_query();
            self.renderer.debug_ui.add_sample(tile_time, rendering_time);
            self.renderer.debug_ui.draw();

            if !ui_event.is_none() {
                self.dirty = true;
            }

            self.ui.update(&mut self.renderer.debug_ui, &mut ui_event);

            // Open a new file if requested.
            if let Some(path) = self.ui.file_to_open.take() {
                let scene = load_scene(&path);
                self.scene_thread_proxy.load_scene(scene);
                update_drawable_size(&self.window, &self.scene_thread_proxy);
                self.dirty = true;
            }

            // Switch camera mode (2D/3D) if requested.
            //
            // FIXME(pcwalton): This mess should really be an MVC setup.
            match (&self.camera, self.ui.threed_enabled) {
                (&Camera::TwoD { .. }, true) => self.camera = Camera::three_d(),
                (&Camera::ThreeD { .. }, false) => self.camera = Camera::two_d(),
                _ => {}
            }

            match ui_event {
                UIEvent::MouseDown(_) if self.camera.is_3d() => {
                    // If nothing handled the mouse-down event, toggle mouselook.
                    self.mouselook_enabled = !self.mouselook_enabled;
                }
                UIEvent::MouseDragged { relative_position, .. } => {
                    if let Camera::TwoD { ref mut position } = self.camera {
                        *position = *position + relative_position.to_f32();
                    }
                }
                _ => {}
            }
        }

        self.window.gl_swap_window();
        self.frame_counter += 1;
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

    fn load_scene(&self, scene: Scene) {
        self.sender.send(MainToSceneMsg::LoadScene(scene)).unwrap();
    }

    fn set_drawable_size(&self, drawable_size: &Size2D<u32>) {
        self.sender.send(MainToSceneMsg::SetDrawableSize(*drawable_size)).unwrap();
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

    fn run(mut self) {
        while let Ok(msg) = self.receiver.recv() {
            match msg {
                MainToSceneMsg::LoadScene(scene) => self.scene = scene,
                MainToSceneMsg::SetDrawableSize(size) => {
                    self.scene.view_box =
                        RectF32::new(Point2DF32::default(),
                                     Point2DF32::new(size.width as f32, size.height as f32));
                }
                MainToSceneMsg::Build(build_options) => {
                    let start_time = Instant::now();
                    let built_scene = build_scene(&self.scene, build_options, self.options.jobs);
                    let tile_time = Instant::now() - start_time;
                    self.sender.send(SceneToMainMsg::Render { built_scene, tile_time }).unwrap();
                }
            }
        }
    }
}

enum MainToSceneMsg {
    LoadScene(Scene),
    SetDrawableSize(Size2D<u32>),
    Build(BuildOptions),
}

struct BuildOptions {
    render_transform: RenderTransform,
    stem_darkening_font_size: Option<f32>,
}

enum SceneToMainMsg {
    Render { built_scene: BuiltScene, tile_time: Duration }
}

#[derive(Clone)]
pub struct Options {
    jobs: Option<usize>,
    threed: bool,
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
        let threed = matches.is_present("3d");
        let input_path = PathBuf::from(matches.value_of("INPUT").unwrap());

        // Set up Rayon.
        let mut thread_pool_builder = ThreadPoolBuilder::new();
        if let Some(jobs) = jobs {
            thread_pool_builder = thread_pool_builder.num_threads(jobs);
        }
        thread_pool_builder.build_global().unwrap();

        Options { jobs, threed, input_path }
    }
}

fn load_scene(input_path: &Path) -> Scene {
    let usvg = Tree::from_file(input_path, &UsvgOptions::default()).unwrap();
    let scene = Scene::from_tree(usvg);
    println!("Scene bounds: {:?}", scene.bounds);
    println!("{} objects, {} paints", scene.objects.len(), scene.paints.len());
    scene
}

fn build_scene(scene: &Scene, build_options: BuildOptions, jobs: Option<usize>) -> BuiltScene {
    let z_buffer = ZBuffer::new(scene.view_box);

    let render_options = RenderOptions {
        transform: build_options.render_transform,
        dilation: match build_options.stem_darkening_font_size {
            None => Point2DF32::default(),
            Some(font_size) => {
                let (x, y) = (STEM_DARKENING_FACTORS[0], STEM_DARKENING_FACTORS[1]);
                Point2DF32::new(x, y).scale(font_size)
            }
        },
    };

    let built_objects = panic::catch_unwind(|| {
         match jobs {
            Some(1) => scene.build_objects_sequentially(render_options, &z_buffer),
            _ => scene.build_objects(render_options, &z_buffer),
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

fn update_drawable_size(window: &Window, scene_thread_proxy: &SceneThreadProxy) -> Size2D<u32> {
    let (drawable_width, drawable_height) = window.drawable_size();
    let drawable_size = Size2D::new(drawable_width as u32, drawable_height as u32);
    scene_thread_proxy.set_drawable_size(&drawable_size);
    drawable_size
}

enum Camera {
    TwoD { position: Point2DF32 },
    ThreeD { position: Point3DF32, velocity: Point3DF32, yaw: f32, pitch: f32 },
}

impl Camera {
    fn two_d() -> Camera {
        Camera::TwoD { position: Point2DF32::new(0.0, 0.0) }
    }

    fn three_d() -> Camera {
        Camera::ThreeD {
            position: Point3DF32::new(500.0, 500.0, 3000.0, 1.0),
            velocity: Point3DF32::new(0.0, 0.0, 0.0, 1.0),
            yaw: 0.0,
            pitch: 0.0,
        }
    }

    fn is_3d(&self) -> bool {
        match *self {
            Camera::ThreeD { .. } => true,
            Camera::TwoD { .. } => false,
        }
    }
}
