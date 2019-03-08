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

use pathfinder_geometry::basic::point::Point2DI32;

pub trait Window {
    fn new(initial_size: Point2DI32) -> Self;
    fn size(&self) -> Point2DI32;
    fn drawable_size(&self) -> Point2DI32;
    fn mouse_position(&self) -> Point2DI32;
    fn get_event(&mut self) -> Event;
    fn try_get_event(&mut self) -> Option<Event>;
    fn present(&self);
    fn create_user_event_id(&self) -> u32;
    fn push_user_event(message_type: u32, message_data: u32);
}

pub enum Event {
    Quit,
    WindowResized,
    KeyDown(Keycode),
    KeyUp(Keycode),
    MouseDown(Point2DI32),
    MouseMoved { position: Point2DI32, relative_position: Point2DI32 },
    MouseDragged { position: Point2DI32, relative_position: Point2DI32 },
    Zoom(f32),
    User { message_type: u32, message_data: u32 },
}

#[derive(Clone, Copy)]
pub enum Keycode {
    Alphanumeric(u8),
    Escape,
}
