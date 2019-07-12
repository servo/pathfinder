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

#[macro_use]
extern crate log;

// Mode is used in Options, so has to be public
pub use crate::camera::Mode;

use crate::camera::Camera;
use crate::concurrent::DemoExecutor;
use crate::device::{GroundProgram, GroundVertexArray};
use crate::ui::{DemoUIModel, DemoUIPresenter, ScreenshotInfo, ScreenshotType, UIAction};
use crate::window::{Event, Keycode, SVGPath, Window, WindowSize};
use clap::{App, Arg};
use pathfinder_content::color::ColorU;
use pathfinder_export::{Export, FileFormat};
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::transform3d::Transform4F;
use pathfinder_geometry::vector::{Vector2F, Vector2I, Vector4F};
use pathfinder_gpu::resources::ResourceLoader;
use pathfinder_gpu::Device;
use pathfinder_renderer::concurrent::scene_proxy::{RenderCommandStream, SceneProxy};
use pathfinder_renderer::gpu::options::{DestFramebuffer, RendererOptions};
use pathfinder_renderer::gpu::renderer::{RenderStats, RenderTime, Renderer};
use pathfinder_renderer::options::{BuildOptions, RenderTransform};
use pathfinder_renderer::post::STEM_DARKENING_FACTORS;
use pathfinder_renderer::scene::Scene;
use pathfinder_svg::BuiltSVG;
use pathfinder_ui::{MousePosition, UIEvent};
use std::fs::File;
use std::io::{BufWriter, Read};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use usvg::{Options as UsvgOptions, Tree};

#[cfg(any(not(target_os = "macos"), feature = "pf-gl"))]
use pathfinder_gl::GLDevice as DeviceImpl;
#[cfg(all(target_os = "macos", not(feature = "pf-gl")))]
use pathfinder_metal::MetalDevice as DeviceImpl;

static DEFAULT_SVG_VIRTUAL_PATH: &'static str = "svg/Ghostscript_Tiger.svg";

const MOUSELOOK_ROTATION_SPEED: f32 = 0.007;
const CAMERA_VELOCITY: f32 = 0.02;

// How much the scene is scaled when a scale gesture is performed.
const CAMERA_SCALE_SPEED_2D: f32 = 6.0;
// How much the scene is scaled when a zoom button is clicked.
const CAMERA_ZOOM_AMOUNT_2D: f32 = 0.1;

// Half of the eye separation distance.
const DEFAULT_EYE_OFFSET: f32 = 0.025;

const LIGHT_BG_COLOR: ColorU = ColorU {
    r: 248,
    g: 248,
    b: 248,
    a: 255,
};
const DARK_BG_COLOR: ColorU = ColorU {
    r: 32,
    g: 32,
    b: 32,
    a: 255,
};
const TRANSPARENT_BG_COLOR: ColorU = ColorU {
    r: 0,
    g: 0,
    b: 0,
    a: 0,
};

const APPROX_FONT_SIZE: f32 = 16.0;

const MESSAGE_TIMEOUT_SECS: u64 = 5;

pub mod window;

mod camera;
mod concurrent;
mod device;
mod renderer;
mod ui;

pub struct DemoApp<W> where W: Window {
    pub window: W,
    pub should_exit: bool,
    pub options: Options,

    window_size: WindowSize,

    scene_metadata: SceneMetadata,
    render_transform: Option<RenderTransform>,
    render_command_stream: Option<RenderCommandStream>,

    camera: Camera,
    frame_counter: u32,
    pending_screenshot_info: Option<ScreenshotInfo>,
    mouselook_enabled: bool,
    pub dirty: bool,
    expire_message_event_id: u32,
    message_epoch: u32,
    last_mouse_position: Vector2I,

    current_frame: Option<Frame>,
    build_time: Option<Duration>,

    ui_model: DemoUIModel,
    ui_presenter: DemoUIPresenter<DeviceImpl>,

