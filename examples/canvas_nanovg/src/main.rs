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
use pathfinder_canvas::{CanvasFontContext, CanvasRenderingContext2D};
use pathfinder_canvas::{FillStyle, LineJoin, Path2D};
use pathfinder_color::{ColorF, ColorU};
use pathfinder_content::fill::FillRule;
use pathfinder_content::gradient::{ColorStop, Gradient};
use pathfinder_content::outline::ArcDirection;
use pathfinder_content::stroke::LineCap;
use pathfinder_geometry::angle;
use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::util;
use pathfinder_geometry::vector::{Vector2F, Vector2I};
use pathfinder_gl::{GLDevice, GLVersion};
use pathfinder_renderer::concurrent::rayon::RayonExecutor;
use pathfinder_renderer::concurrent::scene_proxy::SceneProxy;
use pathfinder_renderer::gpu::options::{DestFramebuffer, RendererOptions};
use pathfinder_renderer::gpu::renderer::Renderer;
use pathfinder_renderer::options::BuildOptions;
use pathfinder_resources::fs::FilesystemResourceLoader;
use pathfinder_simd::default::F32x2;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::video::GLProfile;
use std::f32::consts::PI;
use std::time::Instant;

// TODO(pcwalton): See if we can reduce the amount of code by using the canvas shadow feature.

const PI_2: f32 = PI * 2.0;

static PARAGRAPH_TEXT: &'static str = "This is a longer chunk of text.

I would have used lorem ipsum, but she was busy jumping over the lazy dog with the fox and all \
the men who came to the aid of the party.";

fn render_demo(canvas: &mut CanvasRenderingContext2D,
               mouse_position: Vector2F,
               window_size: Vector2F,
               time: f32) {
    draw_eyes(canvas,
              RectF::new(Vector2F::new(window_size.x() - 250.0, 50.0),
                         Vector2F::new(150.0, 100.0)),
              mouse_position,
              time);
    /*
    FIXME(pcwalton): Too slow. See https://github.com/linebender/skribo/issues/30
    draw_paragraph(canvas,
                   RectF::new(Vector2F::new(window_size.x() - 450.0, 50.0),
                              Vector2F::new(150.0, 100.0)));
    */
    draw_graph(canvas,
               RectF::new(window_size.scale_xy(Vector2F::new(0.0, 0.5)),
                          window_size.scale_xy(Vector2F::new(1.0, 0.5))),
               time);
    draw_color_wheel(canvas,
                     RectF::new(window_size - Vector2F::splat(300.0), Vector2F::splat(250.0)),
                     time);
    draw_lines(canvas,
               RectF::new(Vector2F::new(120.0, window_size.y() - 50.0),
                          Vector2F::new(600.0, 50.0)),
               time);
    draw_caps(canvas, RectF::new(Vector2F::new(10.0, 300.0), Vector2F::new(30.0, 40.0)));
    draw_clip(canvas, Vector2F::new(50.0, window_size.y() - 80.0), time);

    canvas.save();

    // Draw widgets.
    draw_window(canvas,
                "Widgets 'n' Stuff",
                RectF::new(Vector2F::splat(50.0), Vector2F::new(300.0, 400.0)));
    let mut position = Vector2F::new(60.0, 95.0);
    draw_search_box(canvas, "Search", RectF::new(position, Vector2F::new(280.0, 25.0)));
    position += Vector2F::new(0.0, 40.0);
    draw_dropdown(canvas, "Effects", RectF::new(position, Vector2F::new(280.0, 28.0)));
    let popup_position = position + Vector2F::new(0.0, 14.0);
    position += Vector2F::new(0.0, 45.0);

    // Draw login form.
    position += Vector2F::new(0.0, 25.0);
    draw_text_edit_box(canvas, "E-mail address", RectF::new(position, Vector2F::new(280.0, 28.0)));
    position += Vector2F::new(0.0, 35.0);
    draw_text_edit_box(canvas, "Password", RectF::new(position, Vector2F::new(280.0, 28.0)));
    position += Vector2F::new(0.0, 38.0);
    draw_check_box(canvas, "Remember me", RectF::new(position, Vector2F::new(140.0, 28.0)));
    draw_button(canvas,
                "Sign In",
                RectF::new(position + Vector2F::new(138.0, 0.0), Vector2F::new(140.0, 28.0)),
                ColorU::new(0, 96, 128, 255));
    position += Vector2F::new(0.0, 45.0);

    // Draw slider form.
    position += Vector2F::new(0.0, 25.0);
    draw_numeric_edit_box(canvas, "123.00", "px", RectF::new(position + Vector2F::new(180.0, 0.0),
                                                             Vector2F::new(100.0, 28.0)));
    draw_slider(canvas, 0.4, RectF::new(position, Vector2F::new(170.0, 28.0)));
    position += Vector2F::new(0.0, 55.0);

    // Draw dialog box buttons.
    draw_button(canvas,
                "Delete",
                RectF::new(position, Vector2F::new(160.0, 28.0)),
                ColorU::new(128, 16, 8, 255));
    draw_button(canvas,
                "Cancel",
                RectF::new(position + Vector2F::new(170.0, 0.0), Vector2F::new(110.0, 28.0)),
                ColorU::transparent_black());

    canvas.restore();
}

