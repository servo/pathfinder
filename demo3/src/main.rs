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
use pathfinder_geometry::basic::point::{Point2DF32, Point2DI32, Point3DF32};
use pathfinder_geometry::basic::rect::{RectF32, RectI32};
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_geometry::basic::transform3d::{Perspective, Transform3DF32};
use pathfinder_gl::debug::{BUTTON_HEIGHT, BUTTON_TEXT_OFFSET, BUTTON_WIDTH, DebugUI, PADDING};
use pathfinder_gl::debug::{TEXT_COLOR, WINDOW_COLOR};
use pathfinder_gl::device::Texture;
use pathfinder_gl::renderer::Renderer;
use pathfinder_renderer::builder::{RenderOptions, RenderTransform, SceneBuilder};
use pathfinder_renderer::gpu_data::BuiltScene;
use pathfinder_renderer::scene::Scene;
use pathfinder_renderer::z_buffer::ZBuffer;
use pathfinder_svg::SceneExt;
use rayon::ThreadPoolBuilder;
use sdl2::event::{Event, WindowEvent};
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

const BACKGROUND_COLOR: f32 = 0.22;

const EFFECTS_WINDOW_WIDTH: i32 = 550;
const EFFECTS_WINDOW_HEIGHT: i32 = BUTTON_HEIGHT * 3 + PADDING * 4;

const SWITCH_SIZE: i32 = SWITCH_HALF_SIZE * 2 + 1;
const SWITCH_HALF_SIZE: i32 = 96;

