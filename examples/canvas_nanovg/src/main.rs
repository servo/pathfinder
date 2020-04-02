// pathfinder/examples/canvas_nanovg/src/main.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use arrayvec::ArrayVec;
use font_kit::handle::Handle;
use font_kit::sources::mem::MemSource;
use image;
use pathfinder_canvas::{CanvasFontContext, CanvasRenderingContext2D, LineJoin, Path2D};
use pathfinder_canvas::{TextAlign, TextBaseline};
use pathfinder_color::{ColorF, ColorU, rgbau, rgbf, rgbu};
use pathfinder_content::fill::FillRule;
use pathfinder_content::gradient::Gradient;
use pathfinder_content::outline::ArcDirection;
use pathfinder_content::pattern::{Image, Pattern, PatternFlags, PatternSource};
use pathfinder_content::stroke::LineCap;
use pathfinder_geometry::angle;
use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::util;
use pathfinder_geometry::vector::{Vector2F, vec2f, vec2i};
use pathfinder_gl::{GLDevice, GLVersion};
use pathfinder_renderer::concurrent::rayon::RayonExecutor;
use pathfinder_renderer::concurrent::scene_proxy::SceneProxy;
use pathfinder_renderer::gpu::options::{DestFramebuffer, RendererOptions};
use pathfinder_renderer::gpu::renderer::Renderer;
use pathfinder_renderer::options::BuildOptions;
use pathfinder_resources::ResourceLoader;
use pathfinder_resources::fs::FilesystemResourceLoader;
use pathfinder_simd::default::F32x2;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::video::GLProfile;
use std::f32::consts::PI;
use std::sync::Arc;
use std::time::Instant;

// TODO(pcwalton): See if we can reduce the amount of code by using the canvas shadow feature.

const PI_2: f32 = PI * 2.0;
const FRAC_PI_2_3: f32 = PI * 2.0 / 3.0;

const WINDOW_WIDTH: i32 = 1024;
const WINDOW_HEIGHT: i32 = 768;

static FONT_NAME_REGULAR: &'static str = "Roboto-Regular";
static FONT_NAME_BOLD:    &'static str = "Roboto-Bold";
static FONT_NAME_EMOJI:   &'static str = "NotoEmoji";

static PARAGRAPH_TEXT: &'static str = "This is a longer chunk of text.

I would have used lorem ipsum, but she was busy jumping over the lazy dog with the fox and all \
the men who came to the aid of the party. 🎉";

fn render_demo(canvas: &mut CanvasRenderingContext2D,
               mouse_position: Vector2F,
               window_size: Vector2F,
               time: f32,
               data: &DemoData) {
    draw_eyes(canvas,
              RectF::new(vec2f(window_size.x() - 250.0, 50.0), vec2f(150.0, 100.0)),
              mouse_position,
              time);
    draw_paragraph(canvas, RectF::new(vec2f(window_size.x() - 450.0, 50.0), vec2f(150.0, 100.0)));
    draw_graph(canvas,
               RectF::new(window_size * vec2f(0.0, 0.5), window_size * vec2f(1.0, 0.5)),
               time);
    draw_color_wheel(canvas,
                     RectF::new(window_size - vec2f(300.0, 300.0), vec2f(250.0, 250.0)),
                     time);
    draw_lines(canvas, RectF::new(vec2f(120.0, window_size.y() - 50.0), vec2f(600.0, 50.0)), time);
    draw_caps(canvas, RectF::new(vec2f(10.0, 300.0), vec2f(30.0, 40.0)));
    draw_clip(canvas, vec2f(50.0, window_size.y() - 80.0), time);

    canvas.save();

    // Draw widgets.
    draw_window(canvas, "Widgets & Stuff", RectF::new(vec2f(50.0, 50.0), vec2f(300.0, 400.0)));
    let mut position = vec2f(60.0, 95.0);
    draw_search_box(canvas, "Search", RectF::new(position, vec2f(280.0, 25.0)));
    position += vec2f(0.0, 40.0);
    draw_dropdown(canvas, "Effects", RectF::new(position, vec2f(280.0, 28.0)));
    let popup_position = position + vec2f(0.0, 14.0);
    position += vec2f(0.0, 45.0);

    // Draw login form.
    draw_label(canvas, "Login", RectF::new(position, vec2f(280.0, 20.0)));
    position += vec2f(0.0, 25.0);
    draw_text_edit_box(canvas, "E-mail address", RectF::new(position, vec2f(280.0, 28.0)));
    position += vec2f(0.0, 35.0);
    draw_text_edit_box(canvas, "Password", RectF::new(position, vec2f(280.0, 28.0)));
    position += vec2f(0.0, 38.0);
    draw_check_box(canvas, "Remember me", RectF::new(position, vec2f(140.0, 28.0)));
    draw_button(canvas,
                Some("🚪"),
                "Sign In",
                RectF::new(position + vec2f(138.0, 0.0), vec2f(140.0, 28.0)),
                rgbu(0, 96, 128));
    position += vec2f(0.0, 45.0);

    // Draw slider form.
    draw_label(canvas, "Diameter", RectF::new(position, vec2f(280.0, 20.0)));
    position += vec2f(0.0, 25.0);
    draw_numeric_edit_box(canvas, "123.00", "px", RectF::new(position + vec2f(180.0, 0.0),
                                                             vec2f(100.0, 28.0)));
    draw_slider(canvas, 0.4, RectF::new(position, vec2f(170.0, 28.0)));
    position += vec2f(0.0, 55.0);

    // Draw dialog box buttons.
    draw_button(canvas,
                Some("️❌"),
                "Delete",
                RectF::new(position, vec2f(160.0, 28.0)),
                rgbu(128, 16, 8));
    draw_button(canvas,
                None,
                "Cancel",
                RectF::new(position + vec2f(170.0, 0.0), vec2f(110.0, 28.0)),
                rgbau(0, 0, 0, 0));

    // Draw thumbnails.
    draw_thumbnails(canvas,
                    RectF::new(vec2f(365.0, popup_position.y() - 30.0), vec2f(160.0, 300.0)),
                    time,
                    12,
                    &data.image);

    canvas.restore();
}

