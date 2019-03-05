// pathfinder/demo/common/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A demo app for Pathfinder.

use crate::device::{GroundLineVertexArray, GroundProgram, GroundSolidVertexArray};
use crate::ui::{DemoUI, UIAction, UIEvent};
use clap::{App, Arg};
use image::ColorType;
use jemallocator;
use pathfinder_geometry::basic::point::{Point2DF32, Point2DI32, Point3DF32};
use pathfinder_geometry::basic::rect::RectF32;
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_geometry::basic::transform3d::{Perspective, Transform3DF32};
use pathfinder_gl::device::GLDevice;
use pathfinder_gpu::{DepthFunc, DepthState, Device, Primitive, RenderState, Resources};
use pathfinder_gpu::{StencilFunc, StencilState, UniformData};
use pathfinder_renderer::builder::{RenderOptions, RenderTransform, SceneBuilder};
use pathfinder_renderer::gpu::renderer::Renderer;
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

static DEFAULT_SVG_FILENAME: &'static str = "Ghostscript_Tiger.svg";

const MAIN_FRAMEBUFFER_WIDTH: u32 = 1067;
const MAIN_FRAMEBUFFER_HEIGHT: u32 = 800;

const MOUSELOOK_ROTATION_SPEED: f32 = 0.007;
const CAMERA_VELOCITY: f32 = 1000.0;

// How much the scene is scaled when a scale gesture is performed.
const CAMERA_SCALE_SPEED_2D: f32 = 6.0;
// How much the scene is scaled when a zoom button is clicked.
const CAMERA_ZOOM_AMOUNT_2D: f32 = 0.1;

const NEAR_CLIP_PLANE: f32 = 0.01;
const FAR_CLIP_PLANE:  f32 = 10.0;

const LIGHT_BG_COLOR:     ColorU = ColorU { r: 192, g: 192, b: 192, a: 255 };
const DARK_BG_COLOR:      ColorU = ColorU { r: 32,  g: 32,  b: 32,  a: 255 };
const GROUND_SOLID_COLOR: ColorU = ColorU { r: 80,  g: 80,  b: 80,  a: 255 };
const GROUND_LINE_COLOR:  ColorU = ColorU { r: 127, g: 127, b: 127, a: 255 };

const APPROX_FONT_SIZE: f32 = 16.0;

pub const GRIDLINE_COUNT: u8 = 10;

mod device;
mod ui;

pub struct DemoApp {
    window: Window,
    #[allow(dead_code)]
    sdl_context: Sdl,
    #[allow(dead_code)]
    sdl_video: VideoSubsystem,
    sdl_event_pump: EventPump,
    #[allow(dead_code)]
    gl_context: GLContext,

    scale_factor: f32,

    scene_view_box: RectF32,

    camera: Camera,
    frame_counter: u32,
    events: Vec<Event>,
    pending_screenshot_path: Option<PathBuf>,
    exit: bool,
    mouselook_enabled: bool,
    dirty: bool,

    ui: DemoUI<GLDevice>,
    scene_thread_proxy: SceneThreadProxy,
    renderer: Renderer<GLDevice>,

    ground_program: GroundProgram<GLDevice>,
    ground_solid_vertex_array: GroundSolidVertexArray<GLDevice>,
    ground_line_vertex_array: GroundLineVertexArray<GLDevice>,
}

impl DemoApp {
    pub fn new() -> DemoApp {
        let sdl_context = sdl2::init().unwrap();
        let sdl_video = sdl_context.video().unwrap();

        let gl_attributes = sdl_video.gl_attr();
        gl_attributes.set_context_profile(GLProfile::Core);
        gl_attributes.set_context_version(3, 3);
        gl_attributes.set_depth_size(24);
        gl_attributes.set_stencil_size(8);

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

        let device = GLDevice::new();
        let resources = Resources::locate();
        let options = Options::get(&resources);

        let (window_width, _) = window.size();
        let (drawable_width, drawable_height) = window.drawable_size();
        let drawable_size = Point2DI32::new(drawable_width as i32, drawable_height as i32);

        let base_scene = load_scene(&options.input_path);
        let scene_view_box = base_scene.view_box;
        let renderer = Renderer::new(device, &resources, drawable_size);
        let scene_thread_proxy = SceneThreadProxy::new(base_scene, options.clone());
        update_drawable_size(&window, &scene_thread_proxy);

        let camera = if options.three_d {
            Camera::new_3d(scene_view_box)
        } else {
            Camera::new_2d(scene_view_box, drawable_size)
        };

        let ground_program = GroundProgram::new(&renderer.device, &resources);
        let ground_solid_vertex_array =
            GroundSolidVertexArray::new(&renderer.device,
                                        &ground_program,
                                        &renderer.quad_vertex_positions_buffer());
        let ground_line_vertex_array = GroundLineVertexArray::new(&renderer.device,
                                                                  &ground_program);

        let ui = DemoUI::new(&renderer.device, &resources, options);

        DemoApp {
            window,
            sdl_context,
            sdl_video,
            sdl_event_pump,
            gl_context,

            scale_factor: drawable_width as f32 / window_width as f32,

            scene_view_box,

            camera,
            frame_counter: 0,
            pending_screenshot_path: None,
            events: vec![],
            exit: false,
            mouselook_enabled: false,
            dirty: true,

            ui,
            scene_thread_proxy,
            renderer,

            ground_program,
            ground_solid_vertex_array,
            ground_line_vertex_array,
        }
    }

