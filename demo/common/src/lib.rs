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
use crate::ui::{DemoUI, UIAction};
use crate::window::{Event, Keycode, Window};
use clap::{App, Arg};
use image::ColorType;
use jemallocator;
use pathfinder_geometry::basic::point::{Point2DF32, Point2DI32, Point3DF32};
use pathfinder_geometry::basic::rect::RectF32;
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_geometry::basic::transform3d::{Perspective, Transform3DF32};
use pathfinder_geometry::color::ColorU;
use pathfinder_gl::GLDevice;
use pathfinder_gpu::resources::ResourceLoader;
use pathfinder_gpu::{DepthFunc, DepthState, Device, Primitive, RenderState, StencilFunc};
use pathfinder_gpu::{StencilState, UniformData};
use pathfinder_renderer::builder::{RenderOptions, RenderTransform, SceneBuilder};
use pathfinder_renderer::gpu::renderer::Renderer;
use pathfinder_renderer::gpu_data::BuiltScene;
use pathfinder_renderer::post::{DEFRINGING_KERNEL_CORE_GRAPHICS, STEM_DARKENING_FACTORS};
use pathfinder_renderer::scene::Scene;
use pathfinder_renderer::z_buffer::ZBuffer;
use pathfinder_svg::BuiltSVG;
use pathfinder_ui::UIEvent;
use rayon::ThreadPoolBuilder;
use std::f32::consts::FRAC_PI_4;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::mem;
use std::panic;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use usvg::{Options as UsvgOptions, Tree};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

static DEFAULT_SVG_VIRTUAL_PATH: &'static str = "svg/Ghostscript_Tiger.svg";

const MAIN_FRAMEBUFFER_WIDTH: u32 = 1067;
const MAIN_FRAMEBUFFER_HEIGHT: u32 = 800;

const MOUSELOOK_ROTATION_SPEED: f32 = 0.007;
const CAMERA_VELOCITY: f32 = 0.02;

// How much the scene is scaled when a scale gesture is performed.
const CAMERA_SCALE_SPEED_2D: f32 = 6.0;
// How much the scene is scaled when a zoom button is clicked.
const CAMERA_ZOOM_AMOUNT_2D: f32 = 0.1;

const NEAR_CLIP_PLANE: f32 = 0.01;
const FAR_CLIP_PLANE:  f32 = 10.0;

const LIGHT_BG_COLOR:     ColorU = ColorU { r: 248, g: 248, b: 248, a: 255 };
const DARK_BG_COLOR:      ColorU = ColorU { r: 32,  g: 32,  b: 32,  a: 255 };
const GROUND_SOLID_COLOR: ColorU = ColorU { r: 80,  g: 80,  b: 80,  a: 255 };
const GROUND_LINE_COLOR:  ColorU = ColorU { r: 127, g: 127, b: 127, a: 255 };

const APPROX_FONT_SIZE: f32 = 16.0;

const MESSAGE_TIMEOUT_SECS: u64 = 5;

pub const GRIDLINE_COUNT: u8 = 10;

pub mod window;

mod device;
mod ui;

pub struct DemoApp<W> where W: Window {
    pub window: W,
    pub should_exit: bool,

    scale_factor: f32,

    scene_view_box: RectF32,
    scene_is_monochrome: bool,

    camera: Camera,
    frame_counter: u32,
    pending_screenshot_path: Option<PathBuf>,
    mouselook_enabled: bool,
    dirty: bool,
    expire_message_event_id: u32,
    message_epoch: u32,

    ui: DemoUI<GLDevice>,
    scene_thread_proxy: SceneThreadProxy,
    renderer: Renderer<GLDevice>,

    ground_program: GroundProgram<GLDevice>,
    ground_solid_vertex_array: GroundSolidVertexArray<GLDevice>,
    ground_line_vertex_array: GroundLineVertexArray<GLDevice>,
}