fn draw_eyes(canvas: &mut CanvasRenderingContext2D,
             rect: RectF,
             mouse_position: Vector2F,
             time: f32) {
    let eyes_radii = rect.size().scale_xy(Vector2F::new(0.23, 0.5));
    let eyes_left_position = rect.origin() + eyes_radii;
    let eyes_right_position = rect.origin() + Vector2F::new(rect.width() - eyes_radii.x(),
                                                            eyes_radii.y());
    let eyes_center = f32::min(eyes_radii.x(), eyes_radii.y()) * 0.5;
    let blink = 1.0 - f32::powf(f32::sin(time * 0.5), 200.0) * 0.8;

    let mut gradient =
        Gradient::linear(LineSegment2F::new(Vector2F::new(0.0, rect.height() * 0.5),
                                            rect.size().scale_xy(Vector2F::new(0.1, 1.0))) +
                         rect.origin());
    gradient.add_color_stop(ColorStop::new(ColorU::new(0, 0, 0, 32), 0.0));
    gradient.add_color_stop(ColorStop::new(ColorU::new(0, 0, 0, 16), 1.0));
    let mut path = Path2D::new();
    path.ellipse(eyes_left_position  + Vector2F::new(3.0, 16.0), eyes_radii, 0.0, 0.0, PI_2);
    path.ellipse(eyes_right_position + Vector2F::new(3.0, 16.0), eyes_radii, 0.0, 0.0, PI_2);
    canvas.set_fill_style(FillStyle::Gradient(gradient));
    canvas.fill_path(path, FillRule::Winding);

    let mut gradient =
        Gradient::linear(LineSegment2F::new(Vector2F::new(0.0, rect.height() * 0.25),
                                            rect.size().scale_xy(Vector2F::new(0.1, 1.0))) +
                         rect.origin());
    gradient.add_color_stop(ColorStop::new(ColorU::new(220, 220, 220, 255), 0.0));
    gradient.add_color_stop(ColorStop::new(ColorU::new(128, 128, 128, 255), 1.0));
    let mut path = Path2D::new();
    path.ellipse(eyes_left_position, eyes_radii, 0.0, 0.0, PI_2);
    path.ellipse(eyes_right_position, eyes_radii, 0.0, 0.0, PI_2);
    canvas.set_fill_style(FillStyle::Gradient(gradient));
    canvas.fill_path(path, FillRule::Winding);

    let mut delta = (mouse_position - eyes_right_position) / eyes_radii.scale(10.0);
    let distance = delta.length();
    if distance > 1.0 {
        delta = delta.scale(1.0 / distance);
    }
    delta = delta.scale_xy(eyes_radii).scale_xy(Vector2F::new(0.4, 0.5));
    let mut path = Path2D::new();
    path.ellipse(eyes_left_position +
                 delta +
                 Vector2F::new(0.0, eyes_radii.y() * 0.25 * (1.0 - blink)),
                 Vector2F::new(eyes_center, eyes_center * blink),
                 0.0,
                 0.0,
                 PI_2);
    path.ellipse(eyes_right_position +
                 delta +
                 Vector2F::new(0.0, eyes_radii.y() * 0.25 * (1.0 - blink)),
                 Vector2F::new(eyes_center, eyes_center * blink),
                 0.0,
                 0.0,
                 PI_2);
    canvas.set_fill_style(FillStyle::Color(ColorU::new(32, 32, 32, 255)));
    canvas.fill_path(path, FillRule::Winding);

    let gloss_position = eyes_left_position - eyes_radii.scale_xy(Vector2F::new(0.25, 0.5));
    let gloss_radii = F32x2::new(0.1, 0.75) * F32x2::splat(eyes_radii.x());
    let mut gloss = Gradient::radial(LineSegment2F::new(gloss_position, gloss_position),
                                     gloss_radii);
    gloss.add_color_stop(ColorStop::new(ColorU::new(255, 255, 255, 128), 0.0));
    gloss.add_color_stop(ColorStop::new(ColorU::new(255, 255, 255, 0), 1.0));
    canvas.set_fill_style(FillStyle::Gradient(gloss));
    let mut path = Path2D::new();
    path.ellipse(eyes_left_position, eyes_radii, 0.0, 0.0, PI_2);
    canvas.fill_path(path, FillRule::Winding);

    let gloss_position = eyes_right_position - eyes_radii.scale_xy(Vector2F::new(0.25, 0.5));
    let mut gloss = Gradient::radial(LineSegment2F::new(gloss_position, gloss_position),
                                     gloss_radii);
    gloss.add_color_stop(ColorStop::new(ColorU::new(255, 255, 255, 128), 0.0));
    gloss.add_color_stop(ColorStop::new(ColorU::new(255, 255, 255, 0), 1.0));
    canvas.set_fill_style(FillStyle::Gradient(gloss));
    let mut path = Path2D::new();
    path.ellipse(eyes_right_position, eyes_radii, 0.0, 0.0, PI_2);
    canvas.fill_path(path, FillRule::Winding);
}

