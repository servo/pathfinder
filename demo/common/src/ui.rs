// pathfinder/demo/src/ui.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::Options;
use nfd::Response;
use pathfinder_geometry::basic::point::Point2DI32;
use pathfinder_geometry::basic::rect::RectI32;
use pathfinder_gl::debug::{BUTTON_HEIGHT, BUTTON_TEXT_OFFSET, BUTTON_WIDTH, DebugUI, PADDING};
use pathfinder_gl::debug::{TEXT_COLOR, WINDOW_COLOR};
use pathfinder_gl::device::{Device, Texture};
use std::f32::consts::PI;
use std::path::PathBuf;

const SWITCH_SIZE: i32 = SWITCH_HALF_SIZE * 2 + 1;
const SWITCH_HALF_SIZE: i32 = 96;

const SLIDER_WIDTH: i32 = 360;
const SLIDER_HEIGHT: i32 = 48;
const SLIDER_TRACK_HEIGHT: i32 = 24;
const SLIDER_KNOB_WIDTH: i32 = 12;
const SLIDER_KNOB_HEIGHT: i32 = 48;

const EFFECTS_PANEL_WIDTH: i32 = 550;
const EFFECTS_PANEL_HEIGHT: i32 = BUTTON_HEIGHT * 3 + PADDING * 4;

const ROTATE_PANEL_X: i32 = PADDING + (BUTTON_WIDTH + PADDING) * 2 + PADDING + SWITCH_SIZE;
const ROTATE_PANEL_WIDTH: i32 = SLIDER_WIDTH + PADDING * 2;
const ROTATE_PANEL_HEIGHT: i32 = PADDING * 2 + SLIDER_HEIGHT;

static EFFECTS_PNG_NAME: &'static str = "demo-effects";
static OPEN_PNG_NAME: &'static str = "demo-open";
static ROTATE_PNG_NAME: &'static str = "demo-rotate";
static ZOOM_IN_PNG_NAME: &'static str = "demo-zoom-in";
static ZOOM_OUT_PNG_NAME: &'static str = "demo-zoom-out";

pub struct DemoUI {
    effects_texture: Texture,
    open_texture: Texture,
    rotate_texture: Texture,
    zoom_in_texture: Texture,
    zoom_out_texture: Texture,

    effects_panel_visible: bool,
    rotate_panel_visible: bool,

    pub three_d_enabled: bool,
    pub gamma_correction_effect_enabled: bool,
    pub stem_darkening_effect_enabled: bool,
    pub subpixel_aa_effect_enabled: bool,
    pub rotation: i32,
}

impl DemoUI {
    pub fn new(device: &Device, options: Options) -> DemoUI {
        let effects_texture = device.create_texture_from_png(EFFECTS_PNG_NAME);
        let open_texture = device.create_texture_from_png(OPEN_PNG_NAME);
        let rotate_texture = device.create_texture_from_png(ROTATE_PNG_NAME);
        let zoom_in_texture = device.create_texture_from_png(ZOOM_IN_PNG_NAME);
        let zoom_out_texture = device.create_texture_from_png(ZOOM_OUT_PNG_NAME);

        DemoUI {
            effects_texture,
            open_texture,
            rotate_texture,
            zoom_in_texture,
            zoom_out_texture,
            three_d_enabled: options.three_d,
            effects_panel_visible: false,
            rotate_panel_visible: false,
            gamma_correction_effect_enabled: false,
            stem_darkening_effect_enabled: false,
            subpixel_aa_effect_enabled: false,
            rotation: SLIDER_WIDTH / 2,
        }
    }

    fn rotation(&self) -> f32 {
        (self.rotation as f32 / SLIDER_WIDTH as f32 * 2.0 - 1.0) * PI
    }