impl<W> DemoApp<W> where W: Window {
    pub fn new() -> DemoApp<W> {
        let default_framebuffer_size = Point2DI32::new(MAIN_FRAMEBUFFER_WIDTH as i32,
                                                       MAIN_FRAMEBUFFER_HEIGHT as i32);
        let window = W::new(default_framebuffer_size);
        let expire_message_event_id = window.create_user_event_id();

        let device = GLDevice::new(window.gl_version());
        let resources = window.resource_loader();
        let options = Options::get(resources);

        let (window_size, drawable_size) = (window.size(), window.drawable_size());

        let built_svg = load_scene(resources, &options.input_path);
        let message = get_svg_building_message(&built_svg);
        let scene_view_box = built_svg.scene.view_box;
        let scene_is_monochrome = built_svg.scene.is_monochrome();

        let renderer = Renderer::new(device, resources, drawable_size);
        let scene_thread_proxy = SceneThreadProxy::new(built_svg.scene, options.clone());
        scene_thread_proxy.set_drawable_size(window.drawable_size());

        let camera = if options.three_d {
            Camera::new_3d(scene_view_box)
        } else {
            Camera::new_2d(scene_view_box, drawable_size)
        };

        let ground_program = GroundProgram::new(&renderer.device, resources);
        let ground_solid_vertex_array =
            GroundSolidVertexArray::new(&renderer.device,
                                        &ground_program,
                                        &renderer.quad_vertex_positions_buffer());
        let ground_line_vertex_array = GroundLineVertexArray::new(&renderer.device,
                                                                  &ground_program);

        let mut ui = DemoUI::new(&renderer.device, resources, options);
        let mut message_epoch = 0;
        emit_message::<W>(&mut ui, &mut message_epoch, expire_message_event_id, message);

        DemoApp {
            window,
            should_exit: false,

            scale_factor: drawable_size.x() as f32 / window_size.x() as f32,

            scene_view_box,
            scene_is_monochrome,

            camera,
            frame_counter: 0,
            pending_screenshot_path: None,
            mouselook_enabled: false,
            dirty: true,
            expire_message_event_id,
            message_epoch,

            ui,
            scene_thread_proxy,
            renderer,

            ground_program,
            ground_solid_vertex_array,
            ground_line_vertex_array,
        }
    }

    pub fn run_once(&mut self, events: Vec<Event>) {
        // Update the scene.
        self.build_scene();

        // Handle events.
        let ui_event = self.handle_events(events);

        // Draw the scene.
        let render_msg = self.scene_thread_proxy.receiver.recv().unwrap();
        self.draw_scene(render_msg, ui_event);
    }

