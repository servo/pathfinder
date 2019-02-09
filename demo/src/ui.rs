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
use pathfinder_geometry::basic::point::Point2DI32;
use pathfinder_geometry::basic::rect::RectI32;
use pathfinder_gl::debug::{BUTTON_HEIGHT, BUTTON_TEXT_OFFSET, BUTTON_WIDTH, DebugUI, PADDING};
use pathfinder_gl::debug::{TEXT_COLOR, WINDOW_COLOR};
use pathfinder_gl::device::Texture;

const SWITCH_SIZE: i32 = SWITCH_HALF_SIZE * 2 + 1;
const SWITCH_HALF_SIZE: i32 = 96;

const EFFECTS_WINDOW_WIDTH: i32 = 550;
const EFFECTS_WINDOW_HEIGHT: i32 = BUTTON_HEIGHT * 3 + PADDING * 4;

static EFFECTS_PNG_NAME: &'static str = "demo-effects";
static OPEN_PNG_NAME: &'static str = "demo-open";

pub struct DemoUI {
    effects_texture: Texture,
    open_texture: Texture,

    effects_window_visible: bool,
    pub threed_enabled: bool,
    pub gamma_correction_effect_enabled: bool,
    pub stem_darkening_effect_enabled: bool,
    pub subpixel_aa_effect_enabled: bool,
}

impl DemoUI {
    pub fn new(options: Options) -> DemoUI {
        let effects_texture = Texture::from_png(EFFECTS_PNG_NAME);
        let open_texture = Texture::from_png(OPEN_PNG_NAME);

        DemoUI {
            effects_texture,
            open_texture,
            threed_enabled: options.threed,
            effects_window_visible: false,
            gamma_correction_effect_enabled: false,
            stem_darkening_effect_enabled: false,
            subpixel_aa_effect_enabled: false,
        }
    }

    pub fn update(&mut self, debug_ui: &mut DebugUI, event: &mut UIEvent) {
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
        self.draw_effects_window(debug_ui, event);
    }

    fn draw_effects_window(&mut self, debug_ui: &mut DebugUI, event: &mut UIEvent) {
        if !self.effects_window_visible {
            return;
        }

        let bottom = debug_ui.framebuffer_size().height as i32 - PADDING;
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

pub enum UIEvent {
    None,
    MouseDown(Point2DI32),
}

impl UIEvent {
    pub fn is_none(&self) -> bool {
        match *self { UIEvent::None => true, _ => false }
    }

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