    pub fn update(&mut self, debug_ui: &mut DebugUI, event: &mut UIEvent, action: &mut UIAction) {
        let bottom = debug_ui.framebuffer_size().y() - PADDING;

        // Draw effects button.
        let effects_button_position = Point2DI32::new(PADDING, bottom - BUTTON_HEIGHT);
        if self.draw_button(debug_ui, event, effects_button_position, &self.effects_texture) {
            self.effects_panel_visible = !self.effects_panel_visible;
        }

        // Draw open button.
        let open_button_x = PADDING + BUTTON_WIDTH + PADDING;
        let open_button_y = bottom - BUTTON_HEIGHT;
        let open_button_position = Point2DI32::new(open_button_x, open_button_y);
        if self.draw_button(debug_ui, event, open_button_position, &self.open_texture) {
            if let Ok(Response::Okay(file)) = nfd::open_file_dialog(Some("svg"), None) {
                *action = UIAction::OpenFile(PathBuf::from(file));
            }
        }

        // Draw 3D switch.
        let threed_switch_x = PADDING + (BUTTON_WIDTH + PADDING) * 2;
        let threed_switch_origin = Point2DI32::new(threed_switch_x, open_button_y);
        debug_ui.draw_solid_rect(RectI32::new(threed_switch_origin,
                                              Point2DI32::new(SWITCH_SIZE, BUTTON_HEIGHT)),
                                 WINDOW_COLOR);
        self.three_d_enabled = self.draw_switch(debug_ui,
                                               event,
                                               threed_switch_origin,
                                               "2D",
                                               "3D",
                                               self.three_d_enabled);

        // Draw rotate and zoom buttons, if applicable.
        if !self.three_d_enabled {
            let rotate_button_y = bottom - BUTTON_HEIGHT;
            let rotate_button_position = Point2DI32::new(ROTATE_PANEL_X, rotate_button_y);
            if self.draw_button(debug_ui, event, rotate_button_position, &self.rotate_texture) {
                self.rotate_panel_visible = !self.rotate_panel_visible;
            }

            let zoom_in_button_x = ROTATE_PANEL_X + BUTTON_WIDTH + PADDING;
            let zoom_in_button_position = Point2DI32::new(zoom_in_button_x, rotate_button_y);
            if self.draw_button(debug_ui, event, zoom_in_button_position, &self.zoom_in_texture) {
                *action = UIAction::ZoomIn;
            }

            let zoom_out_button_x = ROTATE_PANEL_X + (BUTTON_WIDTH + PADDING) * 2;
            let zoom_out_button_position = Point2DI32::new(zoom_out_button_x, rotate_button_y);
            if self.draw_button(debug_ui,
                                event,
                                zoom_out_button_position,
                                &self.zoom_out_texture) {
                *action = UIAction::ZoomOut;
            }
        }

        // Draw effects panel, if necessary.
        self.draw_effects_panel(debug_ui, event);

        // Draw rotate panel, if necessary.
        self.draw_rotate_panel(debug_ui, event, action);
    }

    fn draw_effects_panel(&mut self, debug_ui: &mut DebugUI, event: &mut UIEvent) {
        if !self.effects_panel_visible {
            return;
        }

        let bottom = debug_ui.framebuffer_size().y() - PADDING;
        let effects_panel_y = bottom - (BUTTON_HEIGHT + PADDING + EFFECTS_PANEL_HEIGHT);
        debug_ui.draw_solid_rect(RectI32::new(Point2DI32::new(PADDING, effects_panel_y),
                                              Point2DI32::new(EFFECTS_PANEL_WIDTH,
                                                              EFFECTS_PANEL_HEIGHT)),
                                WINDOW_COLOR);

        self.gamma_correction_effect_enabled =
            self.draw_effects_switch(debug_ui,
                                     event,
                                     "Gamma Correction",
                                     0,
                                     effects_panel_y,
                                     self.gamma_correction_effect_enabled);
        self.stem_darkening_effect_enabled =
            self.draw_effects_switch(debug_ui,
                                     event,
                                     "Stem Darkening",
                                     1,
                                     effects_panel_y,
                                     self.stem_darkening_effect_enabled);
        self.subpixel_aa_effect_enabled =
            self.draw_effects_switch(debug_ui,
                                     event,
                                     "Subpixel AA",
                                     2,
                                     effects_panel_y,
                                     self.subpixel_aa_effect_enabled);

    }