    scene_proxy: SceneProxy,
    renderer: Renderer<DeviceImpl>,

    scene_framebuffer: Option<<DeviceImpl as Device>::Framebuffer>,

    ground_program: GroundProgram<DeviceImpl>,
    ground_vertex_array: GroundVertexArray<DeviceImpl>,
}

impl<W> DemoApp<W> where W: Window {
    pub fn new(window: W, window_size: WindowSize, mut options: Options) -> DemoApp<W> {
        let expire_message_event_id = window.create_user_event_id();

        let device;
        #[cfg(all(target_os = "macos", not(feature = "pf-gl")))]
        {
            device = DeviceImpl::new(window.metal_layer());
        }
        #[cfg(any(not(target_os = "macos"), feature = "pf-gl"))]
        {
            device = DeviceImpl::new(window.gl_version(), window.gl_default_framebuffer());
        }

        let resources = window.resource_loader();

        // Read command line options.
        options.command_line_overrides();

        // Set up the executor.
        let executor = DemoExecutor::new(options.jobs);

        let mut built_svg = load_scene(resources, &options.input_path);
        let message = get_svg_building_message(&built_svg);

        let viewport = window.viewport(options.mode.view(0));
        let dest_framebuffer = DestFramebuffer::Default {
            viewport,
            window_size: window_size.device_size(),
        };
        // FIXME(pcwalton)
        let render_options = RendererOptions {
            background_color: None,
        };

        let renderer = Renderer::new(device, resources, dest_framebuffer, render_options);
        let scene_metadata = SceneMetadata::new_clipping_view_box(&mut built_svg.scene,
                                                                  viewport.size());
        let camera = Camera::new(options.mode, scene_metadata.view_box, viewport.size());

        let scene_proxy = SceneProxy::from_scene(built_svg.scene, executor);

        let ground_program = GroundProgram::new(&renderer.device, resources);
        let ground_vertex_array = GroundVertexArray::new(&renderer.device,
                                                         &ground_program,
                                                         &renderer.quad_vertex_positions_buffer(),
                                                         &renderer.quad_vertex_indices_buffer());

        let mut ui_model = DemoUIModel::new(&options);

        let mut message_epoch = 0;
        emit_message::<W>(
            &mut ui_model,
            &mut message_epoch,
            expire_message_event_id,
            message,
        );

        let ui_presenter = DemoUIPresenter::new(&renderer.device, resources);

        DemoApp {
            window,
            should_exit: false,
            options,

            window_size,

            scene_metadata,
            render_transform: None,
            render_command_stream: None,

            camera,
            frame_counter: 0,
            pending_screenshot_info: None,
            mouselook_enabled: false,
            dirty: true,
            expire_message_event_id,
            message_epoch,
            last_mouse_position: Vector2I::default(),

            current_frame: None,
            build_time: None,

            ui_presenter,
            ui_model,

            scene_proxy,
            renderer,

            scene_framebuffer: None,

            ground_program,
            ground_vertex_array,
        }
    }

    pub fn prepare_frame(&mut self, events: Vec<Event>) -> u32 {
        // Clear dirty flag.
        self.dirty = false;

        // Handle events.
        let ui_events = self.handle_events(events);

        // Update the scene.
        self.build_scene();

        // Save the frame.
        //
        // FIXME(pcwalton): This is super ugly.
        let transform = self.render_transform.clone().unwrap();
        self.current_frame = Some(Frame::new(transform, ui_events));

        // Prepare to render the frame.
        self.prepare_frame_rendering()
    }