    pub fn run(&mut self) {
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
        let drawable_size = Point2DI32::new(drawable_width as i32, drawable_height as i32);

        let render_transform = match self.camera {
            Camera::ThreeD { ref mut transform, ref mut velocity } => {
                if transform.offset(*velocity) {
                    self.dirty = true;
                }
                let perspective = transform.to_perspective(drawable_size);
                RenderTransform::Perspective(perspective)
            }
            Camera::TwoD(transform) => RenderTransform::Transform2D(transform),
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
                    update_drawable_size(&self.window, &self.scene_thread_proxy);
                    let drawable_size = current_drawable_size(&self.window);
                    self.renderer.set_main_framebuffer_size(drawable_size);
                    self.dirty = true;
                }
                Event::MouseButtonDown { x, y, .. } => {
                    let point = Point2DI32::new(x, y).scale(self.scale_factor as i32);
                    ui_event = UIEvent::MouseDown(point);
                }
                Event::MouseMotion { xrel, yrel, .. } if self.mouselook_enabled => {
                    if let Camera::ThreeD { ref mut transform, .. } = self.camera {
                        transform.yaw += xrel as f32 * MOUSELOOK_ROTATION_SPEED;
                        transform.pitch += yrel as f32 * MOUSELOOK_ROTATION_SPEED;
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
                Event::MultiGesture { d_dist, .. } => {
                    if let Camera::TwoD(ref mut transform) = self.camera {
                        let mouse_state = self.sdl_event_pump.mouse_state();
                        let position = Point2DI32::new(mouse_state.x(), mouse_state.y());
                        let position = position.to_f32().scale(self.scale_factor);
                        *transform = transform.post_translate(-position);
                        let scale_delta = 1.0 + d_dist * CAMERA_SCALE_SPEED_2D;
                        *transform = transform.post_scale(Point2DF32::splat(scale_delta));
                        *transform = transform.post_translate(position);
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::W), .. } => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        let scale_factor = scale_factor_for_view_box(self.scene_view_box);
                        velocity.set_z(-CAMERA_VELOCITY * scale_factor);
                        self.dirty = true;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::S), .. } => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        let scale_factor = scale_factor_for_view_box(self.scene_view_box);
                        velocity.set_z(CAMERA_VELOCITY * scale_factor);
                        self.dirty = true;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::A), .. } => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        let scale_factor = scale_factor_for_view_box(self.scene_view_box);
                        velocity.set_x(-CAMERA_VELOCITY * scale_factor);
                        self.dirty = true;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::D), .. } => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        let scale_factor = scale_factor_for_view_box(self.scene_view_box);
                        velocity.set_x(CAMERA_VELOCITY * scale_factor);
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
        let SceneToMainMsg::Render {
            built_scene,
            transform: render_transform,
            tile_time,
        } = render_msg;

        self.renderer.device.clear(Some(self.background_color().to_f32().0), Some(1.0), Some(0));
        self.draw_environment(&render_transform);
        self.render_vector_scene(&built_scene);

        if self.pending_screenshot_path.is_some() {
            self.take_screenshot();
        }

        let rendering_time = self.renderer.shift_timer_query();
        let stats = built_scene.stats();
        self.renderer.debug_ui.add_sample(stats, tile_time, rendering_time);
        self.renderer.debug_ui.draw(&self.renderer.device);

        if !ui_event.is_none() {
            self.dirty = true;
        }

        let mut ui_action = UIAction::None;
        self.ui.update(&self.renderer.device,
                       &mut self.renderer.debug_ui,
                       &mut ui_event,
                       &mut ui_action);
        self.handle_ui_action(&mut ui_action);

        // Switch camera mode (2D/3D) if requested.
        //
        // FIXME(pcwalton): This mess should really be an MVC setup.
        match (&self.camera, self.ui.three_d_enabled) {
            (&Camera::TwoD { .. }, true) => self.camera = Camera::new_3d(self.scene_view_box),
            (&Camera::ThreeD { .. }, false) => {
                let drawable_size = current_drawable_size(&self.window);
                self.camera = Camera::new_2d(self.scene_view_box, drawable_size);
            }
            _ => {}
        }

        match ui_event {
            UIEvent::MouseDown(_) if self.camera.is_3d() => {
                // If nothing handled the mouse-down event, toggle mouselook.
                self.mouselook_enabled = !self.mouselook_enabled;
            }
            UIEvent::MouseDragged { relative_position, .. } => {
                if let Camera::TwoD(ref mut transform) = self.camera {
                    *transform = transform.post_translate(relative_position.to_f32());
                }
            }
            _ => {}
        }

        self.window.gl_swap_window();
        self.frame_counter += 1;
    }

    fn draw_environment(&self, render_transform: &RenderTransform) {
        let perspective = match *render_transform {
            RenderTransform::Transform2D(..) => return,
            RenderTransform::Perspective(perspective) => perspective,
        };

        let ground_scale = self.scene_view_box.max_x() * 2.0;

        let mut base_transform = perspective.transform;
        base_transform = base_transform.post_mul(&Transform3DF32::from_translation(
            -0.5 * self.scene_view_box.max_x(),
            self.scene_view_box.max_y(),
            -0.5 * ground_scale));

        // Draw gridlines. Use the stencil buffer to avoid Z-fighting.
        let mut transform = base_transform;
        let gridline_scale = ground_scale / GRIDLINE_COUNT as f32;
        transform =
            transform.post_mul(&Transform3DF32::from_scale(gridline_scale, 1.0, gridline_scale));
        let device = &self.renderer.device;
        device.bind_vertex_array(&self.ground_line_vertex_array.vertex_array);
        device.use_program(&self.ground_program.program);
        device.set_uniform(&self.ground_program.transform_uniform, UniformData::Mat4([
            transform.c0,
            transform.c1,
            transform.c2,
            transform.c3,
        ]));
        device.set_uniform(&self.ground_program.color_uniform,
                           UniformData::Vec4(GROUND_LINE_COLOR.to_f32().0));
        device.draw_arrays(Primitive::Lines, (GRIDLINE_COUNT as u32 + 1) * 4, &RenderState {
            depth: Some(DepthState { func: DepthFunc::Always, write: true }),
            stencil: Some(StencilState {
                func: StencilFunc::Always,
                reference: 2,
                mask: 2,
                write: true,
            }),
            ..RenderState::default()
        });

        // Fill ground.
        let mut transform = base_transform;
        transform =
            transform.post_mul(&Transform3DF32::from_scale(ground_scale, 1.0, ground_scale));
        device.bind_vertex_array(&self.ground_solid_vertex_array.vertex_array);
        device.use_program(&self.ground_program.program);
        device.set_uniform(&self.ground_program.transform_uniform, UniformData::Mat4([
            transform.c0,
            transform.c1,
            transform.c2,
            transform.c3,
        ]));
        device.set_uniform(&self.ground_program.color_uniform,
                           UniformData::Vec4(GROUND_SOLID_COLOR.to_f32().0));
        device.draw_arrays(Primitive::TriangleFan, 4, &RenderState {
            depth: Some(DepthState { func: DepthFunc::Less, write: true }),
            stencil: Some(StencilState {
                func: StencilFunc::NotEqual,
                reference: 2,
                mask: 2,
                write: false,
            }),
            ..RenderState::default()
        });
    }

    fn render_vector_scene(&mut self, built_scene: &BuiltScene) {
        if self.ui.gamma_correction_effect_enabled {
            self.renderer.enable_gamma_correction(self.background_color());
        } else {
            self.renderer.disable_gamma_correction();
        }

        if self.ui.subpixel_aa_effect_enabled {
            self.renderer.enable_subpixel_aa(&DEFRINGING_KERNEL_CORE_GRAPHICS);
        } else {
            self.renderer.disable_subpixel_aa();
        }

        if self.ui.three_d_enabled {
            self.renderer.enable_depth();
        } else {
            self.renderer.disable_depth();
        }

        self.renderer.render_scene(&built_scene);
    }

    fn handle_ui_action(&mut self, ui_action: &mut UIAction) {
        match ui_action {
            UIAction::None => {}

            UIAction::OpenFile(ref path) => {
                let scene = load_scene(&path);
                self.scene_view_box = scene.view_box;

                update_drawable_size(&self.window, &self.scene_thread_proxy);
                let drawable_size = current_drawable_size(&self.window);

                self.camera = if self.ui.three_d_enabled {
                    Camera::new_3d(scene.view_box)
                } else {
                    Camera::new_2d(scene.view_box, drawable_size)
                };

                self.scene_thread_proxy.load_scene(scene);
                self.dirty = true;
            }

            UIAction::TakeScreenshot(ref path) => {
                self.pending_screenshot_path = Some((*path).clone());
                self.dirty = true;
            }

            UIAction::ZoomIn => {
                if let Camera::TwoD(ref mut transform) = self.camera {
                    let scale = Point2DF32::splat(1.0 + CAMERA_ZOOM_AMOUNT_2D);
                    let center = center_of_window(&self.window);
                    *transform = transform.post_translate(-center)
                                          .post_scale(scale)
                                          .post_translate(center);
                    self.dirty = true;
                }
            }
            UIAction::ZoomOut => {
                if let Camera::TwoD(ref mut transform) = self.camera {
                    let scale = Point2DF32::splat(1.0 - CAMERA_ZOOM_AMOUNT_2D);
                    let center = center_of_window(&self.window);
                    *transform = transform.post_translate(-center)
                                          .post_scale(scale)
                                          .post_translate(center);
                    self.dirty = true;
                }
            }
            UIAction::Rotate(theta) => {
                if let Camera::TwoD(ref mut transform) = self.camera {
                    let old_rotation = transform.rotation();
                    let center = center_of_window(&self.window);
                    *transform = transform.post_translate(-center)
                                          .post_rotate(*theta - old_rotation)
                                          .post_translate(center);
                }
            }
        }
    }

    fn take_screenshot(&mut self) {
        let screenshot_path = self.pending_screenshot_path.take().unwrap();
        let (drawable_width, drawable_height) = self.window.drawable_size();
        let drawable_size = Point2DI32::new(drawable_width as i32, drawable_height as i32);
        let pixels = self.renderer.device.read_pixels_from_default_framebuffer(drawable_size);
        image::save_buffer(screenshot_path,
                           &pixels,
                           drawable_width,
                           drawable_height,
                           ColorType::RGBA(8)).unwrap();
    }

    fn background_color(&self) -> ColorU {
        if self.ui.dark_background_enabled { DARK_BG_COLOR } else { LIGHT_BG_COLOR }
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

    fn set_drawable_size(&self, drawable_size: Point2DI32) {
        self.sender.send(MainToSceneMsg::SetDrawableSize(drawable_size)).unwrap();
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
                    self.scene.view_box = RectF32::new(Point2DF32::default(), size.to_f32());
                }
                MainToSceneMsg::Build(build_options) => {
                    let render_transform = build_options.render_transform.clone();
                    let start_time = Instant::now();
                    let built_scene = build_scene(&self.scene, build_options, self.options.jobs);
                    let tile_time = Instant::now() - start_time;
                    self.sender.send(SceneToMainMsg::Render {
                        built_scene,
                        transform: render_transform,
                        tile_time,
                    }).unwrap();
                }
            }
        }
    }
}