fn draw_eyes(canvas: &mut CanvasRenderingContext2D,
             rect: RectF,
             mouse_position: Vector2F,
             time: f32) {
    let eyes_radii = rect.size() * vec2f(0.23, 0.5);
    let eyes_left_position = rect.origin() + eyes_radii;
    let eyes_right_position = rect.origin() + vec2f(rect.width() - eyes_radii.x(), eyes_radii.y());
    let eyes_center = f32::min(eyes_radii.x(), eyes_radii.y()) * 0.5;
    let blink = 1.0 - f32::powf(f32::sin(time * 0.5), 200.0) * 0.8;

    let mut gradient = Gradient::linear(
        LineSegment2F::new(vec2f(0.0, rect.height() * 0.5),
                           rect.size() * vec2f(0.1, 1.0)) + rect.origin());
    gradient.add_color_stop(rgbau(0, 0, 0, 32), 0.0);
    gradient.add_color_stop(rgbau(0, 0, 0, 16), 1.0);
    let mut path = Path2D::new();
    path.ellipse(eyes_left_position  + vec2f(3.0, 16.0), eyes_radii, 0.0, 0.0, PI_2);
    path.ellipse(eyes_right_position + vec2f(3.0, 16.0), eyes_radii, 0.0, 0.0, PI_2);
    canvas.set_fill_style(gradient);
    canvas.fill_path(path, FillRule::Winding);

    let mut gradient =
        Gradient::linear(LineSegment2F::new(vec2f(0.0, rect.height() * 0.25),
                                            rect.size() * vec2f(0.1, 1.0)) + rect.origin());
    gradient.add_color_stop(rgbu(220, 220, 220), 0.0);
    gradient.add_color_stop(rgbu(128, 128, 128), 1.0);
    let mut path = Path2D::new();
    path.ellipse(eyes_left_position, eyes_radii, 0.0, 0.0, PI_2);
    path.ellipse(eyes_right_position, eyes_radii, 0.0, 0.0, PI_2);
    canvas.set_fill_style(gradient);
    canvas.fill_path(path, FillRule::Winding);

    let mut delta = (mouse_position - eyes_right_position) / (eyes_radii * 10.0);
    let distance = delta.length();
    if distance > 1.0 {
        delta *= 1.0 / distance;
    }
    delta *= eyes_radii * vec2f(0.4, 0.5);
    let mut path = Path2D::new();
    path.ellipse(eyes_left_position + delta + vec2f(0.0, eyes_radii.y() * 0.25 * (1.0 - blink)),
                 vec2f(eyes_center, eyes_center * blink),
                 0.0,
                 0.0,
                 PI_2);
    path.ellipse(eyes_right_position + delta + vec2f(0.0, eyes_radii.y() * 0.25 * (1.0 - blink)),
                 vec2f(eyes_center, eyes_center * blink),
                 0.0,
                 0.0,
                 PI_2);
    canvas.set_fill_style(rgbu(32, 32, 32));
    canvas.fill_path(path, FillRule::Winding);

    let gloss_position = eyes_left_position - eyes_radii * vec2f(0.25, 0.5);
    let gloss_radii = F32x2::new(0.1, 0.75) * F32x2::splat(eyes_radii.x());
    let mut gloss = Gradient::radial(gloss_position, gloss_radii);
    gloss.add_color_stop(rgbau(255, 255, 255, 128), 0.0);
    gloss.add_color_stop(rgbau(255, 255, 255, 0), 1.0);
    canvas.set_fill_style(gloss);
    let mut path = Path2D::new();
    path.ellipse(eyes_left_position, eyes_radii, 0.0, 0.0, PI_2);
    canvas.fill_path(path, FillRule::Winding);

    let gloss_position = eyes_right_position - eyes_radii * vec2f(0.25, 0.5);
    let mut gloss = Gradient::radial(gloss_position, gloss_radii);
    gloss.add_color_stop(rgbau(255, 255, 255, 128), 0.0);
    gloss.add_color_stop(rgbau(255, 255, 255, 0), 1.0);
    canvas.set_fill_style(gloss);
    let mut path = Path2D::new();
    path.ellipse(eyes_right_position, eyes_radii, 0.0, 0.0, PI_2);
    canvas.fill_path(path, FillRule::Winding);
}

// This is nowhere near correct line layout, but it suffices to more or less match what NanoVG
// does.
fn draw_paragraph(canvas: &mut CanvasRenderingContext2D, rect: RectF) {
    const LINE_HEIGHT: f32 = 24.0;

    canvas.save();

    canvas.set_font(&[FONT_NAME_REGULAR, FONT_NAME_EMOJI][..]);
    canvas.set_font_size(18.0);

    let mut cursor = rect.origin();
    next_line(canvas, &mut cursor, rect);

    let space_width = canvas.measure_text("A B").width - canvas.measure_text("AB").width;

    for space_separated in PARAGRAPH_TEXT.split(' ') {
        let mut first = true;
        for word in space_separated.split('\n') {
            if !first {
                next_line(canvas, &mut cursor, rect);
            }
            first = false;

            let word_width = canvas.measure_text(word).width;
            if cursor.x() + space_width + word_width > rect.max_x() {
                next_line(canvas, &mut cursor, rect);
            } else if cursor.x() > rect.min_x() {
                cursor = cursor + vec2f(space_width, 0.0);
            }

            canvas.set_fill_style(ColorU::white());
            canvas.fill_text(word, cursor);

            cursor = cursor + vec2f(word_width, 0.0);
        }
    }

    canvas.restore();

    fn next_line(canvas: &mut CanvasRenderingContext2D, cursor: &mut Vector2F, rect: RectF) {
        cursor.set_x(rect.min_x());

        canvas.set_fill_style(rgbau(255, 255, 255, 16));
        canvas.fill_rect(RectF::new(*cursor, vec2f(rect.width(), LINE_HEIGHT)));

        *cursor += vec2f(0.0, LINE_HEIGHT);
    }
}

fn draw_graph(canvas: &mut CanvasRenderingContext2D, rect: RectF, time: f32) {
    let sample_spread = rect.width() / 5.0;

    let samples = [
        (1.0 + f32::sin(time * 1.2345  + f32::cos(time * 0.33457) * 0.44)) * 0.5,
        (1.0 + f32::sin(time * 0.68363 + f32::cos(time * 1.30)    * 1.55)) * 0.5,
        (1.0 + f32::sin(time * 1.1642  + f32::cos(time * 0.33457) * 1.24)) * 0.5,
        (1.0 + f32::sin(time * 0.56345 + f32::cos(time * 1.63)    * 0.14)) * 0.5,
        (1.0 + f32::sin(time * 1.6245  + f32::cos(time * 0.254)   * 0.3))  * 0.5,
        (1.0 + f32::sin(time * 0.345   + f32::cos(time * 0.03)    * 0.6))  * 0.5,
    ];

    let sample_scale = vec2f(sample_spread, rect.height() * 0.8);
    let sample_points: ArrayVec<[Vector2F; 6]> = samples.iter()
                                                        .enumerate()
                                                        .map(|(index, &sample)| {
        rect.origin() + vec2f(index as f32, sample) * sample_scale
    }).collect();

    // Draw graph background.
    let mut background = Gradient::linear(
        LineSegment2F::new(vec2f(0.0, 0.0), vec2f(0.0, rect.height())) + rect.origin());
    background.add_color_stop(rgbau(0, 160, 192, 0),  0.0);
    background.add_color_stop(rgbau(0, 160, 192, 64), 1.0);
    canvas.set_fill_style(background);
    let mut path = create_graph_path(&sample_points, sample_spread, Vector2F::zero());
    path.line_to(rect.lower_right());
    path.line_to(rect.lower_left());
    canvas.fill_path(path, FillRule::Winding);

    // Draw graph line shadow.
    canvas.set_stroke_style(rgbau(0, 0, 0, 32));
    canvas.set_line_width(3.0);
    let path = create_graph_path(&sample_points, sample_spread, vec2f(0.0, 2.0));
    canvas.stroke_path(path);

    // Draw graph line.
    canvas.set_stroke_style(rgbu(0, 160, 192));
    canvas.set_line_width(3.0);
    let path = create_graph_path(&sample_points, sample_spread, Vector2F::zero());
    canvas.stroke_path(path);

    // Draw sample position highlights.
    for &sample_point in &sample_points {
        let gradient_center = sample_point + vec2f(0.0, 2.0);
        let mut background = Gradient::radial(gradient_center, F32x2::new(3.0, 8.0));
        background.add_color_stop(rgbau(0, 0, 0, 32), 0.0);
        background.add_color_stop(rgbau(0, 0, 0, 0),  1.0);
        canvas.set_fill_style(background);
        canvas.fill_rect(RectF::new(sample_point + vec2f(-10.0, -10.0 + 2.0), vec2f(20.0, 20.0)));
    }

    // Draw sample positions.
    canvas.set_fill_style(rgbu(0, 160, 192));
    let mut path = Path2D::new();
    for &sample_point in &sample_points {
        path.ellipse(sample_point, vec2f(4.0, 4.0), 0.0, 0.0, PI_2);
    }
    canvas.fill_path(path, FillRule::Winding);
    canvas.set_fill_style(rgbu(220, 220, 220));
    let mut path = Path2D::new();
    for &sample_point in &sample_points {
        path.ellipse(sample_point, vec2f(2.0, 2.0), 0.0, 0.0, PI_2);
    }
    canvas.fill_path(path, FillRule::Winding);

    // Reset state.
    canvas.set_line_width(1.0);
}