    fn draw_rotate_panel(&mut self,
                         debug_ui: &mut DebugUI,
                         event: &mut UIEvent,
                         action: &mut UIAction) {
        if !self.rotate_panel_visible {
            return;
        }

        let bottom = debug_ui.framebuffer_size().y() - PADDING;
        let rotate_panel_y = bottom - (BUTTON_HEIGHT + PADDING + ROTATE_PANEL_HEIGHT);
        debug_ui.draw_solid_rect(RectI32::new(Point2DI32::new(ROTATE_PANEL_X, rotate_panel_y),
                                              Point2DI32::new(ROTATE_PANEL_WIDTH,
                                                              ROTATE_PANEL_HEIGHT)),
                                 WINDOW_COLOR);

        let (widget_x, widget_y) = (ROTATE_PANEL_X + PADDING, rotate_panel_y + PADDING);
        let widget_rect = RectI32::new(Point2DI32::new(widget_x, widget_y),
                                       Point2DI32::new(SLIDER_WIDTH, SLIDER_KNOB_HEIGHT));
        if let Some(position) = event.handle_mouse_down_or_dragged_in_rect(widget_rect) {
            self.rotation = position.x();
            *action = UIAction::Rotate(self.rotation());
        }

        let slider_track_y = rotate_panel_y + PADDING + SLIDER_KNOB_HEIGHT / 2 -
            SLIDER_TRACK_HEIGHT / 2;
        let slider_track_rect =
            RectI32::new(Point2DI32::new(widget_x, slider_track_y),
                         Point2DI32::new(SLIDER_WIDTH, SLIDER_TRACK_HEIGHT));
        debug_ui.draw_rect_outline(slider_track_rect, TEXT_COLOR);

        let slider_knob_x = widget_x + self.rotation - SLIDER_KNOB_WIDTH / 2;
        let slider_knob_rect =
            RectI32::new(Point2DI32::new(slider_knob_x, widget_y),
                         Point2DI32::new(SLIDER_KNOB_WIDTH, SLIDER_KNOB_HEIGHT));
        debug_ui.draw_solid_rect(slider_knob_rect, TEXT_COLOR);
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
        event.handle_mouse_down_in_rect(button_rect).is_some()
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

        let switch_x = PADDING + EFFECTS_PANEL_WIDTH - (SWITCH_SIZE + PADDING);
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
        if event.handle_mouse_down_in_rect(widget_rect).is_some() {
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

#[derive(Clone, Debug, PartialEq)]
pub enum UIAction {
    None,
    OpenFile(PathBuf),
    ZoomIn,
    ZoomOut,
    Rotate(f32),
}

pub enum UIEvent {
    None,
    MouseDown(Point2DI32),
    MouseDragged {
        absolute_position: Point2DI32,
        relative_position: Point2DI32,
    }
}

impl UIEvent {
    pub fn is_none(&self) -> bool {
        match *self { UIEvent::None => true, _ => false }
    }

    fn handle_mouse_down_in_rect(&mut self, rect: RectI32) -> Option<Point2DI32> {
        if let UIEvent::MouseDown(point) = *self {
            if rect.contains_point(point) {
                *self = UIEvent::None;
                return Some(point - rect.origin());
            }
        }
        None
    }

    fn handle_mouse_down_or_dragged_in_rect(&mut self, rect: RectI32) -> Option<Point2DI32> {
        match *self {
            UIEvent::MouseDown(point) | UIEvent::MouseDragged { absolute_position: point, .. }
                    if rect.contains_point(point) => {
                *self = UIEvent::None;
                Some(point - rect.origin())
            }
            _ => None,
        }
    }
}