    fn build_scene(&mut self) {
        self.render_transform = match self.camera {
            Camera::ThreeD {
                ref scene_transform,
                ref mut modelview_transform,
                ref mut velocity,
                ..
            } => {
                if modelview_transform.offset(*velocity) {
                    self.dirty = true;
                }
                let perspective = scene_transform.perspective *
                    scene_transform.modelview_to_eye *
                    modelview_transform.to_transform();
                Some(RenderTransform::Perspective(perspective))
            }
            Camera::TwoD(transform) => Some(RenderTransform::Transform2D(transform)),
        };

        let build_options = BuildOptions {
            transform: self.render_transform.clone().unwrap(),
            dilation: if self.ui_model.stem_darkening_effect_enabled {
                let font_size = APPROX_FONT_SIZE * self.window_size.backing_scale_factor;
                let (x, y) = (STEM_DARKENING_FACTORS[0], STEM_DARKENING_FACTORS[1]);
                Vector2F::new(x, y).scale(font_size)
            } else {
                Vector2F::default()
            },
            subpixel_aa_enabled: self.ui_model.subpixel_aa_effect_enabled,
        };

        self.render_command_stream = Some(self.scene_proxy.build_with_stream(build_options));
    }

    fn handle_events(&mut self, events: Vec<Event>) -> Vec<UIEvent> {
        let mut ui_events = vec![];
        self.dirty = false;

        for event in events {
            match event {
                Event::Quit { .. } | Event::KeyDown(Keycode::Escape) => {
                    self.should_exit = true;
                    self.dirty = true;
                }
                Event::WindowResized(new_size) => {
                    self.window_size = new_size;
                    let viewport = self.window.viewport(self.ui_model.mode.view(0));
                    self.scene_proxy.set_view_box(RectF::new(Vector2F::default(),
                                                               viewport.size().to_f32()));
                    self.renderer
                        .set_main_framebuffer_size(self.window_size.device_size());
                    self.dirty = true;
                }
                Event::MouseDown(new_position) => {
                    let mouse_position = self.process_mouse_position(new_position);
                    ui_events.push(UIEvent::MouseDown(mouse_position));
                }
                Event::MouseMoved(new_position) if self.mouselook_enabled => {
                    let mouse_position = self.process_mouse_position(new_position);
                    if let Camera::ThreeD {
                        ref mut modelview_transform,
                        ..
                    } = self.camera
                    {
                        let rotation = mouse_position
                            .relative
                            .to_f32()
                            .scale(MOUSELOOK_ROTATION_SPEED);
                        modelview_transform.yaw += rotation.x();
                        modelview_transform.pitch += rotation.y();
                        self.dirty = true;
                    }
                }
                Event::MouseDragged(new_position) => {
                    let mouse_position = self.process_mouse_position(new_position);
                    ui_events.push(UIEvent::MouseDragged(mouse_position));
                    self.dirty = true;
                }
                Event::Zoom(d_dist, position) => {
                    if let Camera::TwoD(ref mut transform) = self.camera {
                        let backing_scale_factor = self.window_size.backing_scale_factor;
                        let position = position.to_f32().scale(backing_scale_factor);
                        *transform = Transform2F::from_translation(-position) * *transform;
                        let scale_delta = 1.0 + d_dist * CAMERA_SCALE_SPEED_2D;
                        *transform = Transform2F::from_uniform_scale(scale_delta) * *transform;
                        *transform = Transform2F::from_translation(position) * *transform;
                    }
                }
                Event::Look { pitch, yaw } => {
                    if let Camera::ThreeD {
                        ref mut modelview_transform,
                        ..
                    } = self.camera
                    {
                        modelview_transform.pitch += pitch;
                        modelview_transform.yaw += yaw;
                    }
                }
                Event::SetEyeTransforms(new_eye_transforms) => {
                    if let Camera::ThreeD {
                        ref mut scene_transform,
                        ref mut eye_transforms,
                        ..
                    } = self.camera
                    {
                        *eye_transforms = new_eye_transforms;
                        // Calculate the new scene transform by lerp'ing the eye transforms.
                        *scene_transform = eye_transforms[0];
                        for (index, eye_transform) in eye_transforms.iter().enumerate().skip(1) {
                            let weight = 1.0 / (index + 1) as f32;
                            scene_transform.perspective.transform =
                                scene_transform.perspective 
                                               .transform
                                               .lerp(weight, &eye_transform.perspective.transform);
                            scene_transform.modelview_to_eye =
                                scene_transform.modelview_to_eye
                                               .lerp(weight, &eye_transform.modelview_to_eye);
                         }
                        // TODO: calculate the eye offset from the eye transforms?
                        let z_offset = -DEFAULT_EYE_OFFSET *
                            scene_transform.perspective.transform.c0.x();
                        let z_offset = Vector4F::new(0.0, 0.0, z_offset, 1.0);
                        scene_transform.modelview_to_eye =
                            Transform4F::from_translation(z_offset) *
                            scene_transform.modelview_to_eye;
                    }
                }
                Event::KeyDown(Keycode::Alphanumeric(b'w')) => {
                    if let Camera::ThreeD {
                        ref mut velocity, ..
                    } = self.camera
                    {
                        let scale_factor =
                            camera::scale_factor_for_view_box(self.scene_metadata.view_box);
                        velocity.set_z(-CAMERA_VELOCITY / scale_factor);
                        self.dirty = true;
                    }
                }
                Event::KeyDown(Keycode::Alphanumeric(b's')) => {
                    if let Camera::ThreeD {
                        ref mut velocity, ..
                    } = self.camera
                    {
                        let scale_factor =
                            camera::scale_factor_for_view_box(self.scene_metadata.view_box);
                        velocity.set_z(CAMERA_VELOCITY / scale_factor);
                        self.dirty = true;
                    }
                }
                Event::KeyDown(Keycode::Alphanumeric(b'a')) => {
                    if let Camera::ThreeD {
                        ref mut velocity, ..
                    } = self.camera
                    {
                        let scale_factor =
                            camera::scale_factor_for_view_box(self.scene_metadata.view_box);
                        velocity.set_x(-CAMERA_VELOCITY / scale_factor);
                        self.dirty = true;
                    }
                }
                Event::KeyDown(Keycode::Alphanumeric(b'd')) => {
                    if let Camera::ThreeD {
                        ref mut velocity, ..
                    } = self.camera
                    {
                        let scale_factor =
                            camera::scale_factor_for_view_box(self.scene_metadata.view_box);
                        velocity.set_x(CAMERA_VELOCITY / scale_factor);
                        self.dirty = true;
                    }
                }
                Event::KeyUp(Keycode::Alphanumeric(b'w'))
                | Event::KeyUp(Keycode::Alphanumeric(b's')) => {
                    if let Camera::ThreeD {
                        ref mut velocity, ..
                    } = self.camera
                    {
                        velocity.set_z(0.0);
                        self.dirty = true;
                    }
                }
                Event::KeyUp(Keycode::Alphanumeric(b'a'))
                | Event::KeyUp(Keycode::Alphanumeric(b'd')) => {
                    if let Camera::ThreeD {
                        ref mut velocity, ..
                    } = self.camera
                    {
                        velocity.set_x(0.0);
                        self.dirty = true;
                    }
                }
                Event::KeyDown(Keycode::Tab) => {
                    self.options.ui = match self.options.ui {
                        UIVisibility::None => UIVisibility::Stats,
                        UIVisibility::Stats => UIVisibility::All,
                        UIVisibility::All => UIVisibility::None,
                    }
                }

                Event::OpenSVG(ref svg_path) => {
                    let mut built_svg = load_scene(self.window.resource_loader(), svg_path);
                    self.ui_model.message = get_svg_building_message(&built_svg);

                    let viewport_size = self.window.viewport(self.ui_model.mode.view(0)).size();
                    self.scene_metadata =
                        SceneMetadata::new_clipping_view_box(&mut built_svg.scene, viewport_size);
                    self.camera = Camera::new(self.ui_model.mode,
                                              self.scene_metadata.view_box,
                                              viewport_size);

                    self.scene_proxy.replace_scene(built_svg.scene);

                    self.dirty = true;
                }

                Event::User {
                    message_type: event_id,
                    message_data: expected_epoch,
                } if event_id == self.expire_message_event_id
                    && expected_epoch as u32 == self.message_epoch =>
                {
                    self.ui_model.message = String::new();
                    self.dirty = true;
                }
                _ => continue,
            }
        }

        ui_events
    }