fn draw_color_wheel(canvas: &mut CanvasRenderingContext2D, rect: RectF, time: f32) {
    let hue = time * 0.12;

    canvas.save();

    let center = rect.center();
    let outer_radius = f32::min(rect.width(), rect.height()) * 0.5 - 5.0;
    let inner_radius = outer_radius - 20.0;

    // Half a pixel arc length in radians.
    let half_arc_len = 0.5 / outer_radius;

    // Draw outer circle.
    for segment in 0..6 {
        let start_angle = segment       as f32 / 6.0 * PI_2 - half_arc_len;
        let end_angle   = (segment + 1) as f32 / 6.0 * PI_2 + half_arc_len;
        let line = LineSegment2F::new(vec2f(f32::cos(start_angle), f32::sin(start_angle)),
                                      vec2f(f32::cos(end_angle),   f32::sin(end_angle)));
        let scale = util::lerp(inner_radius, outer_radius, 0.5);
        let mut gradient = Gradient::linear(line * scale + center);
        let start_color = ColorF::from_hsl(start_angle, 1.0, 0.55).to_u8();
        let end_color   = ColorF::from_hsl(end_angle,   1.0, 0.55).to_u8();
        gradient.add_color_stop(start_color, 0.0);
        gradient.add_color_stop(end_color,   1.0);
        canvas.set_fill_style(gradient);
        let mut path = Path2D::new();
        path.arc(center, inner_radius, start_angle, end_angle,   ArcDirection::CW);
        path.arc(center, outer_radius, end_angle,   start_angle, ArcDirection::CCW);
        path.close_path();
        canvas.fill_path(path, FillRule::Winding);
    }

    // Stroke outer circle.
    canvas.set_stroke_style(rgbau(0, 0, 0, 64));
    canvas.set_line_width(1.0);
    let mut path = Path2D::new();
    path.ellipse(center, inner_radius - 0.5, 0.0, 0.0, PI_2);
    path.ellipse(center, outer_radius + 0.5, 0.0, 0.0, PI_2);
    canvas.stroke_path(path);

    // Prepare to draw the selector.
    canvas.save();
    let original_transform = canvas.current_transform();
    canvas.set_current_transform(&(original_transform *
                                   Transform2F::from_translation(center) *
                                   Transform2F::from_rotation(hue)));

    canvas.set_stroke_style(rgbau(255, 255, 255, 192));
    canvas.set_line_width(2.0);
    canvas.stroke_rect(RectF::new(vec2f(inner_radius - 1.0, -3.0),
                                  vec2f(outer_radius - inner_radius + 2.0, 6.0)));

    // TODO(pcwalton): Marker fill with box gradient

    // Draw center triangle.
    let triangle_radius = inner_radius - 6.0;
    let triangle_vertex_a = vec2f(triangle_radius, 0.0);
    let triangle_vertex_b = vec2f(FRAC_PI_2_3.cos(), FRAC_PI_2_3.sin()) * triangle_radius;
    let triangle_vertex_c = vec2f((-FRAC_PI_2_3).cos(), (-FRAC_PI_2_3).sin()) * triangle_radius;
    let mut gradient_0 = Gradient::linear_from_points(triangle_vertex_a, triangle_vertex_b);
    gradient_0.add_color_stop(ColorF::from_hsl(hue, 1.0, 0.5).to_u8(), 0.0);
    gradient_0.add_color_stop(ColorU::white(), 1.0);
    let mut gradient_1 =
        Gradient::linear_from_points(triangle_vertex_a.lerp(triangle_vertex_b, 0.5),
                                     triangle_vertex_c);
    gradient_1.add_color_stop(ColorU::transparent_black(), 0.0);
    gradient_1.add_color_stop(ColorU::black(),             1.0);
    let mut path = Path2D::new();
    path.move_to(triangle_vertex_a);
    path.line_to(triangle_vertex_b);
    path.line_to(triangle_vertex_c);
    path.close_path();
    canvas.set_fill_style(gradient_0);
    canvas.fill_path(path.clone(), FillRule::Winding);
    canvas.set_fill_style(gradient_1);
    canvas.fill_path(path.clone(), FillRule::Winding);
    canvas.set_stroke_style(rgbau(0, 0, 0, 64));
    canvas.stroke_path(path);

    // Stroke the selection circle on the triangle.
    let selection_circle_center = vec2f(FRAC_PI_2_3.cos(), FRAC_PI_2_3.sin()) * triangle_radius *
        vec2f(0.3, 0.4);
    canvas.set_stroke_style(rgbau(255, 255, 255, 192));
    canvas.set_line_width(2.0);
    let mut path = Path2D::new();
    path.ellipse(selection_circle_center, vec2f(5.0, 5.0), 0.0, 0.0, PI_2);
    canvas.stroke_path(path);

    // Fill the selection circle.
    let mut gradient = Gradient::radial(selection_circle_center, F32x2::new(7.0, 9.0));
    gradient.add_color_stop(rgbau(0, 0, 0, 64), 0.0);
    gradient.add_color_stop(rgbau(0, 0, 0, 0),  1.0);
    canvas.set_fill_style(gradient);
    let mut path = Path2D::new();
    path.rect(RectF::new(selection_circle_center - vec2f(20.0, 20.0), vec2f(40.0, 40.0)));
    path.ellipse(selection_circle_center, vec2f(7.0, 7.0), 0.0, 0.0, PI_2);
    canvas.fill_path(path, FillRule::EvenOdd);

    canvas.restore();
    canvas.restore();
}