enum MainToSceneMsg {
    LoadScene(Scene),
    SetDrawableSize(Point2DI32),
    Build(BuildOptions),
}

struct BuildOptions {
    render_transform: RenderTransform,
    stem_darkening_font_size: Option<f32>,
}

enum SceneToMainMsg {
    Render { built_scene: BuiltScene, transform: RenderTransform, tile_time: Duration }
}

#[derive(Clone)]
pub struct Options {
    jobs: Option<usize>,
    three_d: bool,
    input_path: PathBuf,
}

impl Options {
    fn get(resources: &Resources) -> Options {
        let matches = App::new("tile-svg")
            .arg(
                Arg::with_name("jobs")
                    .short("j")
                    .long("jobs")
                    .value_name("THREADS")
                    .takes_value(true)
                    .help("Number of threads to use"),
            )
            .arg(Arg::with_name("3d").short("3").long("3d").help("Run in 3D"))
            .arg(Arg::with_name("INPUT").help("Path to the SVG file to render").index(1))
            .get_matches();

        let jobs: Option<usize> = matches
            .value_of("jobs")
            .map(|string| string.parse().unwrap());
        let three_d = matches.is_present("3d");

        let input_path = match matches.value_of("INPUT") {
            Some(path) => PathBuf::from(path),
            None => {
                let mut path = resources.resources_directory.clone();
                path.push("svg");
                path.push(DEFAULT_SVG_FILENAME);
                path
            }
        };

        // Set up Rayon.
        let mut thread_pool_builder = ThreadPoolBuilder::new();
        if let Some(jobs) = jobs {
            thread_pool_builder = thread_pool_builder.num_threads(jobs);
        }
        thread_pool_builder.build_global().unwrap();

        Options { jobs, three_d, input_path }
    }
}