// This is nowhere near correct line layout, but it suffices to more or less match what NanoVG
// does.
fn draw_paragraph(canvas: &mut CanvasRenderingContext2D, rect: RectF) {
    const LINE_HEIGHT: f32 = 24.0;

    canvas.save();

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
                cursor = cursor + Vector2F::new(space_width, 0.0);
            }

            canvas.set_fill_style(FillStyle::Color(ColorU::white()));
            canvas.fill_text(word, cursor);

            cursor = cursor + Vector2F::new(word_width, 0.0);
        }
    }

    canvas.restore();

    fn next_line(canvas: &mut CanvasRenderingContext2D, cursor: &mut Vector2F, rect: RectF) {
        cursor.set_x(rect.min_x());

        canvas.set_fill_style(FillStyle::Color(ColorU::new(255, 255, 255, 16)));
        canvas.fill_rect(RectF::new(*cursor, Vector2F::new(rect.width(), LINE_HEIGHT)));

        *cursor = *cursor + Vector2F::new(0.0, LINE_HEIGHT);
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

    let sample_scale = Vector2F::new(sample_spread, rect.height() * 0.8);
    let sample_points: ArrayVec<[Vector2F; 6]> = samples.iter()
                                                        .enumerate()
                                                        .map(|(index, &sample)| {
        rect.origin() + Vector2F::new(index as f32, sample).scale_xy(sample_scale)
    }).collect();

    // Draw graph background.
    let mut background = Gradient::linear(LineSegment2F::new(Vector2F::default(),
                                                             Vector2F::new(0.0, rect.height())) +
                                          rect.origin());
    background.add_color_stop(ColorStop::new(ColorU::new(0, 160, 192, 0),  0.0));
    background.add_color_stop(ColorStop::new(ColorU::new(0, 160, 192, 64), 1.0));
    canvas.set_fill_style(FillStyle::Gradient(background));
    let mut path = create_graph_path(&sample_points, sample_spread, Vector2F::default());
    path.line_to(rect.lower_right());
    path.line_to(rect.lower_left());
    canvas.fill_path(path, FillRule::Winding);

    // Draw graph line shadow.
    canvas.set_stroke_style(FillStyle::Color(ColorU::new(0, 0, 0, 32)));
    canvas.set_line_width(3.0);
    let path = create_graph_path(&sample_points, sample_spread, Vector2F::new(0.0, 2.0));
    canvas.stroke_path(path);

    // Draw graph line.
    canvas.set_stroke_style(FillStyle::Color(ColorU::new(0, 160, 192, 255)));
    canvas.set_line_width(3.0);
    let path = create_graph_path(&sample_points, sample_spread, Vector2F::default());
    canvas.stroke_path(path);

    // Draw sample position highlights.
    for &sample_point in &sample_points {
        let gradient_center = sample_point + Vector2F::new(0.0, 2.0);
        let mut background = Gradient::radial(LineSegment2F::new(gradient_center, gradient_center),
                                              F32x2::new(3.0, 8.0));
        background.add_color_stop(ColorStop::new(ColorU::new(0, 0, 0, 32), 0.0));
        background.add_color_stop(ColorStop::new(ColorU::transparent_black(), 1.0));
        canvas.set_fill_style(FillStyle::Gradient(background));
        canvas.fill_rect(RectF::new(sample_point + Vector2F::new(-10.0, -10.0 + 2.0),
                                    Vector2F::splat(20.0)));
    }

    // Draw sample positions.
    canvas.set_fill_style(FillStyle::Color(ColorU::new(0, 160, 192, 255)));
    let mut path = Path2D::new();
    for &sample_point in &sample_points {
        path.ellipse(sample_point, Vector2F::splat(4.0), 0.0, 0.0, PI_2);
    }
    canvas.fill_path(path, FillRule::Winding);
    canvas.set_fill_style(FillStyle::Color(ColorU::new(220, 220, 220, 255)));
    let mut path = Path2D::new();
    for &sample_point in &sample_points {
        path.ellipse(sample_point, Vector2F::splat(2.0), 0.0, 0.0, PI_2);
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
        let line = LineSegment2F::new(Vector2F::new(f32::cos(start_angle), f32::sin(start_angle)),
                                      Vector2F::new(f32::cos(end_angle),   f32::sin(end_angle)));
        let scale = util::lerp(inner_radius, outer_radius, 0.5);
        let mut gradient = Gradient::linear(line.scale(scale) + center);
        let start_color = ColorF::from_hsl(start_angle, 1.0, 0.55).to_u8();
        let end_color   = ColorF::from_hsl(end_angle,   1.0, 0.55).to_u8();
        gradient.add_color_stop(ColorStop::new(start_color, 0.0));
        gradient.add_color_stop(ColorStop::new(end_color,   1.0));
        canvas.set_fill_style(FillStyle::Gradient(gradient));
        let mut path = Path2D::new();
        path.arc(center, inner_radius, start_angle, end_angle,   ArcDirection::CW);
        path.arc(center, outer_radius, end_angle,   start_angle, ArcDirection::CCW);
        path.close_path();
        canvas.fill_path(path, FillRule::Winding);
    }

    // Stroke outer circle.
    canvas.set_stroke_style(FillStyle::Color(ColorU::new(0, 0, 0, 64)));
    canvas.set_line_width(1.0);
    let mut path = Path2D::new();
    path.ellipse(center, Vector2F::splat(inner_radius - 0.5), 0.0, 0.0, PI_2);
    path.ellipse(center, Vector2F::splat(outer_radius + 0.5), 0.0, 0.0, PI_2);
    canvas.stroke_path(path);

    // Prepare to draw the selector.
    canvas.save();
    canvas.set_current_transform(&(Transform2F::from_translation(center) *
                                   Transform2F::from_rotation(hue)));

    canvas.set_stroke_style(FillStyle::Color(ColorU::new(255, 255, 255, 192)));
    canvas.set_line_width(2.0);
    canvas.stroke_rect(RectF::new(Vector2F::new(inner_radius - 1.0, -3.0),
                                  Vector2F::new(outer_radius - inner_radius + 2.0, 6.0)));

    // TODO(pcwalton): Marker fill with box gradient

    // Draw center triangle.
    let triangle_radius = inner_radius - 6.0;
    let triangle_vertex_a = Vector2F::new(triangle_radius, 0.0);
    let triangle_vertex_b = Vector2F::new(f32::cos(PI * 2.0 / 3.0),
                                          f32::sin(PI * 2.0 / 3.0)).scale(triangle_radius);
    let triangle_vertex_c = Vector2F::new(f32::cos(PI * -2.0 / 3.0),
                                          f32::sin(PI * -2.0 / 3.0)).scale(triangle_radius);
    let mut gradient_0 = Gradient::linear(LineSegment2F::new(triangle_vertex_a,
                                                             triangle_vertex_b));
    gradient_0.add_color_stop(ColorStop::new(ColorF::from_hsl(hue, 1.0, 0.5).to_u8(), 0.0));
    gradient_0.add_color_stop(ColorStop::new(ColorU::white(), 1.0));
    let mut gradient_1 =
        Gradient::linear(LineSegment2F::new(triangle_vertex_a.lerp(triangle_vertex_b, 0.5),
                                            triangle_vertex_c));
    gradient_1.add_color_stop(ColorStop::new(ColorU::transparent_black(), 0.0));
    gradient_1.add_color_stop(ColorStop::new(ColorU::black(), 1.0));
    let mut path = Path2D::new();
    path.move_to(triangle_vertex_a);
    path.line_to(triangle_vertex_b);
    path.line_to(triangle_vertex_c);
    path.close_path();
    canvas.set_fill_style(FillStyle::Gradient(gradient_0));
    canvas.fill_path(path.clone(), FillRule::Winding);
    canvas.set_fill_style(FillStyle::Gradient(gradient_1));
    canvas.fill_path(path.clone(), FillRule::Winding);
    canvas.set_stroke_style(FillStyle::Color(ColorU::new(0, 0, 0, 64)));
    canvas.stroke_path(path);

    // Stroke the selection circle on the triangle.
    let selection_circle_center =
        Vector2F::new(f32::cos(PI_2 / 3.0),
                      f32::sin(PI_2 / 3.0)).scale(triangle_radius)
                                           .scale_xy(Vector2F::new(0.3, 0.4));
    canvas.set_stroke_style(FillStyle::Color(ColorU::new(255, 255, 255, 192)));
    canvas.set_line_width(2.0);
    let mut path = Path2D::new();
    path.ellipse(selection_circle_center, Vector2F::splat(5.0), 0.0, 0.0, PI_2);
    canvas.stroke_path(path);

    // Fill the selection circle.
    let mut gradient = Gradient::radial(LineSegment2F::new(selection_circle_center,
                                                           selection_circle_center),
                                        F32x2::new(7.0, 9.0));
    gradient.add_color_stop(ColorStop::new(ColorU::new(0, 0, 0, 64), 0.0));
    gradient.add_color_stop(ColorStop::new(ColorU::transparent_black(), 1.0));
    canvas.set_fill_style(FillStyle::Gradient(gradient));
    let mut path = Path2D::new();
    path.rect(RectF::new(selection_circle_center - Vector2F::splat(20.0), Vector2F::splat(40.0)));
    path.ellipse(selection_circle_center, Vector2F::splat(7.0), 0.0, 0.0, PI_2);
    canvas.fill_path(path, FillRule::EvenOdd);

    canvas.restore();
    canvas.restore();
}