fn draw_lines(canvas: &mut CanvasRenderingContext2D, rect: RectF, time: f32) {
    const PADDING: f32 = 5.0;

    let spacing = rect.width() / 9.0 - PADDING * 2.0;

    canvas.save();

    let points = [
        vec2f(-spacing * 0.25 + f32::cos(time * 0.3)  * spacing * 0.5,
              f32::sin(time * 0.3)  * spacing * 0.5),
        vec2f(-spacing * 0.25, 0.0),
        vec2f( spacing * 0.25, 0.0),
        vec2f( spacing * 0.25 + f32::cos(time * -0.3) * spacing * 0.5,
              f32::sin(time * -0.3) * spacing * 0.5),
    ];

    for (cap_index, &cap) in [LineCap::Butt, LineCap::Round, LineCap::Square].iter().enumerate() {
        for (join_index, &join) in [
            LineJoin::Miter, LineJoin::Round, LineJoin::Bevel
        ].iter().enumerate() {
            let origin = rect.origin() +
                vec2f(0.5, -0.5) * spacing +
                vec2f((cap_index * 3 + join_index) as f32 / 9.0 * rect.width(), 0.0) +
                PADDING;

            canvas.set_line_cap(cap);
            canvas.set_line_join(join);
            canvas.set_line_width(spacing * 0.3);
            canvas.set_stroke_style(rgbau(0, 0, 0, 160));

            let mut path = Path2D::new();
            path.move_to(points[0] + origin);
            path.line_to(points[1] + origin);
            path.line_to(points[2] + origin);
            path.line_to(points[3] + origin);
            canvas.stroke_path(path.clone());

            canvas.set_line_cap(LineCap::Butt);
            canvas.set_line_join(LineJoin::Bevel);
            canvas.set_line_width(1.0);
            canvas.set_stroke_style(rgbu(0, 192, 255));

            canvas.stroke_path(path);
        }
    }

    canvas.restore();
}

fn draw_caps(canvas: &mut CanvasRenderingContext2D, rect: RectF) {
    const LINE_WIDTH: f32 = 8.0;

    canvas.save();

    canvas.set_fill_style(rgbau(255, 255, 255, 32));
    canvas.fill_rect(rect.dilate(vec2f(LINE_WIDTH / 2.0, 0.0)));
    canvas.fill_rect(rect);

    canvas.set_line_width(LINE_WIDTH);
    for (cap_index, &cap) in [LineCap::Butt, LineCap::Round, LineCap::Square].iter().enumerate() {
        canvas.set_line_cap(cap);
        canvas.set_stroke_style(ColorU::black());
        let offset = cap_index as f32 * 10.0 + 5.0;
        let mut path = Path2D::new();
        path.move_to(rect.origin()      + vec2f(0.0, offset));
        path.line_to(rect.upper_right() + vec2f(0.0, offset));
        canvas.stroke_path(path);
    }

    canvas.restore();
}

fn draw_clip(canvas: &mut CanvasRenderingContext2D, origin: Vector2F, time: f32) {
    canvas.save();

    // Draw first rect.
    let original_transform = canvas.current_transform();
    let transform_a = original_transform *
        Transform2F::from_translation(origin) *
        Transform2F::from_rotation(angle::angle_from_degrees(5.0));
    canvas.set_current_transform(&transform_a);
    canvas.set_fill_style(rgbu(255, 0, 0));
    let mut clip_path = Path2D::new();
    clip_path.rect(RectF::new(vec2f(-20.0, -20.0), vec2f(60.0, 40.0)));
    canvas.fill_path(clip_path.clone(), FillRule::Winding);

    // Draw second rectangle with no clip.
    let transform_b = transform_a * Transform2F::from_translation(vec2f(40.0, 0.0)) *
                                    Transform2F::from_rotation(time);
    canvas.set_current_transform(&transform_b);
    canvas.set_fill_style(rgbau(255, 128, 0, 64));
    let fill_rect = RectF::new(vec2f(-20.0, -10.0), vec2f(60.0, 30.0));
    canvas.fill_rect(fill_rect);

    // Draw second rectangle with clip.
    canvas.set_current_transform(&transform_a);
    canvas.clip_path(clip_path, FillRule::Winding);
    canvas.set_current_transform(&transform_b);
    canvas.set_fill_style(rgbu(255, 128, 0));
    canvas.fill_rect(fill_rect);

    canvas.restore();
}

fn draw_window(canvas: &mut CanvasRenderingContext2D, title: &str, rect: RectF) {
    const CORNER_RADIUS: f32 = 3.0;

    canvas.save();

    // Draw window with shadow.
    canvas.set_fill_style(rgbau(28, 30, 34, 192));
    canvas.set_shadow_offset(vec2f(0.0, 2.0));
    canvas.set_shadow_blur(10.0);
    canvas.set_shadow_color(rgbau(0, 0, 0, 128));
    canvas.fill_path(create_rounded_rect_path(rect, CORNER_RADIUS), FillRule::Winding);
    canvas.set_shadow_color(rgbau(0, 0, 0, 0));

    // Header.
    let mut header_gradient =
        Gradient::linear(LineSegment2F::new(Vector2F::zero(), vec2f(0.0, 15.0)) + rect.origin());
    header_gradient.add_color_stop(rgbau(0, 0, 0, 128), 0.0);
    header_gradient.add_color_stop(rgbau(0, 0, 0, 0),   1.0);
    canvas.set_fill_style(header_gradient);
    canvas.fill_path(create_rounded_rect_path(RectF::new(rect.origin() + vec2f(1.0, 1.0),
                                                         vec2f(rect.width() - 2.0, 30.0)),
                                              CORNER_RADIUS - 1.0),
                     FillRule::Winding);
    let mut path = Path2D::new();
    path.move_to(rect.origin() + vec2f(0.5, 30.5));
    path.line_to(rect.origin() + vec2f(rect.width() - 0.5, 30.5));
    canvas.set_stroke_style(rgbau(0, 0, 0, 32));
    canvas.stroke_path(path);

    canvas.set_font(FONT_NAME_BOLD);
    canvas.set_font_size(15.0);
    canvas.set_text_align(TextAlign::Center);
    canvas.set_text_baseline(TextBaseline::Middle);
    canvas.set_fill_style(rgbau(220, 220, 220, 160));
    canvas.set_shadow_blur(2.0);
    canvas.set_shadow_offset(vec2f(0.0, 1.0));
    canvas.set_shadow_color(rgbau(0, 0, 0, 128));
    canvas.fill_text(title, rect.origin() + vec2f(rect.width() * 0.5, 16.0));

    canvas.restore();
}