fn load_scene(input_path: &Path) -> Scene {
    Scene::from_tree(Tree::from_file(input_path, &UsvgOptions::default()).unwrap())
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

    let built_options = render_options.prepare(scene.bounds);
    let quad = built_options.quad();

    let built_objects = panic::catch_unwind(|| {
         match jobs {
            Some(1) => scene.build_objects_sequentially(built_options, &z_buffer),
            _ => scene.build_objects(built_options, &z_buffer),
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

    let mut built_scene = BuiltScene::new(scene.view_box, &quad, scene.objects.len() as u32);
    built_scene.shaders = scene.build_shaders();

    let mut scene_builder = SceneBuilder::new(built_objects, z_buffer, scene.view_box);
    built_scene.solid_tiles = scene_builder.build_solid_tiles();
    while let Some(batch) = scene_builder.build_batch() {
        built_scene.batches.push(batch);
    }

    built_scene
}

fn current_drawable_size(window: &Window) -> Point2DI32 {
    let (drawable_width, drawable_height) = window.drawable_size();
    Point2DI32::new(drawable_width as i32, drawable_height as i32)
}

fn update_drawable_size(window: &Window, scene_thread_proxy: &SceneThreadProxy) {
    scene_thread_proxy.set_drawable_size(current_drawable_size(window));
}

fn center_of_window(window: &Window) -> Point2DF32 {
    let (drawable_width, drawable_height) = window.drawable_size();
    Point2DI32::new(drawable_width as i32, drawable_height as i32).to_f32().scale(0.5)
}

enum Camera {
    TwoD(Transform2DF32),
    ThreeD { transform: CameraTransform3D, velocity: Point3DF32 },
}

impl Camera {
    fn new_2d(view_box: RectF32, drawable_size: Point2DI32) -> Camera {
        let scale = i32::min(drawable_size.x(), drawable_size.y()) as f32 *
            scale_factor_for_view_box(view_box);
        let origin = drawable_size.to_f32().scale(0.5) - view_box.size().scale(scale * 0.5);
        Camera::TwoD(Transform2DF32::from_scale(&Point2DF32::splat(scale)).post_translate(origin))
    }

    fn new_3d(view_box: RectF32) -> Camera {
        Camera::ThreeD {
            transform: CameraTransform3D::new(view_box),
            velocity: Point3DF32::default(),
        }
    }

    fn is_3d(&self) -> bool {
        match *self { Camera::ThreeD { .. } => true, Camera::TwoD { .. } => false }
    }
}

#[derive(Clone, Copy)]
struct CameraTransform3D {
    position: Point3DF32,
    yaw: f32,
    pitch: f32,
    scale: f32,
}

impl CameraTransform3D {
    fn new(view_box: RectF32) -> CameraTransform3D {
        let scale = scale_factor_for_view_box(view_box);
        CameraTransform3D {
            position: Point3DF32::new(0.5 * view_box.max_x(),
                                      -0.5 * view_box.max_y(),
                                      1.5 / scale,
                                      1.0),
            yaw: 0.0,
            pitch: 0.0,
            scale,
        }
    }

    fn offset(&mut self, vector: Point3DF32) -> bool {
        let update = !vector.is_zero();
        if update {
            let rotation = Transform3DF32::from_rotation(-self.yaw, -self.pitch, 0.0);
            self.position = self.position + rotation.transform_point(vector);
        }
        update
    }

    fn to_perspective(&self, drawable_size: Point2DI32) -> Perspective {
        let aspect = drawable_size.x() as f32 / drawable_size.y() as f32;
        let mut transform =
            Transform3DF32::from_perspective(FRAC_PI_4, aspect, NEAR_CLIP_PLANE, FAR_CLIP_PLANE);

        transform = transform.post_mul(&Transform3DF32::from_rotation(self.yaw, self.pitch, 0.0));
        transform = transform.post_mul(&Transform3DF32::from_uniform_scale(2.0 * self.scale));
        transform = transform.post_mul(&Transform3DF32::from_translation(-self.position.x(),
                                                                         -self.position.y(),
                                                                         -self.position.z()));

        // Flip Y.
        transform = transform.post_mul(&Transform3DF32::from_scale(1.0, -1.0, 1.0));

        Perspective::new(&transform, drawable_size)
    }
}

fn scale_factor_for_view_box(view_box: RectF32) -> f32 {
    1.0 / f32::min(view_box.size().x(), view_box.size().y())
}
