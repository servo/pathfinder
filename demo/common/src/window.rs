// pathfinder/demo/common/src/window.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A minimal cross-platform windowing layer.

use gl::types::GLuint;
use pathfinder_geometry::basic::point::Point2DI32;
use pathfinder_geometry::basic::rect::RectI32;
use pathfinder_geometry::basic::transform3d::Perspective;
use pathfinder_geometry::basic::transform3d::Transform3DF32;
use pathfinder_geometry::distortion::BarrelDistortionCoefficients;
use pathfinder_gl::GLVersion;
use pathfinder_gpu::resources::ResourceLoader;
use rayon::ThreadPoolBuilder;
use std::path::PathBuf;

pub trait Window {
    fn gl_version(&self) -> GLVersion;
    fn gl_default_framebuffer(&self) -> GLuint { 0 }
    fn mouse_position(&self) -> Point2DI32;
    fn view_box_size(&self, mode: Mode) -> Point2DI32;
    fn make_current(&mut self, mode: Mode, index: Option<u32>) -> RectI32;
    fn present(&mut self);
    fn resource_loader(&self) -> &dyn ResourceLoader;
    fn create_user_event_id(&self) -> u32;
    fn push_user_event(message_type: u32, message_data: u32);
    fn present_open_svg_dialog(&mut self);
    fn run_save_dialog(&self, extension: &str) -> Result<PathBuf, ()>;

    fn adjust_thread_pool_settings(&self, thread_pool_builder: ThreadPoolBuilder) -> ThreadPoolBuilder {
        thread_pool_builder
    }

    #[inline]
    fn barrel_distortion_coefficients(&self) -> BarrelDistortionCoefficients {
        BarrelDistortionCoefficients::default()
    }
}

pub enum Event {
    Quit,
    WindowResized(WindowSize),
    KeyDown(Keycode),
    KeyUp(Keycode),
    MouseDown(Point2DI32),
    MouseMoved(Point2DI32),
    MouseDragged(Point2DI32),
    Zoom(f32),
    Look { pitch: f32, yaw: f32 },
    CameraTransforms(Vec<CameraTransform>),
    OpenSVG(SVGPath),
    User { message_type: u32, message_data: u32 },
}

#[derive(Clone, Copy)]
pub enum Keycode {
    Alphanumeric(u8),
    Escape,
    Tab,
}

#[derive(Clone, Copy, Debug)]
pub struct WindowSize {
    pub logical_size: Point2DI32,
    pub backing_scale_factor: f32,
}

impl WindowSize {
    #[inline]
    pub fn device_size(&self) -> Point2DI32 {
        self.logical_size.to_f32().scale(self.backing_scale_factor).to_i32()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CameraTransform {
    // The perspective which converts from camera coordinates to display coordinates
    pub perspective: Perspective,

    // The view transform which converts from world coordinates to camera coordinates
    pub view: Transform3DF32,
}

#[derive(Clone)]
pub enum SVGPath {
    Default,
    Resource(String),
    Path(PathBuf),
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    TwoD   = 0,
    ThreeD = 1,
    VR     = 2,
}

impl Mode {
    pub fn viewport_count(self) -> usize {
        match self { Mode::TwoD | Mode::ThreeD => 1, Mode::VR => 2 }
    }
}