fn draw_search_box(canvas: &mut CanvasRenderingContext2D, text: &str, rect: RectF) {
    let corner_radius = rect.height() * 0.5 - 1.0;

    fill_path_with_box_gradient(canvas,
                                create_rounded_rect_path(rect, corner_radius),
                                FillRule::Winding,
                                rect + vec2f(0.0, 1.5),
                                rect.height() * 0.5,
                                5.0,
                                rgbau(0, 0, 0, 16),
                                rgbau(0, 0, 0, 92));

    canvas.set_font_size(rect.height() * 0.5);
    canvas.set_font(FONT_NAME_EMOJI);
    canvas.set_fill_style(rgbau(255, 255, 255, 64));
    canvas.set_text_align(TextAlign::Center);
    canvas.set_text_baseline(TextBaseline::Middle);
    canvas.fill_text("🔍", rect.origin() + (rect.height() * 0.55));

    canvas.set_font(FONT_NAME_REGULAR);
    canvas.set_font_size(17.0);
    canvas.set_fill_style(rgbau(255, 255, 255, 64));
    canvas.set_text_align(TextAlign::Left);
    canvas.set_text_baseline(TextBaseline::Middle);
    canvas.fill_text(text, rect.origin() + vec2f(1.05, 0.5) * rect.height());

    canvas.set_font_size(rect.height() * 0.5);
    canvas.set_font(FONT_NAME_EMOJI);
    canvas.set_text_align(TextAlign::Center);
    canvas.fill_text("️❌", rect.upper_right() + vec2f(-1.0, 1.0) * (rect.height() * 0.55));
}

fn draw_dropdown(canvas: &mut CanvasRenderingContext2D, text: &str, rect: RectF) {
    const CORNER_RADIUS: f32 = 4.0;

    let mut background_gradient = Gradient::linear_from_points(rect.origin(), rect.lower_left());
    background_gradient.add_color_stop(rgbau(255, 255, 255, 16), 0.0);
    background_gradient.add_color_stop(rgbau(0,   0,   0,   16), 1.0);
    canvas.set_fill_style(background_gradient);
    canvas.fill_path(create_rounded_rect_path(rect.contract(1.0), CORNER_RADIUS - 1.0),
                     FillRule::Winding);

    canvas.set_stroke_style(rgbau(0, 0, 0, 48));
    canvas.stroke_path(create_rounded_rect_path(rect.contract(0.5), CORNER_RADIUS - 0.5));

    canvas.set_font(FONT_NAME_REGULAR);
    canvas.set_font_size(17.0);
    canvas.set_fill_style(rgbau(255, 255, 255, 160));
    canvas.set_text_align(TextAlign::Left);
    canvas.set_text_baseline(TextBaseline::Middle);
    canvas.fill_text(text, rect.origin() + vec2f(0.3, 0.5) * rect.height());

    // Draw chevron. This is a glyph in the original, but I don't want to grab an icon font just
    // for this.
    canvas.save();
    let original_transform = canvas.current_transform();
    canvas.set_current_transform(&(
        original_transform *
        Transform2F::from_translation(rect.upper_right() + vec2f(-0.5, 0.33) * rect.height()) *
        Transform2F::from_scale(0.1)));
    canvas.set_fill_style(rgbau(255, 255, 255, 64));
    let mut path = Path2D::new();
    path.move_to(vec2f(0.0,  100.0));
    path.line_to(vec2f(32.8, 50.0));
    path.line_to(vec2f(0.0,  0.0));
    path.line_to(vec2f(22.1, 0.0));
    path.line_to(vec2f(54.2, 50.0));
    path.line_to(vec2f(22.1, 100.0));
    path.close_path();
    canvas.fill_path(path, FillRule::Winding);
    canvas.restore();
}

fn draw_label(canvas: &mut CanvasRenderingContext2D, text: &str, rect: RectF) {
    canvas.set_font(FONT_NAME_REGULAR);
    canvas.set_font_size(15.0);
    canvas.set_fill_style(rgbau(255, 255, 255, 128));
    canvas.set_text_align(TextAlign::Left);
    canvas.set_text_baseline(TextBaseline::Middle);
    canvas.fill_text(text, rect.origin() + vec2f(0.0, rect.height() * 0.5));
}

fn draw_edit_box(canvas: &mut CanvasRenderingContext2D, rect: RectF) {
    const CORNER_RADIUS: f32 = 4.0;

    fill_path_with_box_gradient(canvas,
                                create_rounded_rect_path(rect.contract(1.0), CORNER_RADIUS - 1.0),
                                FillRule::Winding,
                                rect.contract(1.0) + vec2f(0.0, 1.5),
                                3.0,
                                4.0,
                                rgbau(255, 255, 255, 32),
                                rgbau(32,  32,  32,  32));

    canvas.set_stroke_style(rgbau(0, 0, 0, 48));
    canvas.stroke_path(create_rounded_rect_path(rect.contract(0.5), CORNER_RADIUS - 0.5));
}

fn draw_text_edit_box(canvas: &mut CanvasRenderingContext2D, text: &str, rect: RectF) {
    draw_edit_box(canvas, rect);

    canvas.set_font(FONT_NAME_REGULAR);
    canvas.set_font_size(17.0);
    canvas.set_fill_style(rgbau(255, 255, 255, 64));
    canvas.set_text_align(TextAlign::Left);
    canvas.set_text_baseline(TextBaseline::Middle);
    canvas.fill_text(text, rect.origin() + vec2f(0.3, 0.5) * rect.height());
}

fn draw_numeric_edit_box(canvas: &mut CanvasRenderingContext2D,
                         value: &str,
                         unit: &str,
                         rect: RectF) {
    draw_edit_box(canvas, rect);

    canvas.set_font(FONT_NAME_REGULAR);
    canvas.set_font_size(15.0);
    let unit_width = canvas.measure_text(unit).width;

    canvas.set_fill_style(rgbau(255, 255, 255, 64));
    canvas.set_text_align(TextAlign::Right);
    canvas.set_text_baseline(TextBaseline::Middle);
    canvas.fill_text(unit, rect.upper_right() + vec2f(-0.3, 0.5) * rect.height());

    canvas.set_font_size(17.0);
    canvas.set_fill_style(rgbau(255, 255, 255, 128));
    canvas.set_text_align(TextAlign::Right);
    canvas.set_text_baseline(TextBaseline::Middle);
    canvas.fill_text(value, rect.upper_right() + vec2f(-unit_width - rect.height() * 0.5,
                                                       rect.height() * 0.5));
}

fn draw_check_box(canvas: &mut CanvasRenderingContext2D, text: &str, rect: RectF) {
    const CORNER_RADIUS: f32 = 3.0;

    canvas.set_font(FONT_NAME_REGULAR);
    canvas.set_font_size(15.0);
    canvas.set_fill_style(rgbau(255, 255, 255, 160));
    canvas.set_text_align(TextAlign::Left);
    canvas.set_text_baseline(TextBaseline::Middle);
    canvas.fill_text(text, rect.origin() + vec2f(28.0, rect.height() * 0.5));

    let check_box_rect = RectF::new(vec2f(rect.origin_x(), rect.center().y().floor() - 9.0),
                                    vec2f(20.0, 20.0)).contract(1.0);
    fill_path_with_box_gradient(canvas,
                                create_rounded_rect_path(check_box_rect, CORNER_RADIUS),
                                FillRule::Winding,
                                check_box_rect + vec2f(0.0, 1.0),
                                CORNER_RADIUS,
                                3.0,
                                rgbau(0, 0, 0, 32),
                                rgbau(0, 0, 0, 92));

    canvas.set_font(FONT_NAME_EMOJI);
    canvas.set_font_size(17.0);
    canvas.set_fill_style(rgbau(255, 255, 255, 128));
    canvas.set_text_align(TextAlign::Center);
    canvas.fill_text("✔︎", rect.origin() + vec2f(11.0, rect.height() * 0.5));
}