static EFFECTS_PNG_NAME: &'static str = "demo-effects";
static OPEN_PNG_NAME: &'static str = "demo-open";

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
                 .resizable()
                 .allow_highdpi()
                 .build()
                 .unwrap();

    let _gl_context = window.gl_create_context().unwrap();
    gl::load_with(|name| sdl_video.gl_get_proc_address(name) as *const _);

    let mut sdl_event_pump = sdl_context.event_pump().unwrap();
    let mut exit = false;

    let (window_width, _) = window.size();
    let (drawable_width, drawable_height) = window.drawable_size();
    let scale_factor = drawable_width / window_width;
    let mut drawable_size = Size2D::new(drawable_width, drawable_height);
    let mut renderer = Renderer::new(&drawable_size);

    let mut camera_position = Point3DF32::new(500.0, 500.0, 3000.0, 1.0);
    let mut camera_velocity = Point3DF32::new(0.0, 0.0, 0.0, 1.0);
    let (mut camera_yaw, mut camera_pitch) = (0.0, 0.0);

    let base_scene = load_scene(&options);
    let scene_thread_proxy = SceneThreadProxy::new(base_scene, options.clone());
    scene_thread_proxy.set_drawable_size(&drawable_size);

    let mut demo_ui = DemoUI::new();

    let mut events = vec![];
    let mut first_frame = true;
    let mut mouselook_enabled = false;

    while !exit {
        // Update the scene.
        let perspective = if options.run_in_3d {
            let rotation = Transform3DF32::from_rotation(-camera_yaw, -camera_pitch, 0.0);
            camera_position = camera_position + rotation.transform_point(camera_velocity);

            let aspect = drawable_size.width as f32 / drawable_size.height as f32;
            let mut transform = Transform3DF32::from_perspective(FRAC_PI_4, aspect, 0.025, 100.0);

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

            Some(Perspective::new(&transform, &drawable_size))
        } else {
            None
        };

        scene_thread_proxy.sender.send(MainToSceneMsg::Build(perspective)).unwrap();

        let mut event_handled = false;

        // FIXME(pcwalton): This can cause us to miss UI events if things get backed up...
        let mut ui_event = UIEvent::None;

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
                    Event::Window { win_event: WindowEvent::SizeChanged(..), .. } => {
                        let (drawable_width, drawable_height) = window.drawable_size();
                        drawable_size = Size2D::new(drawable_width as u32, drawable_height as u32);
                        scene_thread_proxy.set_drawable_size(&drawable_size);
                        renderer.set_main_framebuffer_size(&drawable_size);
                    }
                    Event::MouseButtonDown { x, y, .. } => {
                        let point = Point2DI32::new(x, y).scale(scale_factor as i32);
                        ui_event = UIEvent::MouseDown(point);
                    }
                    Event::MouseMotion { xrel, yrel, .. } if mouselook_enabled => {
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

        // Draw the scene.
        if !first_frame {
            if let Ok(SceneToMainMsg::Render {
                built_scene,
                tile_time
            }) = scene_thread_proxy.receiver.recv() {
                unsafe {
                    gl::ClearColor(BACKGROUND_COLOR, BACKGROUND_COLOR, BACKGROUND_COLOR, 1.0);
                    gl::Clear(gl::COLOR_BUFFER_BIT);
                    renderer.render_scene(&built_scene);

                    let rendering_time = renderer.shift_timer_query();
                    renderer.debug_ui.add_sample(tile_time, rendering_time);
                    renderer.debug_ui.draw();

                    demo_ui.update(&mut renderer.debug_ui, &mut ui_event);

                    // If nothing handled the mouse-down event, toggle mouselook.
                    if let UIEvent::MouseDown(_) = ui_event {
                        mouselook_enabled = !mouselook_enabled;
                    }
                }
            }
        }

        window.gl_swap_window();
        first_frame = false;
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
                MainToSceneMsg::SetDrawableSize(size) => {
                    self.scene.view_box =
                        RectF32::new(Point2DF32::default(),
                                     Point2DF32::new(size.width as f32, size.height as f32));
                }
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
    SetDrawableSize(Size2D<u32>),
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

fn load_scene(options: &Options) -> Scene {
    let usvg = Tree::from_file(&options.input_path, &UsvgOptions::default()).unwrap();
    let scene = Scene::from_tree(usvg);
    println!("Scene bounds: {:?}", scene.bounds);
    println!("{} objects, {} paints", scene.objects.len(), scene.paints.len());
    scene
}

fn build_scene(scene: &Scene, perspective: Option<Perspective>, options: &Options) -> BuiltScene {
    let z_buffer = ZBuffer::new(scene.view_box);

    let render_options = RenderOptions {
        transform: match perspective {
            None => RenderTransform::Transform2D(Transform2DF32::default()),
            Some(perspective) => RenderTransform::Perspective(perspective),
        },
        dilation: Point2DF32::default(),
    };

    let built_objects = panic::catch_unwind(|| {
         match options.jobs {
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

struct DemoUI {
    effects_texture: Texture,
    open_texture: Texture,

    threed_enabled: bool,
    effects_window_visible: bool,
    gamma_correction_effect_enabled: bool,
    stem_darkening_effect_enabled: bool,
    subpixel_aa_effect_enabled: bool,
}

impl DemoUI {
    fn new() -> DemoUI {
        let effects_texture = Texture::from_png(EFFECTS_PNG_NAME);
        let open_texture = Texture::from_png(OPEN_PNG_NAME);

        DemoUI {
            effects_texture,
            open_texture,
            threed_enabled: true,
            effects_window_visible: false,
            gamma_correction_effect_enabled: false,
            stem_darkening_effect_enabled: false,
            subpixel_aa_effect_enabled: false,
        }
    }

    fn update(&mut self, debug_ui: &mut DebugUI, event: &mut UIEvent) {
        let bottom = debug_ui.framebuffer_size().height as i32 - PADDING;

        // Draw effects button.
        let effects_button_position = Point2DI32::new(PADDING, bottom - BUTTON_HEIGHT);
        if self.draw_button(debug_ui, event, effects_button_position, &self.effects_texture) {
            self.effects_window_visible = !self.effects_window_visible;
        }

        // Draw open button.
        let open_button_x = PADDING + BUTTON_WIDTH + PADDING;
        let open_button_y = bottom - BUTTON_HEIGHT;
        let open_button_position = Point2DI32::new(open_button_x, open_button_y);
        self.draw_button(debug_ui, event, open_button_position, &self.open_texture);

        // Draw 3D switch.
        let threed_switch_x = PADDING + (BUTTON_WIDTH + PADDING) * 2;
        let threed_switch_origin = Point2DI32::new(threed_switch_x, open_button_y);
        debug_ui.draw_solid_rect(RectI32::new(threed_switch_origin,
                                              Point2DI32::new(SWITCH_SIZE, BUTTON_HEIGHT)),
                                 WINDOW_COLOR);
        self.threed_enabled = self.draw_switch(debug_ui,
                                               event,
                                               threed_switch_origin,
                                               "2D",
                                               "3D",
                                               self.threed_enabled);

        // Draw effects window, if necessary.
        if self.effects_window_visible {
            let effects_window_y = bottom - (BUTTON_HEIGHT + PADDING + EFFECTS_WINDOW_HEIGHT);
            debug_ui.draw_solid_rect(RectI32::new(Point2DI32::new(PADDING, effects_window_y),
                                                Point2DI32::new(EFFECTS_WINDOW_WIDTH,
                                                                EFFECTS_WINDOW_HEIGHT)),
                                    WINDOW_COLOR);
            self.gamma_correction_effect_enabled =
                self.draw_effects_switch(debug_ui,
                                        event,
                                        "Gamma Correction",
                                        0,
                                        effects_window_y,
                                        self.gamma_correction_effect_enabled);
            self.stem_darkening_effect_enabled =
                self.draw_effects_switch(debug_ui,
                                        event,
                                        "Stem Darkening",
                                        1,
                                        effects_window_y,
                                        self.stem_darkening_effect_enabled);
            self.subpixel_aa_effect_enabled =
                self.draw_effects_switch(debug_ui,
                                        event,
                                        "Subpixel AA",
                                        2,
                                        effects_window_y,
                                        self.subpixel_aa_effect_enabled);
        }
    }

    fn draw_button(&self,
                   debug_ui: &mut DebugUI,
                   event: &mut UIEvent,
                   origin: Point2DI32,
                   texture: &Texture)
                   -> bool {
        let button_rect = RectI32::new(origin, Point2DI32::new(BUTTON_WIDTH, BUTTON_HEIGHT));
        debug_ui.draw_solid_rect(button_rect, WINDOW_COLOR);
        debug_ui.draw_rect_outline(button_rect, TEXT_COLOR);
        debug_ui.draw_texture(origin + Point2DI32::new(PADDING, PADDING), texture, TEXT_COLOR);
        event.handle_mouse_down_in_rect(button_rect)
    }

    fn draw_effects_switch(&self,
                           debug_ui: &mut DebugUI,
                           event: &mut UIEvent,
                           text: &str,
                           index: i32,
                           window_y: i32,
                           value: bool)
                           -> bool {
        let text_x = PADDING * 2;
        let text_y = window_y + PADDING + BUTTON_TEXT_OFFSET + (BUTTON_HEIGHT + PADDING) * index;
        debug_ui.draw_text(text, Point2DI32::new(text_x, text_y), false);

        let switch_x = PADDING + EFFECTS_WINDOW_WIDTH - (SWITCH_SIZE + PADDING);
        let switch_y = window_y + PADDING + (BUTTON_HEIGHT + PADDING) * index;
        self.draw_switch(debug_ui, event, Point2DI32::new(switch_x, switch_y), "Off", "On", value)
    }

    fn draw_switch(&self,
                   debug_ui: &mut DebugUI,
                   event: &mut UIEvent,
                   origin: Point2DI32,
                   off_text: &str,
                   on_text: &str,
                   mut value: bool)
                   -> bool {
        let widget_rect = RectI32::new(origin, Point2DI32::new(SWITCH_SIZE, BUTTON_HEIGHT));
        if event.handle_mouse_down_in_rect(widget_rect) {
            value = !value;
        }

        debug_ui.draw_rect_outline(widget_rect, TEXT_COLOR);

        let highlight_size = Point2DI32::new(SWITCH_HALF_SIZE, BUTTON_HEIGHT);
        if !value {
            debug_ui.draw_solid_rect(RectI32::new(origin, highlight_size), TEXT_COLOR);
        } else {
            let x_offset = SWITCH_HALF_SIZE + 1;
            debug_ui.draw_solid_rect(RectI32::new(origin + Point2DI32::new(x_offset, 0),
                                                  highlight_size),
                                     TEXT_COLOR);
        }

        let off_size = debug_ui.measure_text(off_text);
        let on_size = debug_ui.measure_text(on_text);
        let off_offset = SWITCH_HALF_SIZE / 2 - off_size / 2;
        let on_offset  = SWITCH_HALF_SIZE + SWITCH_HALF_SIZE / 2 - on_size / 2;
        let text_top = BUTTON_TEXT_OFFSET;

        debug_ui.draw_text(off_text, origin + Point2DI32::new(off_offset, text_top), !value);
        debug_ui.draw_text(on_text, origin + Point2DI32::new(on_offset, text_top), value);

        value
    }
}

enum UIEvent {
    None,
    MouseDown(Point2DI32),
}

impl UIEvent {
    fn handle_mouse_down_in_rect(&mut self, rect: RectI32) -> bool {
        if let UIEvent::MouseDown(point) = *self {
            if rect.contains_point(point) {
                *self = UIEvent::None;
                return true;
            }
        }
        false
    }
}