    fn process_mouse_position(&mut self, new_position: Vector2I) -> MousePosition {
        let absolute = new_position.scale(self.window_size.backing_scale_factor as i32);
        let relative = absolute - self.last_mouse_position;
        self.last_mouse_position = absolute;
        MousePosition { absolute, relative }
    }

    pub fn finish_drawing_frame(&mut self) {
        self.maybe_take_screenshot();
        self.update_stats();
        self.draw_debug_ui();

        let frame = self.current_frame.take().unwrap();
        for ui_event in &frame.ui_events {
            self.dirty = true;
            self.renderer.debug_ui_presenter.ui_presenter.event_queue.push(*ui_event);
        }

        self.renderer.debug_ui_presenter.ui_presenter.mouse_position = self
            .last_mouse_position
            .to_f32()
            .scale(self.window_size.backing_scale_factor);

        self.ui_presenter.set_show_text_effects(self.scene_metadata.monochrome_color.is_some());

        let mut ui_action = UIAction::None;
        if self.options.ui == UIVisibility::All {
            self.ui_presenter.update(
                &self.renderer.device,
                &mut self.window,
                &mut self.renderer.debug_ui_presenter,
                &mut ui_action,
                &mut self.ui_model,
            );
        }

        self.handle_ui_events(frame, &mut ui_action);

        self.renderer.device.end_commands();

        self.window.present(&mut self.renderer.device);
        self.frame_counter += 1;
    }