fn draw_button(canvas: &mut CanvasRenderingContext2D,
               pre_icon: Option<&str>,
               text: &str,
               rect: RectF,
               color: ColorU) {
    const CORNER_RADIUS: f32 = 4.0;

    let path = create_rounded_rect_path(rect.contract(1.0), CORNER_RADIUS - 1.0);
    if color != ColorU::transparent_black() {
        canvas.set_fill_style(color);
        canvas.fill_path(path.clone(), FillRule::Winding);
    }
    let alpha = if color == ColorU::transparent_black() { 16 } else { 32 };
    let mut background_gradient = Gradient::linear_from_points(rect.origin(), rect.lower_left());
    background_gradient.add_color_stop(rgbau(255, 255, 255, alpha), 0.0);
    background_gradient.add_color_stop(rgbau(0,   0,   0,   alpha), 1.0);
    canvas.set_fill_style(background_gradient);
    canvas.fill_path(path, FillRule::Winding);

    canvas.set_stroke_style(rgbau(0, 0, 0, 48));
    canvas.stroke_path(create_rounded_rect_path(rect.contract(0.5), CORNER_RADIUS - 0.5));

    // TODO(pcwalton): Icon.
    canvas.set_font(FONT_NAME_BOLD);
    canvas.set_font_size(17.0);
    let text_width = canvas.measure_text(text).width;

    let icon_width;
    match pre_icon {
        None => icon_width = 0.0,
        Some(icon) => {
            canvas.set_font_size(rect.height() * 0.7);
            canvas.set_font(FONT_NAME_EMOJI);
            icon_width = canvas.measure_text(icon).width + rect.height() * 0.15;
            canvas.set_fill_style(rgbau(255, 255, 255, 96));
            canvas.set_text_align(TextAlign::Left);
            canvas.set_text_baseline(TextBaseline::Middle);
            canvas.fill_text(icon,
                             rect.center() - vec2f(text_width * 0.5 + icon_width * 0.75, 0.0));
        }
    }

    canvas.set_font(FONT_NAME_BOLD);
    canvas.set_font_size(17.0);
    let text_origin = rect.center() + vec2f(icon_width * 0.25 - text_width * 0.5, 0.0);
    canvas.set_text_align(TextAlign::Left);
    canvas.set_text_baseline(TextBaseline::Middle);
    canvas.set_shadow_color(rgbau(0, 0, 0, 160));
    canvas.set_shadow_offset(vec2f(0.0, -1.0));
    canvas.set_shadow_blur(0.0);
    canvas.set_fill_style(rgbau(255, 255, 255, 160));
    canvas.fill_text(text, text_origin);
    canvas.set_shadow_color(ColorU::transparent_black());
}

fn draw_slider(canvas: &mut CanvasRenderingContext2D, value: f32, rect: RectF) {
    let (center_y, knob_radius) = (rect.center().y().floor(), (rect.height() * 0.25).floor());

    canvas.save();

    // Draw track.
    let track_rect = RectF::new(vec2f(rect.origin_x(), center_y - 2.0), vec2f(rect.width(), 4.0));
    fill_path_with_box_gradient(canvas,
                                create_rounded_rect_path(track_rect, 2.0),
                                FillRule::Winding,
                                track_rect + vec2f(0.0, 1.0),
                                2.0,
                                2.0,
                                rgbau(0, 0, 0, 32),
                                rgbau(0, 0, 0, 128));

    // Draw knob shadow.
    let knob_position = vec2f(rect.origin_x() + (value * rect.width()).floor(), center_y);
    let mut background_gradient =
        Gradient::radial(LineSegment2F::new(knob_position, knob_position) + vec2f(0.0, 1.0),
                         F32x2::splat(knob_radius) * F32x2::new(-3.0, 3.0));
    background_gradient.add_color_stop(rgbau(0, 0, 0, 64), 0.0);
    background_gradient.add_color_stop(rgbau(0, 0, 0, 0),  1.0);
    canvas.set_fill_style(background_gradient);
    let mut path = Path2D::new();
    path.rect(RectF::new(knob_position, Vector2F::zero()).dilate(knob_radius + 5.0));
    path.ellipse(knob_position, knob_radius, 0.0, 0.0, PI_2);
    canvas.fill_path(path, FillRule::EvenOdd);

    // Fill knob.
    let mut background_gradient =
        Gradient::linear_from_points(knob_position - vec2f(0.0, knob_radius),
                                     knob_position + vec2f(0.0, knob_radius));
    background_gradient.add_color_stop(rgbau(255, 255, 255, 16), 0.0);
    background_gradient.add_color_stop(rgbau(0,   0,   0,   16), 1.0);
    let mut path = Path2D::new();
    path.ellipse(knob_position, knob_radius - 1.0, 0.0, 0.0, PI_2);
    canvas.set_fill_style(rgbu(40, 43, 48));
    canvas.fill_path(path.clone(), FillRule::Winding);
    canvas.set_fill_style(background_gradient);
    canvas.fill_path(path, FillRule::Winding);

    // Outline knob.
    let mut path = Path2D::new();
    path.ellipse(knob_position, knob_radius - 0.5, 0.0, 0.0, PI_2);
    canvas.set_stroke_style(rgbau(0, 0, 0, 92));
    canvas.stroke_path(path);

    canvas.restore();
}

