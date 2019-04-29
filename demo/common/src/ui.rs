// pathfinder/demo/src/ui.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::window::Window;
use crate::{BackgroundColor, Mode, Options};
use pathfinder_geometry::basic::point::Point2DI32;
use pathfinder_geometry::basic::rect::RectI32;
use pathfinder_gpu::resources::ResourceLoader;
use pathfinder_gpu::Device;
use pathfinder_renderer::gpu::debug::DebugUI;
use pathfinder_ui::{BUTTON_HEIGHT, BUTTON_TEXT_OFFSET, BUTTON_WIDTH, FONT_ASCENT, PADDING};
use pathfinder_ui::{TEXT_COLOR, TOOLTIP_HEIGHT, WINDOW_COLOR};
use std::f32::consts::PI;
use std::path::PathBuf;

const SLIDER_WIDTH: i32 = 360;
const SLIDER_HEIGHT: i32 = 48;
const SLIDER_TRACK_HEIGHT: i32 = 24;
const SLIDER_KNOB_WIDTH: i32 = 12;
const SLIDER_KNOB_HEIGHT: i32 = 48;

const EFFECTS_PANEL_WIDTH: i32 = 550;
const EFFECTS_PANEL_HEIGHT: i32 = BUTTON_HEIGHT * 3 + PADDING * 4;

const BACKGROUND_PANEL_WIDTH: i32 = 250;
const BACKGROUND_PANEL_HEIGHT: i32 = BUTTON_HEIGHT * 3;

const ROTATE_PANEL_WIDTH: i32 = SLIDER_WIDTH + PADDING * 2;
const ROTATE_PANEL_HEIGHT: i32 = PADDING * 2 + SLIDER_HEIGHT;

static EFFECTS_PNG_NAME: &'static str = "demo-effects";
static OPEN_PNG_NAME: &'static str = "demo-open";
static ROTATE_PNG_NAME: &'static str = "demo-rotate";
static ZOOM_IN_PNG_NAME: &'static str = "demo-zoom-in";
static ZOOM_OUT_PNG_NAME: &'static str = "demo-zoom-out";
static BACKGROUND_PNG_NAME: &'static str = "demo-background";
static SCREENSHOT_PNG_NAME: &'static str = "demo-screenshot";

pub struct DemoUI<D>
where
    D: Device,
{
    effects_texture: D::Texture,
    open_texture: D::Texture,
    rotate_texture: D::Texture,
    zoom_in_texture: D::Texture,
    zoom_out_texture: D::Texture,
    background_texture: D::Texture,
    screenshot_texture: D::Texture,

    effects_panel_visible: bool,
    background_panel_visible: bool,
    rotate_panel_visible: bool,

    // FIXME(pcwalton): Factor the below out into a model class.
    pub mode: Mode,
    pub background_color: BackgroundColor,
    pub gamma_correction_effect_enabled: bool,
    pub stem_darkening_effect_enabled: bool,
    pub subpixel_aa_effect_enabled: bool,
    pub rotation: i32,
    pub message: String,
    pub show_text_effects: bool,
}