    fn build_scene(&mut self) {
        let drawable_size = self.window.drawable_size();

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

    fn handle_events(&mut self, events: Vec<Event>) -> UIEvent {
        let mut ui_event = UIEvent::None;
        self.dirty = false;

        for event in events {
            match event {
                Event::Quit { .. } |
                Event::KeyDown(Keycode::Escape) => {
                    self.should_exit = true;
                    self.dirty = true;
                }
                Event::WindowResized => {
                    self.scene_thread_proxy.set_drawable_size(self.window.drawable_size());
                    self.renderer.set_main_framebuffer_size(self.window.drawable_size());
                    self.dirty = true;
                }
                Event::MouseDown(position) => {
                    ui_event = UIEvent::MouseDown(position.scale(self.scale_factor as i32));
                }
                Event::MouseMoved { relative_position, .. } if self.mouselook_enabled => {
                    if let Camera::ThreeD { ref mut transform, .. } = self.camera {
                        let rotation = relative_position.to_f32().scale(MOUSELOOK_ROTATION_SPEED);
                        transform.yaw += rotation.x();
                        transform.pitch += rotation.y();
                        self.dirty = true;
                    }
                }
                Event::MouseDragged { position, relative_position } => {
                    let absolute_position = position.scale(self.scale_factor as i32);
                    let relative_position = relative_position.scale(self.scale_factor as i32);
                    ui_event = UIEvent::MouseDragged { absolute_position, relative_position };
                    self.dirty = true;
                }
                Event::Zoom(d_dist) => {
                    if let Camera::TwoD(ref mut transform) = self.camera {
                        let position = get_mouse_position(&self.window, self.scale_factor);
                        *transform = transform.post_translate(-position);
                        let scale_delta = 1.0 + d_dist * CAMERA_SCALE_SPEED_2D;
                        *transform = transform.post_scale(Point2DF32::splat(scale_delta));
                        *transform = transform.post_translate(position);
                    }
                }
                Event::KeyDown(Keycode::Alphanumeric(b'w')) => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        let scale_factor = scale_factor_for_view_box(self.scene_view_box);
                        velocity.set_z(-CAMERA_VELOCITY / scale_factor);
                        self.dirty = true;
                    }
                }
                Event::KeyDown(Keycode::Alphanumeric(b's')) => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        let scale_factor = scale_factor_for_view_box(self.scene_view_box);
                        velocity.set_z(CAMERA_VELOCITY / scale_factor);
                        self.dirty = true;
                    }
                }
                Event::KeyDown(Keycode::Alphanumeric(b'a')) => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        let scale_factor = scale_factor_for_view_box(self.scene_view_box);
                        velocity.set_x(-CAMERA_VELOCITY / scale_factor);
                        self.dirty = true;
                    }
                }
                Event::KeyDown(Keycode::Alphanumeric(b'd')) => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        let scale_factor = scale_factor_for_view_box(self.scene_view_box);
                        velocity.set_x(CAMERA_VELOCITY / scale_factor);
                        self.dirty = true;
                    }
                }
                Event::KeyUp(Keycode::Alphanumeric(b'w')) |
                Event::KeyUp(Keycode::Alphanumeric(b's')) => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        velocity.set_z(0.0);
                        self.dirty = true;
                    }
                }
                Event::KeyUp(Keycode::Alphanumeric(b'a')) |
                Event::KeyUp(Keycode::Alphanumeric(b'd')) => {
                    if let Camera::ThreeD { ref mut velocity, .. } = self.camera {
                        velocity.set_x(0.0);
                        self.dirty = true;
                    }
                }
                Event::User { message_type: event_id, message_data: expected_epoch } if
                        event_id == self.expire_message_event_id &&
                        expected_epoch as u32 == self.message_epoch => {
                    self.ui.message = String::new();
                    self.dirty = true;
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

        self.renderer.debug_ui.ui.event = ui_event;
        self.renderer.debug_ui.ui.mouse_position = get_mouse_position(&self.window,
                                                                      self.scale_factor);
        self.ui.show_text_effects = self.scene_is_monochrome;

        let mut ui_action = UIAction::None;
        self.ui.update(&self.renderer.device,
                       &self.window,
                       &mut self.renderer.debug_ui,
                       &mut ui_action);

        ui_event = mem::replace(&mut self.renderer.debug_ui.ui.event, UIEvent::None);
        self.handle_ui_action(&mut ui_action);

        // Switch camera mode (2D/3D) if requested.
        //
        // FIXME(pcwalton): This mess should really be an MVC setup.
        match (&self.camera, self.ui.three_d_enabled) {
            (&Camera::TwoD { .. }, true) => self.camera = Camera::new_3d(self.scene_view_box),
            (&Camera::ThreeD { .. }, false) => {
                let drawable_size = self.window.drawable_size();
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

        self.window.present();
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
                let built_svg = load_scene(self.window.resource_loader(), &Some((*path).clone()));
                self.ui.message = get_svg_building_message(&built_svg);

                self.scene_view_box = built_svg.scene.view_box;
                self.scene_is_monochrome = built_svg.scene.is_monochrome();

                self.scene_thread_proxy.set_drawable_size(self.window.drawable_size());
                let drawable_size = self.window.drawable_size();

                self.camera = if self.ui.three_d_enabled {
                    Camera::new_3d(built_svg.scene.view_box)
                } else {
                    Camera::new_2d(built_svg.scene.view_box, drawable_size)
                };

                self.scene_thread_proxy.load_scene(built_svg.scene);
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
        let drawable_size = self.window.drawable_size();
        let pixels = self.renderer.device.read_pixels_from_default_framebuffer(drawable_size);
        image::save_buffer(screenshot_path,
                           &pixels,
                           drawable_size.x() as u32,
                           drawable_size.y() as u32,
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
    input_path: Option<PathBuf>,
}

impl Options {
    fn get(resources: &dyn ResourceLoader) -> Options {
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

        let input_path = matches.value_of("INPUT").map(PathBuf::from);

        // Set up Rayon.
        let mut thread_pool_builder = ThreadPoolBuilder::new();
        if let Some(jobs) = jobs {
            thread_pool_builder = thread_pool_builder.num_threads(jobs);
        }
        thread_pool_builder.build_global().unwrap();

        Options { jobs, three_d, input_path }
    }
}

fn load_scene(resource_loader: &dyn ResourceLoader, input_path: &Option<PathBuf>) -> BuiltSVG {
    let mut data;
    match *input_path {
        Some(ref input_path) => {
            data = vec![];
            let mut file = match File::open(input_path) {
                Ok(file) => file,
                Err(err) => panic!(),
            };
            file.read_to_end(&mut data).unwrap();
        }
        None => data = resource_loader.slurp(DEFAULT_SVG_VIRTUAL_PATH).unwrap(),
    }

    BuiltSVG::from_tree(Tree::from_data(&data, &UsvgOptions::default()).unwrap())
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

fn center_of_window<W>(window: &W) -> Point2DF32 where W: Window {
    window.drawable_size().to_f32().scale(0.5)
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

fn get_mouse_position<W>(window: &W, scale_factor: f32) -> Point2DF32 where W: Window {
    window.mouse_position().to_f32().scale(scale_factor)
}

fn get_svg_building_message(built_svg: &BuiltSVG) -> String {
    if built_svg.result_flags.is_empty() {
        return String::new();
    }
    format!("Warning: These features in the SVG are unsupported: {}.", built_svg.result_flags)
}

fn emit_message<W>(ui: &mut DemoUI<GLDevice>,
                   message_epoch: &mut u32,
                   expire_message_event_id: u32,
                   message: String)
                   where W: Window {
    if message.is_empty() {
        return;
    }

    ui.message = message;
    let expected_epoch = *message_epoch + 1;
    *message_epoch = expected_epoch;
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(MESSAGE_TIMEOUT_SECS));
        W::push_user_event(expire_message_event_id, expected_epoch);
    });
}