fn draw_lines(canvas: &mut CanvasRenderingContext2D, rect: RectF, time: f32) {
    const PADDING: f32 = 5.0;

    let spacing = rect.width() / 9.0 - PADDING * 2.0;

    canvas.save();

    let points = [
        Vector2F::new(-spacing * 0.25 + f32::cos(time * 0.3)  * spacing * 0.5,
                                        f32::sin(time * 0.3)  * spacing * 0.5),
        Vector2F::new(-spacing * 0.25, 0.0),
        Vector2F::new( spacing * 0.25, 0.0),
        Vector2F::new( spacing * 0.25 + f32::cos(time * -0.3) * spacing * 0.5,
                                        f32::sin(time * -0.3) * spacing * 0.5),
    ];

    for (cap_index, &cap) in [LineCap::Butt, LineCap::Round, LineCap::Square].iter().enumerate() {
        for (join_index, &join) in [
            LineJoin::Miter, LineJoin::Miter /* FIXME(pcwalton): Round crashes */, LineJoin::Bevel
        ].iter().enumerate() {
            let origin = rect.origin() +
                Vector2F::new(spacing, -spacing).scale(0.5) +
                Vector2F::new((cap_index * 3 + join_index) as f32 / 9.0 * rect.width(), 0.0) +
                Vector2F::splat(PADDING);

            canvas.set_line_cap(cap);
            canvas.set_line_join(join);
            canvas.set_line_width(spacing * 0.3);
            canvas.set_stroke_style(FillStyle::Color(ColorU::new(0, 0, 0, 160)));

            let mut path = Path2D::new();
            path.move_to(points[0] + origin);
            path.line_to(points[1] + origin);
            path.line_to(points[2] + origin);
            path.line_to(points[3] + origin);
            canvas.stroke_path(path.clone());

            canvas.set_line_cap(LineCap::Butt);
            canvas.set_line_join(LineJoin::Bevel);
            canvas.set_line_width(1.0);
            canvas.set_stroke_style(FillStyle::Color(ColorU::new(0, 192, 255, 255)));

            canvas.stroke_path(path);
        }
    }

    canvas.restore();
}