impl<D> DemoUI<D>
where
    D: Device,
{
    pub fn new(device: &D, resources: &dyn ResourceLoader, options: Options) -> DemoUI<D> {
        let effects_texture = device.create_texture_from_png(resources, EFFECTS_PNG_NAME);
        let open_texture = device.create_texture_from_png(resources, OPEN_PNG_NAME);
        let rotate_texture = device.create_texture_from_png(resources, ROTATE_PNG_NAME);
        let zoom_in_texture = device.create_texture_from_png(resources, ZOOM_IN_PNG_NAME);
        let zoom_out_texture = device.create_texture_from_png(resources, ZOOM_OUT_PNG_NAME);
        let background_texture = device.create_texture_from_png(resources, BACKGROUND_PNG_NAME);
        let screenshot_texture = device.create_texture_from_png(resources, SCREENSHOT_PNG_NAME);

        DemoUI {
            effects_texture,
            open_texture,
            rotate_texture,
            zoom_in_texture,
            zoom_out_texture,
            background_texture,
            screenshot_texture,

            effects_panel_visible: false,
            background_panel_visible: false,
            rotate_panel_visible: false,

            mode: options.mode,
            background_color: options.background_color,
            gamma_correction_effect_enabled: false,
            stem_darkening_effect_enabled: false,
            subpixel_aa_effect_enabled: false,
            rotation: SLIDER_WIDTH / 2,
            message: String::new(),
            show_text_effects: true,
        }
    }

    fn rotation(&self) -> f32 {
        (self.rotation as f32 / SLIDER_WIDTH as f32 * 2.0 - 1.0) * PI
    }

    pub fn update<W>(
        &mut self,
        device: &D,
        window: &mut W,
        debug_ui: &mut DebugUI<D>,
        action: &mut UIAction,
    ) where
        W: Window,
    {
        // Draw message text.

        self.draw_message_text(device, debug_ui);

        // Draw button strip.

        let bottom = debug_ui.ui.framebuffer_size().y() - PADDING;
        let mut position = Point2DI32::new(PADDING, bottom - BUTTON_HEIGHT);

        let button_size = Point2DI32::new(BUTTON_WIDTH, BUTTON_HEIGHT);

        // Draw text effects button.
        if self.show_text_effects {
            if debug_ui
                .ui
                .draw_button(device, position, &self.effects_texture)
            {
                self.effects_panel_visible = !self.effects_panel_visible;
            }
            if !self.effects_panel_visible {
                debug_ui.ui.draw_tooltip(
                    device,
                    "Text Effects",
                    RectI32::new(position, button_size),
                );
            }
            position += Point2DI32::new(button_size.x() + PADDING, 0);
        }

        // Draw open button.
        if debug_ui
            .ui
            .draw_button(device, position, &self.open_texture)
        {
            // FIXME(pcwalton): This is not sufficient for Android, where we will need to take in
            // the contents of the file.
            window.present_open_svg_dialog();
        }
        debug_ui
            .ui
            .draw_tooltip(device, "Open SVG", RectI32::new(position, button_size));
        position += Point2DI32::new(BUTTON_WIDTH + PADDING, 0);

        // Draw screenshot button.
        if debug_ui
            .ui
            .draw_button(device, position, &self.screenshot_texture)
        {
            // FIXME(pcwalton): This is not sufficient for Android, where we will need to take in
            // the contents of the file.
            if let Ok(file) = window.run_save_dialog("png") {
                *action = UIAction::TakeScreenshot(file);
            }
        }
        debug_ui.ui.draw_tooltip(
            device,
            "Take Screenshot",
            RectI32::new(position, button_size),
        );
        position += Point2DI32::new(BUTTON_WIDTH + PADDING, 0);

        // Draw mode switch.
        let new_mode =
            debug_ui
                .ui
                .draw_text_switch(device, position, &["2D", "3D", "VR"], self.mode as u8);
        if new_mode != self.mode as u8 {
            self.mode = match new_mode {
                0 => Mode::TwoD,
                1 => Mode::ThreeD,
                _ => Mode::VR,
            };
            *action = UIAction::ModelChanged;
        }

        let mode_switch_width = debug_ui.ui.measure_switch(3);
        let mode_switch_size = Point2DI32::new(mode_switch_width, BUTTON_HEIGHT);
        debug_ui.ui.draw_tooltip(
            device,
            "2D/3D/VR Mode",
            RectI32::new(position, mode_switch_size),
        );
        position += Point2DI32::new(mode_switch_width + PADDING, 0);

        // Draw background switch.
        if debug_ui
            .ui
            .draw_button(device, position, &self.background_texture)
        {
            self.background_panel_visible = !self.background_panel_visible;
        }
        if !self.background_panel_visible {
            debug_ui.ui.draw_tooltip(
                device,
                "Background Color",
                RectI32::new(position, button_size),
            );
        }

        // Draw background panel, if necessary.
        self.draw_background_panel(device, debug_ui, position.x(), action);
        position += Point2DI32::new(button_size.x() + PADDING, 0);

        // Draw effects panel, if necessary.
        self.draw_effects_panel(device, debug_ui);

        // Draw rotate and zoom buttons, if applicable.
        if self.mode != Mode::TwoD {
            return;
        }

        if debug_ui
            .ui
            .draw_button(device, position, &self.rotate_texture)
        {
            self.rotate_panel_visible = !self.rotate_panel_visible;
        }
        if !self.rotate_panel_visible {
            debug_ui
                .ui
                .draw_tooltip(device, "Rotate", RectI32::new(position, button_size));
        }
        self.draw_rotate_panel(device, debug_ui, position.x(), action);
        position += Point2DI32::new(BUTTON_WIDTH + PADDING, 0);

        if debug_ui
            .ui
            .draw_button(device, position, &self.zoom_in_texture)
        {
            *action = UIAction::ZoomIn;
        }
        debug_ui
            .ui
            .draw_tooltip(device, "Zoom In", RectI32::new(position, button_size));
        position += Point2DI32::new(BUTTON_WIDTH + PADDING, 0);

        if debug_ui
            .ui
            .draw_button(device, position, &self.zoom_out_texture)
        {
            *action = UIAction::ZoomOut;
        }
        debug_ui
            .ui
            .draw_tooltip(device, "Zoom Out", RectI32::new(position, button_size));
        position += Point2DI32::new(BUTTON_WIDTH + PADDING, 0);
    }

    fn draw_message_text(&mut self, device: &D, debug_ui: &mut DebugUI<D>) {
        if self.message.is_empty() {
            return;
        }

        let message_size = debug_ui.ui.measure_text(&self.message);
        let window_origin = Point2DI32::new(PADDING, PADDING);
        let window_size = Point2DI32::new(PADDING * 2 + message_size, TOOLTIP_HEIGHT);
        debug_ui.ui.draw_solid_rounded_rect(
            device,
            RectI32::new(window_origin, window_size),
            WINDOW_COLOR,
        );
        debug_ui.ui.draw_text(
            device,
            &self.message,
            window_origin + Point2DI32::new(PADDING, PADDING + FONT_ASCENT),
            false,
        );
    }

    fn draw_effects_panel(&mut self, device: &D, debug_ui: &mut DebugUI<D>) {
        if !self.effects_panel_visible {
            return;
        }

        let bottom = debug_ui.ui.framebuffer_size().y() - PADDING;
        let effects_panel_y = bottom - (BUTTON_HEIGHT + PADDING + EFFECTS_PANEL_HEIGHT);
        debug_ui.ui.draw_solid_rounded_rect(
            device,
            RectI32::new(
                Point2DI32::new(PADDING, effects_panel_y),
                Point2DI32::new(EFFECTS_PANEL_WIDTH, EFFECTS_PANEL_HEIGHT),
            ),
            WINDOW_COLOR,
        );

        self.gamma_correction_effect_enabled = self.draw_effects_switch(
            device,
            debug_ui,
            "Gamma Correction",
            0,
            effects_panel_y,
            self.gamma_correction_effect_enabled,
        );
        self.stem_darkening_effect_enabled = self.draw_effects_switch(
            device,
            debug_ui,
            "Stem Darkening",
            1,
            effects_panel_y,
            self.stem_darkening_effect_enabled,
        );
        self.subpixel_aa_effect_enabled = self.draw_effects_switch(
            device,
            debug_ui,
            "Subpixel AA",
            2,
            effects_panel_y,
            self.subpixel_aa_effect_enabled,
        );
    }

    fn draw_background_panel(
        &mut self,
        device: &D,
        debug_ui: &mut DebugUI<D>,
        panel_x: i32,
        action: &mut UIAction,
    ) {
        if !self.background_panel_visible {
            return;
        }

        let bottom = debug_ui.ui.framebuffer_size().y() - PADDING;
        let panel_y = bottom - (BUTTON_HEIGHT + PADDING + BACKGROUND_PANEL_HEIGHT);
        let panel_position = Point2DI32::new(panel_x, panel_y);
        debug_ui.ui.draw_solid_rounded_rect(
            device,
            RectI32::new(
                panel_position,
                Point2DI32::new(BACKGROUND_PANEL_WIDTH, BACKGROUND_PANEL_HEIGHT),
            ),
            WINDOW_COLOR,
        );

        self.draw_background_menu_item(
            device,
            debug_ui,
            BackgroundColor::Light,
            panel_position,
            action,
        );
        self.draw_background_menu_item(
            device,
            debug_ui,
            BackgroundColor::Dark,
            panel_position,
            action,
        );
        self.draw_background_menu_item(
            device,
            debug_ui,
            BackgroundColor::Transparent,
            panel_position,
            action,
        );
    }

    fn draw_rotate_panel(
        &mut self,
        device: &D,
        debug_ui: &mut DebugUI<D>,
        rotate_panel_x: i32,
        action: &mut UIAction,
    ) {
        if !self.rotate_panel_visible {
            return;
        }

        let bottom = debug_ui.ui.framebuffer_size().y() - PADDING;
        let rotate_panel_y = bottom - (BUTTON_HEIGHT + PADDING + ROTATE_PANEL_HEIGHT);
        let rotate_panel_origin = Point2DI32::new(rotate_panel_x, rotate_panel_y);
        let rotate_panel_size = Point2DI32::new(ROTATE_PANEL_WIDTH, ROTATE_PANEL_HEIGHT);
        debug_ui.ui.draw_solid_rounded_rect(
            device,
            RectI32::new(rotate_panel_origin, rotate_panel_size),
            WINDOW_COLOR,
        );

        let (widget_x, widget_y) = (rotate_panel_x + PADDING, rotate_panel_y + PADDING);
        let widget_rect = RectI32::new(
            Point2DI32::new(widget_x, widget_y),
            Point2DI32::new(SLIDER_WIDTH, SLIDER_KNOB_HEIGHT),
        );
        if let Some(position) = debug_ui
            .ui
            .event_queue
            .handle_mouse_down_or_dragged_in_rect(widget_rect)
        {
            self.rotation = position.x();
            *action = UIAction::Rotate(self.rotation());
        }

        let slider_track_y =
            rotate_panel_y + PADDING + SLIDER_KNOB_HEIGHT / 2 - SLIDER_TRACK_HEIGHT / 2;
        let slider_track_rect = RectI32::new(
            Point2DI32::new(widget_x, slider_track_y),
            Point2DI32::new(SLIDER_WIDTH, SLIDER_TRACK_HEIGHT),
        );
        debug_ui
            .ui
            .draw_rect_outline(device, slider_track_rect, TEXT_COLOR);

        let slider_knob_x = widget_x + self.rotation - SLIDER_KNOB_WIDTH / 2;
        let slider_knob_rect = RectI32::new(
            Point2DI32::new(slider_knob_x, widget_y),
            Point2DI32::new(SLIDER_KNOB_WIDTH, SLIDER_KNOB_HEIGHT),
        );
        debug_ui
            .ui
            .draw_solid_rect(device, slider_knob_rect, TEXT_COLOR);
    }

    fn draw_background_menu_item(
        &mut self,
        device: &D,
        debug_ui: &mut DebugUI<D>,
        color: BackgroundColor,
        panel_position: Point2DI32,
        action: &mut UIAction,
    ) {
        let (text, index) = (color.as_str(), color as i32);

        let widget_size = Point2DI32::new(BACKGROUND_PANEL_WIDTH, BUTTON_HEIGHT);
        let widget_origin = panel_position + Point2DI32::new(0, widget_size.y() * index);
        let widget_rect = RectI32::new(widget_origin, widget_size);

        if color == self.background_color {
            debug_ui
                .ui
                .draw_solid_rounded_rect(device, widget_rect, TEXT_COLOR);
        }

        let (text_x, text_y) = (PADDING * 2, BUTTON_TEXT_OFFSET);
        let text_position = widget_origin + Point2DI32::new(text_x, text_y);
        debug_ui
            .ui
            .draw_text(device, text, text_position, color == self.background_color);

        if let Some(_) = debug_ui
            .ui
            .event_queue
            .handle_mouse_down_in_rect(widget_rect)
        {
            self.background_color = color;
            *action = UIAction::ModelChanged;
        }
    }

    fn draw_effects_switch(
        &self,
        device: &D,
        debug_ui: &mut DebugUI<D>,
        text: &str,
        index: i32,
        window_y: i32,
        value: bool,
    ) -> bool {
        let text_x = PADDING * 2;
        let text_y = window_y + PADDING + BUTTON_TEXT_OFFSET + (BUTTON_HEIGHT + PADDING) * index;
        debug_ui
            .ui
            .draw_text(device, text, Point2DI32::new(text_x, text_y), false);

        let switch_width = debug_ui.ui.measure_switch(2);
        let switch_x = PADDING + EFFECTS_PANEL_WIDTH - (switch_width + PADDING);
        let switch_y = window_y + PADDING + (BUTTON_HEIGHT + PADDING) * index;
        let switch_position = Point2DI32::new(switch_x, switch_y);
        debug_ui
            .ui
            .draw_text_switch(device, switch_position, &["Off", "On"], value as u8)
            != 0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum UIAction {
    None,
    ModelChanged,
    TakeScreenshot(PathBuf),
    ZoomIn,
    ZoomOut,
    Rotate(f32),
}