fn draw_thumbnails(canvas: &mut CanvasRenderingContext2D,
                   rect: RectF,
                   time: f32,
                   image_count: usize,
                   image: &Image) {
    const CORNER_RADIUS: f32 = 3.0;
    const THUMB_HEIGHT: f32 = 60.0;
    const ARROW_Y_POSITION: f32 = 30.5;
    const IMAGES_ACROSS: usize = 4;

    let stack_height = image_count as f32 * 0.5 * (THUMB_HEIGHT + 10.0) + 10.0;
    let scroll_height = rect.height() / stack_height * (rect.height() - 8.0);
    let scroll_y = (1.0 + f32::cos(time * 0.5)) * 0.5;
    let load_y = (1.0 - f32::cos(time * 0.2)) * 0.5;
    let image_y_scale = 1.0 / (image_count as f32 - 1.0);

    canvas.save();

    // Draw drop shadow.
    let mut path = create_rounded_rect_path(rect, CORNER_RADIUS);
    path.rect(RectF::new(rect.origin() - vec2f(10.0, 10.0), rect.size() + vec2f(20.0, 30.0)));
    fill_path_with_box_gradient(canvas,
                                path,
                                FillRule::EvenOdd,
                                rect + vec2f(0.0, 4.0),
                                CORNER_RADIUS * 2.0,
                                20.0,
                                rgbau(0, 0, 0, 128),
                                rgbau(0, 0, 0, 0));

    // Draw window.
    let mut path = create_rounded_rect_path(rect, CORNER_RADIUS);
    path.move_to(rect.origin() + vec2f(-10.0, ARROW_Y_POSITION));
    path.line_to(rect.origin() + vec2f(1.0, ARROW_Y_POSITION - 11.0));
    path.line_to(rect.origin() + vec2f(1.0, ARROW_Y_POSITION + 11.0));
    canvas.set_fill_style(rgbu(200, 200, 200));
    canvas.fill_path(path, FillRule::Winding);

    // Draw images.

    canvas.save();
    let mut clip_path = Path2D::new();
    clip_path.rect(rect);
    canvas.clip_path(clip_path, FillRule::Winding);
    let original_transform = canvas.current_transform();
    canvas.set_current_transform(&(original_transform * Transform2F::from_translation(
        vec2f(0.0, -scroll_y * (stack_height - rect.height())))));

    for image_index in 0..image_count {
        let image_origin = rect.origin() + vec2f(10.0, 10.0) +
            vec2i(image_index as i32 % 2, image_index as i32 / 2).to_f32() * (THUMB_HEIGHT + 10.0);
        let image_rect = RectF::new(image_origin, Vector2F::splat(THUMB_HEIGHT)); 

        let image_y = image_index as f32 * image_y_scale;
        let alpha = util::clamp((load_y - image_y) / image_y_scale, 0.0, 1.0);
        if alpha < 1.0 {
            draw_spinner(canvas, image_rect.center(), THUMB_HEIGHT * 0.25, time);
        }

        let image_path = create_rounded_rect_path(image_rect, 5.0);
        let pattern_transform = Transform2F::from_translation(
            image_rect.origin() - vec2i(
                (image_index % IMAGES_ACROSS) as i32,
                (image_index / IMAGES_ACROSS) as i32).to_f32() * THUMB_HEIGHT) *
            Transform2F::from_scale(0.5);
        let pattern = Pattern::new(PatternSource::Image((*image).clone()),
                                   pattern_transform,
                                   PatternFlags::empty());
        canvas.set_fill_style(pattern);
        canvas.set_global_alpha(alpha);
        canvas.fill_path(image_path, FillRule::Winding);
        canvas.set_global_alpha(1.0);

        let mut shadow_path = create_rounded_rect_path(image_rect, 6.0);
        shadow_path.rect(image_rect.dilate(5.0));
        // TODO(pcwalton): Union clip paths.
        /*
        fill_path_with_box_gradient(
            canvas,
            shadow_path,
            FillRule::EvenOdd,
            image_rect.dilate(1.0) + vec2f(0.0, 1.0),
            5.0,
            3.0,
            rgbau(0, 0, 0, 128),
            rgbau(0, 0, 0, 0));
        */

        canvas.set_stroke_style(rgbau(255, 255, 255, 192));
        canvas.stroke_path(create_rounded_rect_path(image_rect.dilate(0.5), 3.5));
    }

    canvas.restore();

    // Draw fade-away gradients.

    let mut fade_gradient = Gradient::linear_from_points(rect.origin(),
                                                         rect.origin() + vec2f(0.0, 6.0));
    fade_gradient.add_color_stop(rgbau(200, 200, 200, 255), 0.0);
    fade_gradient.add_color_stop(rgbau(200, 200, 200, 0),   1.0);
    canvas.set_fill_style(fade_gradient);
    canvas.fill_rect(RectF::new(rect.origin() + vec2f(4.0, 0.0), vec2f(rect.width() - 8.0, 6.0)));

    let mut fade_gradient = Gradient::linear_from_points(rect.lower_left(),
                                                         rect.lower_left() - vec2f(0.0, 6.0));
    fade_gradient.add_color_stop(rgbau(200, 200, 200, 255), 0.0);
    fade_gradient.add_color_stop(rgbau(200, 200, 200, 0),   1.0);
    canvas.set_fill_style(fade_gradient);
    canvas.fill_rect(RectF::new(rect.lower_left() + vec2f(4.0, -6.0),
                                vec2f(rect.width() - 8.0, 6.0)));

    // Draw scroll bar.

    let scroll_bar_rect = RectF::new(rect.upper_right() + vec2f(-12.0, 4.0),
                                     vec2f(8.0, rect.height() - 8.0));
    fill_path_with_box_gradient(canvas,
                                create_rounded_rect_path(scroll_bar_rect, CORNER_RADIUS),
                                FillRule::Winding,
                                scroll_bar_rect + vec2f(0.0, 1.0),
                                CORNER_RADIUS,
                                4.0,
                                rgbau(0, 0, 0, 32),
                                rgbau(0, 0, 0, 92));

    let knob_rect = RectF::new(
        rect.upper_right() + vec2f(-11.0, 5.0 + (rect.height() - 8.0 - scroll_height) * scroll_y),
        vec2f(6.0, scroll_height - 2.0));
    fill_path_with_box_gradient(canvas,
                                create_rounded_rect_path(knob_rect, 2.0),
                                FillRule::Winding,
                                knob_rect.dilate(2.0) + vec2f(0.0, 1.0),
                                3.0,
                                4.0,
                                rgbu(220, 220, 220),
                                rgbu(128, 128, 128));

    canvas.restore();
}

fn draw_spinner(canvas: &mut CanvasRenderingContext2D, center: Vector2F, radius: f32, time: f32) {
    let (start_angle, end_angle) = (time * 6.0, PI + time * 6.0);
    let (outer_radius, inner_radius) = (radius, radius * 0.75);
    let average_radius = util::lerp(outer_radius, inner_radius, 0.5);

    canvas.save();

    let mut path = Path2D::new();
    path.arc(center, outer_radius, start_angle, end_angle, ArcDirection::CW);
    path.arc(center, inner_radius, end_angle, start_angle, ArcDirection::CCW);
    path.close_path();
    set_linear_gradient_fill_style(
        canvas,
        center + vec2f(outer_radius.cos(), outer_radius.sin()) * average_radius,
        center + vec2f(inner_radius.cos(), inner_radius.sin()) * average_radius,
        rgbau(0, 0, 0, 0),
        rgbau(0, 0, 0, 128));
    canvas.fill_path(path, FillRule::Winding);

    canvas.restore();
}