fn draw_caps(canvas: &mut CanvasRenderingContext2D, rect: RectF) {
    const LINE_WIDTH: f32 = 8.0;

    canvas.save();

    canvas.set_fill_style(FillStyle::Color(ColorU::new(255, 255, 255, 32)));
    canvas.fill_rect(rect.dilate(Vector2F::new(LINE_WIDTH / 2.0, 0.0)));
    canvas.fill_rect(rect);

    canvas.set_line_width(LINE_WIDTH);
    for (cap_index, &cap) in [LineCap::Butt, LineCap::Round, LineCap::Square].iter().enumerate() {
        canvas.set_line_cap(cap);
        canvas.set_stroke_style(FillStyle::Color(ColorU::black()));
        let offset = cap_index as f32 * 10.0 + 5.0;
        let mut path = Path2D::new();
        path.move_to(rect.origin()      + Vector2F::new(0.0, offset));
        path.line_to(rect.upper_right() + Vector2F::new(0.0, offset));
        canvas.stroke_path(path);
    }

    canvas.restore();
}

fn draw_clip(canvas: &mut CanvasRenderingContext2D, origin: Vector2F, time: f32) {
    canvas.save();

    // Draw first rect.
    let transform_a = Transform2F::from_translation(origin) *
        Transform2F::from_rotation(angle::angle_from_degrees(5.0));
    canvas.set_current_transform(&transform_a);
    canvas.set_fill_style(FillStyle::Color(ColorU::new(255, 0, 0, 255)));
    let mut clip_path = Path2D::new();
    clip_path.rect(RectF::new(Vector2F::splat(-20.0), Vector2F::new(60.0, 40.0)));
    canvas.fill_path(clip_path.clone(), FillRule::Winding);

    // Draw second rectangle with no clip.
    let transform_b = transform_a * Transform2F::from_translation(Vector2F::new(40.0, 0.0)) *
                                    Transform2F::from_rotation(time);
    canvas.set_current_transform(&transform_b);
    canvas.set_fill_style(FillStyle::Color(ColorU::new(255, 128, 0, 64)));
    let fill_rect = RectF::new(Vector2F::new(-20.0, -10.0), Vector2F::new(60.0, 30.0));
    canvas.fill_rect(fill_rect);

    // Draw second rectangle with clip.
    canvas.set_current_transform(&transform_a);
    canvas.clip_path(clip_path, FillRule::Winding);
    canvas.set_current_transform(&transform_b);
    canvas.set_fill_style(FillStyle::Color(ColorU::new(255, 128, 0, 255)));
    canvas.fill_rect(fill_rect);

    canvas.restore();
}