    fn update_stats(&mut self) {
        let frame = self.current_frame.as_mut().unwrap();
        if let Some(rendering_time) = self.renderer.shift_rendering_time() {
            frame.scene_rendering_times.push(rendering_time);
        }

        if frame.scene_stats.is_empty() && frame.scene_rendering_times.is_empty() {
            return
        }

        let build_time = self.build_time.unwrap();

        let zero = RenderStats::default();
        let aggregate_stats = frame.scene_stats.iter().fold(zero, |sum, item| sum + *item);
        let total_rendering_time = if frame.scene_rendering_times.is_empty() {
            None
        } else {
            Some(
                frame
                    .scene_rendering_times
                    .iter()
                    .fold(RenderTime::default(), |sum, item| sum + *item),
            )
        };

        self.renderer.debug_ui_presenter.add_sample(aggregate_stats,
                                                    build_time,
                                                    total_rendering_time);
    }

    fn maybe_take_screenshot(&mut self) {
        match self.pending_screenshot_info.take() {
            None => {}
            Some(ScreenshotInfo { kind: ScreenshotType::PNG, path }) => {
                self.take_raster_screenshot(path)
            }
            Some(ScreenshotInfo { kind: ScreenshotType::SVG, path }) => {
                // FIXME(pcwalton): This won't work on Android.
                let mut writer = BufWriter::new(File::create(path).unwrap());
                self.scene_proxy.copy_scene().export(&mut writer, FileFormat::SVG).unwrap();
            }
        }
    }

    fn handle_ui_events(&mut self, mut frame: Frame, ui_action: &mut UIAction) {
        frame.ui_events = self.renderer.debug_ui_presenter.ui_presenter.event_queue.drain();

        self.handle_ui_action(ui_action);

        // Switch camera mode (2D/3D) if requested.
        //
        // FIXME(pcwalton): This should really be an MVC setup.
        if self.camera.mode() != self.ui_model.mode {
            let viewport_size = self.window.viewport(self.ui_model.mode.view(0)).size();
            self.camera = Camera::new(self.ui_model.mode,
                                      self.scene_metadata.view_box,
                                      viewport_size);
        }

        for ui_event in frame.ui_events {
            match ui_event {
                UIEvent::MouseDown(_) if self.camera.is_3d() => {
                    // If nothing handled the mouse-down event, toggle mouselook.
                    self.mouselook_enabled = !self.mouselook_enabled;
                }
                UIEvent::MouseDragged(position) => {
                    if let Camera::TwoD(ref mut transform) = self.camera {
                        *transform = Transform2F::from_translation(position.relative.to_f32()) *
                            *transform;
                    }
                }
                _ => {}
            }
        }
    }