fn fill_path_with_box_gradient(canvas: &mut CanvasRenderingContext2D,
                               path: Path2D,
                               fill_rule: FillRule,
                               rect: RectF,
                               corner_radius: f32,
                               blur_radius: f32,
                               inner_color: ColorU,
                               outer_color: ColorU) {
    // TODO(pcwalton): Fill the corners with radial gradients.

    let window_rect = RectF::new(Vector2F::zero(), vec2i(WINDOW_WIDTH, WINDOW_HEIGHT).to_f32());
    let (inner_rect, outer_rect) = (rect.contract(blur_radius), rect.dilate(blur_radius));

    canvas.save();

    canvas.clip_path(path, fill_rule);

    // Draw left part.
    let mut section = Path2D::new();
    section.move_to(window_rect.origin());
    section.line_to(outer_rect.origin());
    section.line_to(inner_rect.origin());
    section.line_to(rect.center());
    section.line_to(inner_rect.lower_left());
    section.line_to(outer_rect.lower_left());
    section.line_to(window_rect.lower_left());
    section.close_path();
    set_linear_gradient_fill_style(canvas,
                                   outer_rect.origin(),
                                   vec2f(inner_rect.min_x(), outer_rect.min_y()),
                                   outer_color,
                                   inner_color);
    canvas.fill_path(section, FillRule::Winding);

    // Draw top part.
    let mut section = Path2D::new();
    section.move_to(window_rect.origin());
    section.line_to(outer_rect.origin());
    section.line_to(inner_rect.origin());
    section.line_to(rect.center());
    section.line_to(inner_rect.upper_right());
    section.line_to(outer_rect.upper_right());
    section.line_to(window_rect.upper_right());
    section.close_path();
    set_linear_gradient_fill_style(canvas,
                                   outer_rect.origin(),
                                   vec2f(outer_rect.min_x(), inner_rect.min_y()),
                                   outer_color,
                                   inner_color);
    canvas.fill_path(section, FillRule::Winding);

    // Draw right part.
    let mut section = Path2D::new();
    section.move_to(window_rect.upper_right());
    section.line_to(outer_rect.upper_right());
    section.line_to(inner_rect.upper_right());
    section.line_to(rect.center());
    section.line_to(inner_rect.lower_right());
    section.line_to(outer_rect.lower_right());
    section.line_to(window_rect.lower_right());
    section.close_path();
    set_linear_gradient_fill_style(canvas,
                                   outer_rect.upper_right(),
                                   vec2f(inner_rect.max_x(), outer_rect.min_y()),
                                   outer_color,
                                   inner_color);
    canvas.fill_path(section, FillRule::Winding);

    // Draw bottom part.
    let mut section = Path2D::new();
    section.move_to(window_rect.lower_right());
    section.line_to(outer_rect.lower_right());
    section.line_to(inner_rect.lower_right());
    section.line_to(rect.center());
    section.line_to(inner_rect.lower_left());
    section.line_to(outer_rect.lower_left());
    section.line_to(window_rect.lower_left());
    section.close_path();
    set_linear_gradient_fill_style(canvas,
                                   outer_rect.lower_left(),
                                   vec2f(outer_rect.min_x(), inner_rect.max_y()),
                                   outer_color,
                                   inner_color);
    canvas.fill_path(section, FillRule::Winding);

    canvas.restore();
}

fn set_linear_gradient_fill_style(canvas: &mut CanvasRenderingContext2D,
                                  from_position: Vector2F,
                                  to_position: Vector2F,
                                  from_color: ColorU,
                                  to_color: ColorU) {
    let mut gradient = Gradient::linear(LineSegment2F::new(from_position, to_position));
    gradient.add_color_stop(from_color, 0.0);
    gradient.add_color_stop(to_color, 1.0);
    canvas.set_fill_style(gradient);
}

fn create_graph_path(sample_points: &[Vector2F], sample_spread: f32, offset: Vector2F) -> Path2D {
    let mut path = Path2D::new();
    path.move_to(sample_points[0] + vec2f(0.0, 2.0));
    for pair in sample_points.windows(2) {
        path.bezier_curve_to(pair[0] + offset + vec2f(sample_spread * 0.5, 0.0),
                             pair[1] + offset - vec2f(sample_spread * 0.5, 0.0),
                             pair[1] + offset);
    }
    path
}

fn create_rounded_rect_path(rect: RectF, radius: f32) -> Path2D {
    let mut path = Path2D::new();
    path.move_to(rect.origin() + vec2f(radius, 0.0));
    path.arc_to(rect.upper_right(), rect.upper_right() + vec2f(0.0,  radius), radius);
    path.arc_to(rect.lower_right(), rect.lower_right() + vec2f(-radius, 0.0), radius);
    path.arc_to(rect.lower_left(),  rect.lower_left()  + vec2f(0.0, -radius), radius);
    path.arc_to(rect.origin(),      rect.origin()      + vec2f(radius,  0.0), radius);
    path.close_path();
    path
}

struct DemoData {
    image: Image,
}

impl DemoData {
    fn load(resources: &dyn ResourceLoader) -> DemoData {
        let data = resources.slurp("textures/example-nanovg.png").unwrap();
        let image = image::load_from_memory(&data).unwrap().to_rgba();
        let image = Image::from_image_buffer(image);
        DemoData { image }
    }
}

fn main() {
    // Set up SDL2.
    let sdl_context = sdl2::init().unwrap();
    let video = sdl_context.video().unwrap();

    // Make sure we have at least a GL 3.0 context. Pathfinder requires this.
    let gl_attributes = video.gl_attr();
    gl_attributes.set_context_profile(GLProfile::Core);
    gl_attributes.set_context_version(3, 3);

    // Open a window.
    let window_size = vec2i(WINDOW_WIDTH, WINDOW_HEIGHT);
    let window =
        video.window("NanoVG example port", window_size.x() as u32, window_size.y() as u32)
             .opengl()
             .allow_highdpi()
             .build()
             .unwrap();

    // Create the GL context, and make it current.
    let gl_context = window.gl_create_context().unwrap();
    gl::load_with(|name| video.gl_get_proc_address(name) as *const _);
    window.gl_make_current(&gl_context).unwrap();

    // Get the real window size (for HiDPI).
    let (drawable_width, drawable_height) = window.drawable_size();
    let drawable_size = vec2i(drawable_width as i32, drawable_height as i32);
    let hidpi_factor = drawable_size.to_f32() / window_size.to_f32();

    // Load demo data.
    let resources = FilesystemResourceLoader::locate();
    let font_data = vec![
        Handle::from_memory(Arc::new(resources.slurp("fonts/Roboto-Regular.ttf").unwrap()), 0),
        Handle::from_memory(Arc::new(resources.slurp("fonts/Roboto-Bold.ttf").unwrap()), 0),
        Handle::from_memory(Arc::new(resources.slurp("fonts/NotoEmoji-Regular.ttf").unwrap()), 0),
    ];
    let demo_data = DemoData::load(&resources);

    // Create a Pathfinder renderer.
    let mut renderer = Renderer::new(GLDevice::new(GLVersion::GL3, 0),
                                     &resources,
                                     DestFramebuffer::full_window(drawable_size),
                                     RendererOptions {
                                         background_color: Some(rgbf(0.3, 0.3, 0.32)),
                                     });

    // Initialize font state.
    let font_source = Arc::new(MemSource::from_fonts(font_data.into_iter()).unwrap());
    let font_context = CanvasFontContext::new(font_source.clone());

    // Initialize general state.
    let mut event_pump = sdl_context.event_pump().unwrap();
    let mut mouse_position = Vector2F::zero();
    let start_time = Instant::now();

    // Enter the main loop.
    loop {
        // Make a canvas.
        let mut canvas = CanvasRenderingContext2D::new(font_context.clone(),
                                                       drawable_size.to_f32());

        // Render the demo.
        let time = (Instant::now() - start_time).as_secs_f32();
        canvas.set_current_transform(&Transform2F::from_scale(hidpi_factor));
        render_demo(&mut canvas, mouse_position, window_size.to_f32(), time, &demo_data);

        // Render the canvas to screen.
        let scene = SceneProxy::from_scene(canvas.into_scene(), RayonExecutor);
        scene.build_and_render(&mut renderer, BuildOptions::default());
        window.gl_swap_window();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => return,
                Event::MouseMotion { x, y, .. } => mouse_position = vec2i(x, y).to_f32(),
                _ => {}
            }
        }
    }
}