fn draw_window(canvas: &mut CanvasRenderingContext2D, title: &str, rect: RectF) {
    const CORNER_RADIUS: f32 = 3.0;

    canvas.save();

    // Draw window with shadow.
    canvas.set_fill_style(FillStyle::Color(ColorU::new(28, 30, 34, 192)));
    canvas.set_shadow_offset(Vector2F::new(0.0, 2.0));
    canvas.set_shadow_blur(10.0);
    canvas.set_shadow_color(ColorU::new(0, 0, 0, 128));
    canvas.fill_path(create_rounded_rect_path(rect, CORNER_RADIUS), FillRule::Winding);
    canvas.set_shadow_color(ColorU::transparent_black());

    // Header.
    let mut header_gradient = Gradient::linear(LineSegment2F::new(Vector2F::default(),
                                                                  Vector2F::new(0.0, 15.0)) +
                                               rect.origin());
    header_gradient.add_color_stop(ColorStop::new(ColorU::new(0, 0, 0, 128), 0.0));
    header_gradient.add_color_stop(ColorStop::new(ColorU::transparent_black(), 1.0));
    canvas.set_fill_style(FillStyle::Gradient(header_gradient));
    canvas.fill_path(create_rounded_rect_path(RectF::new(rect.origin() + Vector2F::splat(1.0),
                                                         Vector2F::new(rect.width() - 2.0, 30.0)),
                                              CORNER_RADIUS - 1.0),
                     FillRule::Winding);
    let mut path = Path2D::new();
    path.move_to(rect.origin() + Vector2F::new(0.5, 30.5));
    path.line_to(rect.origin() + Vector2F::new(rect.width() - 0.5, 30.5));
    canvas.set_stroke_style(FillStyle::Color(ColorU::new(0, 0, 0, 32)));
    canvas.stroke_path(path);

    canvas.restore();
}

fn draw_search_box(canvas: &mut CanvasRenderingContext2D, text: &str, rect: RectF) {
    let corner_radius = rect.height() * 0.5 - 1.0;

    // TODO(pcwalton): Box gradients.

    canvas.set_fill_style(FillStyle::Color(ColorU::new(0, 0, 0, 54)));
    canvas.fill_path(create_rounded_rect_path(rect, corner_radius), FillRule::Winding);
}

fn draw_dropdown(canvas: &mut CanvasRenderingContext2D, text: &str, rect: RectF) {
    const CORNER_RADIUS: f32 = 4.0;

    let mut background_gradient = Gradient::linear(LineSegment2F::new(rect.origin(),
                                                                      rect.lower_left()));
    background_gradient.add_color_stop(ColorStop::new(ColorU::new(255, 255, 255, 16), 0.0));
    background_gradient.add_color_stop(ColorStop::new(ColorU::new(0, 0, 0, 16), 1.0));
    canvas.set_fill_style(FillStyle::Gradient(background_gradient));
    canvas.fill_path(create_rounded_rect_path(rect.contract(Vector2F::splat(1.0)),
                                              CORNER_RADIUS - 1.0),
                     FillRule::Winding);

    canvas.set_stroke_style(FillStyle::Color(ColorU::new(0, 0, 0, 48)));
    canvas.stroke_path(create_rounded_rect_path(rect.contract(Vector2F::splat(0.5)),
                                                CORNER_RADIUS - 0.5));
}

fn draw_edit_box(canvas: &mut CanvasRenderingContext2D, rect: RectF) {
    const CORNER_RADIUS: f32 = 4.0;

    // TODO(pcwalton): Box gradient.

    let mut background_gradient =
        Gradient::linear(LineSegment2F::new(rect.origin() + Vector2F::new(0.0, 1.0),
                                            rect.origin() + Vector2F::new(0.0, 4.0)));
    background_gradient.add_color_stop(ColorStop::new(ColorU::new(32, 32, 32, 32), 0.0));
    background_gradient.add_color_stop(ColorStop::new(ColorU::new(255, 255, 255, 32), 1.0));
    canvas.set_fill_style(FillStyle::Gradient(background_gradient));
    canvas.fill_path(create_rounded_rect_path(rect.contract(Vector2F::splat(1.0)),
                                              CORNER_RADIUS - 1.0),
                     FillRule::Winding);

    canvas.set_stroke_style(FillStyle::Color(ColorU::new(0, 0, 0, 48)));
    canvas.stroke_path(create_rounded_rect_path(rect.contract(Vector2F::splat(0.5)),
                                                CORNER_RADIUS - 0.5));
}