    fn handle_ui_action(&mut self, ui_action: &mut UIAction) {
        match ui_action {
            UIAction::None => {}
            UIAction::ModelChanged => self.dirty = true,
            UIAction::TakeScreenshot(ref info) => {
                self.pending_screenshot_info = Some((*info).clone());
                self.dirty = true;
            }
            UIAction::ZoomIn => {
                if let Camera::TwoD(ref mut transform) = self.camera {
                    let scale = Vector2F::splat(1.0 + CAMERA_ZOOM_AMOUNT_2D);
                    let center = center_of_window(&self.window_size);
                    *transform = Transform2F::from_translation(center) *
                        Transform2F::from_scale(scale) *
                        Transform2F::from_translation(-center) *
                        *transform;
                    self.dirty = true;
                }
            }
            UIAction::ZoomOut => {
                if let Camera::TwoD(ref mut transform) = self.camera {
                    let scale = Vector2F::splat(1.0 - CAMERA_ZOOM_AMOUNT_2D);
                    let center = center_of_window(&self.window_size);
                    *transform = Transform2F::from_translation(center) *
                        Transform2F::from_scale(scale) *
                        Transform2F::from_translation(-center) *
                        *transform;
                    self.dirty = true;
                }
            }
            UIAction::ZoomActualSize => {
                if let Camera::TwoD(ref mut transform) = self.camera {
                    *transform = Transform2F::default();
                    self.dirty = true;
                }
            }
            UIAction::Rotate(theta) => {
                if let Camera::TwoD(ref mut transform) = self.camera {
                    let old_rotation = transform.rotation();
                    let center = center_of_window(&self.window_size);
                    *transform = Transform2F::from_translation(-center) *
                        Transform2F::from_rotation(*theta - old_rotation) *
                        Transform2F::from_translation(center) *
                        *transform;
                }
            }
        }
    }

    fn background_color(&self) -> ColorU {
        match self.ui_model.background_color {
            BackgroundColor::Light => LIGHT_BG_COLOR,
            BackgroundColor::Dark => DARK_BG_COLOR,
            BackgroundColor::Transparent => TRANSPARENT_BG_COLOR,
        }
    }
}

#[derive(Clone)]
pub struct Options {
    pub jobs: Option<usize>,
    pub mode: Mode,
    pub input_path: SVGPath,
    pub ui: UIVisibility,
    pub background_color: BackgroundColor,
    hidden_field_for_future_proofing: (),
}

impl Default for Options {
    fn default() -> Self {
        Options {
            jobs: None,
            mode: Mode::TwoD,
            input_path: SVGPath::Default,
            ui: UIVisibility::All,
            background_color: BackgroundColor::Light,
            hidden_field_for_future_proofing: (),
        }
    }
}

impl Options {
    fn command_line_overrides(&mut self) {
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
                    .help("Run in 3D")
                    .conflicts_with("vr"),
            )
            .arg(
                Arg::with_name("vr")
                    .short("V")
                    .long("vr")
                    .help("Run in VR")
                    .conflicts_with("3d"),
            )
            .arg(
                Arg::with_name("ui")
                    .short("u")
                    .long("ui")
                    .takes_value(true)
                    .possible_values(&["none", "stats", "all"])
                    .help("How much UI to show"),
            )
            .arg(
                Arg::with_name("background")
                    .short("b")
                    .long("background")
                    .takes_value(true)
                    .possible_values(&["light", "dark", "transparent"])
                    .help("The background color to use"),
            )
            .arg(
                Arg::with_name("INPUT")
                    .help("Path to the SVG file to render")
                    .index(1),
            )
            .get_matches();