fn draw_text_edit_box(canvas: &mut CanvasRenderingContext2D, text: &str, rect: RectF) {
    draw_edit_box(canvas, rect)
}

fn draw_numeric_edit_box(canvas: &mut CanvasRenderingContext2D,
                         value: &str,
                         unit: &str,
                         rect: RectF) {
    draw_edit_box(canvas, rect)
}

fn draw_check_box(canvas: &mut CanvasRenderingContext2D, text: &str, rect: RectF) {
    const CORNER_RADIUS: f32 = 3.0;

    // TODO(pcwalton): Box gradients.

    let check_box_rect =
        RectF::new(Vector2F::new(rect.origin_x(), rect.center().y().floor() - 9.0),
                   Vector2F::splat(20.0)).contract(Vector2F::splat(1.0));
    let mut background_gradient = Gradient::linear(LineSegment2F::new(rect.origin(),
                                                                      rect.lower_left()));
    background_gradient.add_color_stop(ColorStop::new(ColorU::new(0, 0, 0, 32), 0.0));
    background_gradient.add_color_stop(ColorStop::new(ColorU::new(0, 0, 0, 92), 1.0));
    canvas.set_fill_style(FillStyle::Gradient(background_gradient));
    canvas.fill_path(create_rounded_rect_path(check_box_rect, CORNER_RADIUS), FillRule::Winding);
}

fn draw_button(canvas: &mut CanvasRenderingContext2D, text: &str, rect: RectF, color: ColorU) {
    const CORNER_RADIUS: f32 = 4.0;

    let path = create_rounded_rect_path(rect.contract(Vector2F::splat(1.0)), CORNER_RADIUS - 1.0);
    if color != ColorU::transparent_black() {
        canvas.set_fill_style(FillStyle::Color(color));
        canvas.fill_path(path.clone(), FillRule::Winding);
    }
    let alpha = if color == ColorU::transparent_black() { 16 } else { 32 };
    let mut background_gradient = Gradient::linear(LineSegment2F::new(rect.origin(),
                                                                      rect.lower_left()));
    background_gradient.add_color_stop(ColorStop::new(ColorU::new(255, 255, 255, alpha), 0.0));
    background_gradient.add_color_stop(ColorStop::new(ColorU::new(0, 0, 0, alpha), 1.0));
    canvas.set_fill_style(FillStyle::Gradient(background_gradient));
    canvas.fill_path(path, FillRule::Winding);

    canvas.set_stroke_style(FillStyle::Color(ColorU::new(0, 0, 0, 48)));
    canvas.stroke_path(create_rounded_rect_path(rect.contract(Vector2F::splat(0.5)),
                                                CORNER_RADIUS - 0.5));
}

fn draw_slider(canvas: &mut CanvasRenderingContext2D, value: f32, rect: RectF) {
    let (center_y, knob_radius) = (rect.center().y().floor(), (rect.height() * 0.25).floor());

    canvas.save();

    // Draw track.
    // TODO(pcwalton): Box gradient.
    let track_rect = RectF::new(Vector2F::new(rect.origin_x(), center_y - 2.0),
                                Vector2F::new(rect.width(), 4.0));
    let mut background_gradient =
        Gradient::linear(LineSegment2F::new(track_rect.origin(), track_rect.lower_left()) +
                         Vector2F::new(0.0, 1.0));
    background_gradient.add_color_stop(ColorStop::new(ColorU::new(0, 0, 0, 32), 0.0));
    background_gradient.add_color_stop(ColorStop::new(ColorU::new(0, 0, 0, 128), 1.0));
    canvas.set_fill_style(FillStyle::Gradient(background_gradient));
    canvas.fill_path(create_rounded_rect_path(track_rect, 2.0), FillRule::Winding);

    // Draw knob shadow.
    let knob_position = Vector2F::new(rect.origin_x() + (value * rect.width()).floor(), center_y);
    let mut background_gradient =
        Gradient::radial(LineSegment2F::new(knob_position,
                                            knob_position) + Vector2F::new(0.0, 1.0),
                         F32x2::splat(knob_radius) * F32x2::new(-3.0, 3.0));
    background_gradient.add_color_stop(ColorStop::new(ColorU::new(0, 0, 0, 64), 0.0));
    background_gradient.add_color_stop(ColorStop::new(ColorU::transparent_black(), 1.0));
    canvas.set_fill_style(FillStyle::Gradient(background_gradient));
    let mut path = Path2D::new();
    path.rect(RectF::new(knob_position,
                         Vector2F::default()).dilate(Vector2F::splat(knob_radius + 5.0)));
    path.ellipse(knob_position, Vector2F::splat(knob_radius), 0.0, 0.0, PI_2);
    canvas.fill_path(path, FillRule::EvenOdd);

    // Fill knob.
    let mut background_gradient =
        Gradient::linear(LineSegment2F::new(knob_position - Vector2F::new(0.0, knob_radius),
                                            knob_position + Vector2F::new(0.0, knob_radius)));
    background_gradient.add_color_stop(ColorStop::new(ColorU::new(255, 255, 255, 16), 0.0));
    background_gradient.add_color_stop(ColorStop::new(ColorU::new(0, 0, 0, 16), 1.0));
    let mut path = Path2D::new();
    path.ellipse(knob_position, Vector2F::splat(knob_radius - 1.0), 0.0, 0.0, PI_2);
    canvas.set_fill_style(FillStyle::Color(ColorU::new(40, 43, 48, 255)));
    canvas.fill_path(path.clone(), FillRule::Winding);
    canvas.set_fill_style(FillStyle::Gradient(background_gradient));
    canvas.fill_path(path, FillRule::Winding);

    // Outline knob.
    let mut path = Path2D::new();
    path.ellipse(knob_position, Vector2F::splat(knob_radius - 0.5), 0.0, 0.0, PI_2);
    canvas.set_stroke_style(FillStyle::Color(ColorU::new(0, 0, 0, 92)));
    canvas.stroke_path(path);

    canvas.restore();
}

fn create_graph_path(sample_points: &[Vector2F], sample_spread: f32, offset: Vector2F) -> Path2D {
    let mut path = Path2D::new();
    path.move_to(sample_points[0] + Vector2F::new(0.0, 2.0));
    for pair in sample_points.windows(2) {
        path.bezier_curve_to(pair[0] + offset + Vector2F::new(sample_spread * 0.5, 0.0),
                             pair[1] + offset - Vector2F::new(sample_spread * 0.5, 0.0),
                             pair[1] + offset);
    }
    path
}

fn create_rounded_rect_path(rect: RectF, radius: f32) -> Path2D {
    let mut path = Path2D::new();
    path.move_to(rect.origin() + Vector2F::new(radius, 0.0));
    path.arc_to(rect.upper_right(), rect.upper_right() + Vector2F::new(0.0,  radius), radius);
    path.arc_to(rect.lower_right(), rect.lower_right() + Vector2F::new(-radius, 0.0), radius);
    path.arc_to(rect.lower_left(),  rect.lower_left()  + Vector2F::new(0.0, -radius), radius);
    path.arc_to(rect.origin(),      rect.origin()      + Vector2F::new(radius,  0.0), radius);
    path.close_path();
    path
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
    let window_size = Vector2I::new(800, 600);
    let window =
        video.window("NanoVG example port", window_size.x() as u32, window_size.y() as u32)
             .opengl()
             .build()
             .unwrap();

    // Create the GL context, and make it current.
    let gl_context = window.gl_create_context().unwrap();
    gl::load_with(|name| video.gl_get_proc_address(name) as *const _);
    window.gl_make_current(&gl_context).unwrap();

    // Create a Pathfinder renderer.
    let mut renderer = Renderer::new(GLDevice::new(GLVersion::GL3, 0),
                                     &FilesystemResourceLoader::locate(),
                                     DestFramebuffer::full_window(window_size),
                                     RendererOptions {
                                         background_color: Some(ColorF::new(0.3, 0.3, 0.32, 1.0)),
                                     });

    // Initialize state.
    let mut event_pump = sdl_context.event_pump().unwrap();
    let mut mouse_position = Vector2F::default();
    let start_time = Instant::now();
    let font_context = CanvasFontContext::from_system_source();

    // Enter the main loop.
    loop {
        // Make a canvas.
        let mut canvas = CanvasRenderingContext2D::new(font_context.clone(), window_size.to_f32());

        // Render the demo.
        let time = (Instant::now() - start_time).as_secs_f32();
        render_demo(&mut canvas, mouse_position, window_size.to_f32(), time);

        // Render the canvas to screen.
        let scene = SceneProxy::from_scene(canvas.into_scene(), RayonExecutor);
        scene.build_and_render(&mut renderer, BuildOptions::default());
        window.gl_swap_window();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => return,
                Event::MouseMotion { x, y, .. } => {
                    mouse_position = Vector2I::new(x, y).to_f32();
                }
                _ => {}
            }
        }
    }
}