        if let Some(jobs) = matches.value_of("jobs") {
            self.jobs = jobs.parse().ok();
        }

        if matches.is_present("3d") {
            self.mode = Mode::ThreeD;
        } else if matches.is_present("vr") {
            self.mode = Mode::VR;
        }

        if let Some(ui) = matches.value_of("ui") {
            self.ui = match ui {
                "none" => UIVisibility::None,
                "stats" => UIVisibility::Stats,
                _ => UIVisibility::All,
            };
        }

        if let Some(background_color) = matches.value_of("background") {
            self.background_color = match background_color {
                "light" => BackgroundColor::Light,
                "dark" => BackgroundColor::Dark,
                _ => BackgroundColor::Transparent,
            };
        }

        if let Some(path) = matches.value_of("INPUT") {
            self.input_path = SVGPath::Path(PathBuf::from(path));
        };
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum UIVisibility {
    None,
    Stats,
    All,
}

fn load_scene(resource_loader: &dyn ResourceLoader, input_path: &SVGPath) -> BuiltSVG {
    let mut data;
    match *input_path {
        SVGPath::Default => data = resource_loader.slurp(DEFAULT_SVG_VIRTUAL_PATH).unwrap(),
        SVGPath::Resource(ref name) => data = resource_loader.slurp(name).unwrap(),
        SVGPath::Path(ref path) => {
            data = vec![];
            File::open(path).unwrap().read_to_end(&mut data).unwrap();
        }
    };

    BuiltSVG::from_tree(Tree::from_data(&data, &UsvgOptions::default()).unwrap())
}

fn center_of_window(window_size: &WindowSize) -> Vector2F {
    window_size.device_size().to_f32().scale(0.5)
}

fn get_svg_building_message(built_svg: &BuiltSVG) -> String {
    if built_svg.result_flags.is_empty() {
        return String::new();
    }
    format!(
        "Warning: These features in the SVG are unsupported: {}.",
        built_svg.result_flags
    )
}

fn emit_message<W>(
    ui_model: &mut DemoUIModel,
    message_epoch: &mut u32,
    expire_message_event_id: u32,
    message: String,
) where
    W: Window,
{
    if message.is_empty() {
        return;
    }

    ui_model.message = message;
    let expected_epoch = *message_epoch + 1;
    *message_epoch = expected_epoch;
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(MESSAGE_TIMEOUT_SECS));
        W::push_user_event(expire_message_event_id, expected_epoch);
    });
}

struct Frame {
    transform: RenderTransform,
    ui_events: Vec<UIEvent>,
    scene_rendering_times: Vec<RenderTime>,
    scene_stats: Vec<RenderStats>,
}

impl Frame {
    fn new(transform: RenderTransform, ui_events: Vec<UIEvent>) -> Frame {
        Frame {
            transform,
            ui_events,
            scene_rendering_times: vec![],
            scene_stats: vec![],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BackgroundColor {
    Light = 0,
    Dark = 1,
    Transparent = 2,
}

impl BackgroundColor {
    fn as_str(&self) -> &'static str {
        match *self {
            BackgroundColor::Light => "Light",
            BackgroundColor::Dark => "Dark",
            BackgroundColor::Transparent => "Transparent",
        }
    }
}

struct SceneMetadata {
    view_box: RectF,
    monochrome_color: Option<ColorU>,
}

impl SceneMetadata {
    // FIXME(pcwalton): The fact that this mutates the scene is really ugly!
    // Can we simplify this?
    fn new_clipping_view_box(scene: &mut Scene, viewport_size: Vector2I) -> SceneMetadata {
        let view_box = scene.view_box();
        let monochrome_color = scene.monochrome_color();
        scene.set_view_box(RectF::new(Vector2F::default(), viewport_size.to_f32()));
        SceneMetadata { view_box, monochrome_color }
    }
}
