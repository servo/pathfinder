// pathfinder/canvas/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A simple API for Pathfinder that mirrors a subset of HTML canvas.

use pathfinder_color::ColorU;
use pathfinder_content::dash::OutlineDash;
use pathfinder_content::effects::BlendMode;
use pathfinder_content::fill::FillRule;
use pathfinder_content::gradient::Gradient;
use pathfinder_content::outline::{ArcDirection, Contour, Outline};
use pathfinder_content::pattern::Pattern;
use pathfinder_content::stroke::{LineCap, LineJoin as StrokeLineJoin};
use pathfinder_content::stroke::{OutlineStrokeToFill, StrokeStyle};
use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_geometry::vector::Vector2F;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_renderer::paint::{Paint, PaintId};
use pathfinder_renderer::scene::{ClipPath, ClipPathId, DrawPath, Scene};
use std::borrow::Cow;
use std::default::Default;
use std::f32::consts::PI;
use std::mem;
use std::sync::Arc;
use text::FontCollection;

#[cfg(feature = "pf-text")]
pub use text::TextMetrics;
pub use text::CanvasFontContext;

const HAIRLINE_STROKE_WIDTH: f32 = 0.0333;
const DEFAULT_FONT_SIZE: f32 = 10.0;

#[cfg_attr(not(feature = "pf-text"), path = "text_no_text.rs")]
mod text;

pub struct CanvasRenderingContext2D {
    scene: Scene,
    current_state: State,
    saved_states: Vec<State>,
    #[allow(dead_code)]
    font_context: CanvasFontContext,
}

impl CanvasRenderingContext2D {
    #[inline]
    pub fn new(font_context: CanvasFontContext, size: Vector2F) -> CanvasRenderingContext2D {
        let mut scene = Scene::new();
        scene.set_view_box(RectF::new(Vector2F::default(), size));
        CanvasRenderingContext2D::from_scene(font_context, scene)
    }

    pub fn from_scene(font_context: CanvasFontContext, scene: Scene) -> CanvasRenderingContext2D {
        #[cfg(feature = "pf-text")]
        let default_font_collection = font_context.default_font_collection.clone();
        #[cfg(not(feature = "pf-text"))]
        let default_font_collection = Arc::new(FontCollection);
        CanvasRenderingContext2D {
            scene,
            current_state: State::default(default_font_collection),
            saved_states: vec![],
            font_context,
        }
    }

    #[inline]
    pub fn into_scene(self) -> Scene {
        self.scene
    }

    // Drawing rectangles

    #[inline]
    pub fn fill_rect(&mut self, rect: RectF) {
        let mut path = Path2D::new();
        path.rect(rect);
        self.fill_path(path, FillRule::Winding);
    }

    #[inline]
    pub fn stroke_rect(&mut self, rect: RectF) {
        let mut path = Path2D::new();
        path.rect(rect);
        self.stroke_path(path);
    }

    pub fn clear_rect(&mut self, rect: RectF) {
        let mut path = Path2D::new();
        path.rect(rect);

        let mut outline = path.into_outline();
        outline.transform(&self.current_state.transform);

        let paint = Paint::transparent_black();
        let paint = self.current_state.resolve_paint(&paint);
        let paint_id = self.scene.push_paint(&paint);

        self.scene.push_path(DrawPath::new(outline,
                                           paint_id,
                                           None,
                                           FillRule::Winding,
                                           BlendMode::Clear,
                                           String::new()))
    }

    // Line styles

    #[inline]
    pub fn set_line_width(&mut self, new_line_width: f32) {
        self.current_state.line_width = new_line_width
    }

    #[inline]
    pub fn set_line_cap(&mut self, new_line_cap: LineCap) {
        self.current_state.line_cap = new_line_cap
    }

    #[inline]
    pub fn set_line_join(&mut self, new_line_join: LineJoin) {
        self.current_state.line_join = new_line_join
    }

    #[inline]
    pub fn set_miter_limit(&mut self, new_miter_limit: f32) {
        self.current_state.miter_limit = new_miter_limit
    }

    #[inline]
    pub fn set_line_dash(&mut self, mut new_line_dash: Vec<f32>) {
        // Duplicate and concatenate if an odd number of dashes are present.
        if new_line_dash.len() % 2 == 1 {
            let mut real_line_dash = new_line_dash.clone();
            real_line_dash.extend(new_line_dash.into_iter());
            new_line_dash = real_line_dash;
        }

        self.current_state.line_dash = new_line_dash
    }

    #[inline]
    pub fn set_line_dash_offset(&mut self, new_line_dash_offset: f32) {
        self.current_state.line_dash_offset = new_line_dash_offset
    }

    // Fill and stroke styles

    #[inline]
    pub fn set_fill_style(&mut self, new_fill_style: FillStyle) {
        self.current_state.fill_paint = new_fill_style.into_paint();
    }

    #[inline]
    pub fn set_stroke_style(&mut self, new_stroke_style: FillStyle) {
        self.current_state.stroke_paint = new_stroke_style.into_paint();
    }

    // Shadows

    #[inline]
    pub fn set_shadow_color(&mut self, new_shadow_color: ColorU) {
        self.current_state.shadow_paint = Paint::Color(new_shadow_color);
    }

    #[inline]
    pub fn set_shadow_offset(&mut self, new_shadow_offset: Vector2F) {
        self.current_state.shadow_offset = new_shadow_offset;
    }

    // Drawing paths

    #[inline]
    pub fn fill_path(&mut self, path: Path2D, fill_rule: FillRule) {
        let mut outline = path.into_outline();
        outline.transform(&self.current_state.transform);

        let paint = self.current_state.resolve_paint(&self.current_state.fill_paint);
        let paint_id = self.scene.push_paint(&paint);

        self.push_path(outline, paint_id, fill_rule);
    }

    #[inline]
    pub fn stroke_path(&mut self, path: Path2D) {
        let paint = self.current_state.resolve_paint(&self.current_state.stroke_paint);
        let paint_id = self.scene.push_paint(&paint);

        let mut stroke_style = self.current_state.resolve_stroke_style();
        
        // The smaller scale is relevant here, as we multiply by it and want to ensure it is always
        // bigger than `HAIRLINE_STROKE_WIDTH`.
        let transform_scale = f32::min(self.current_state.transform.m11(),
                                       self.current_state.transform.m22());
        // Avoid the division in the normal case of sufficient thickness.
        if stroke_style.line_width * transform_scale < HAIRLINE_STROKE_WIDTH {
            stroke_style.line_width = HAIRLINE_STROKE_WIDTH / transform_scale;
        }

        let mut outline = path.into_outline();
        if !self.current_state.line_dash.is_empty() {
            let mut dash = OutlineDash::new(&outline,
                                            &self.current_state.line_dash,
                                            self.current_state.line_dash_offset);
            dash.dash();
            outline = dash.into_outline();
        }

        let mut stroke_to_fill = OutlineStrokeToFill::new(&outline, stroke_style);
        stroke_to_fill.offset();
        outline = stroke_to_fill.into_outline();

        outline.transform(&self.current_state.transform);
        self.push_path(outline, paint_id, FillRule::Winding);
    }

    pub fn clip_path(&mut self, path: Path2D, fill_rule: FillRule) {
        let mut outline = path.into_outline();
        outline.transform(&self.current_state.transform);

        let clip_path_id = self.scene   
                               .push_clip_path(ClipPath::new(outline, fill_rule, String::new()));

        self.current_state.clip_path = Some(clip_path_id);
    }

    fn push_path(&mut self, outline: Outline, paint_id: PaintId, fill_rule: FillRule) {
        let clip_path = self.current_state.clip_path;
        let blend_mode = self.current_state.global_composite_operation.to_blend_mode();

        if !self.current_state.shadow_paint.is_fully_transparent() {
            let paint = self.current_state.resolve_paint(&self.current_state.shadow_paint);
            let paint_id = self.scene.push_paint(&paint);

            let mut outline = outline.clone();
            outline.transform(&Transform2F::from_translation(self.current_state.shadow_offset));
            self.scene.push_path(DrawPath::new(outline,
                                               paint_id,
                                               clip_path,
                                               fill_rule,
                                               blend_mode,
                                               String::new()))
        }

        self.scene.push_path(DrawPath::new(outline,
                                           paint_id,
                                           clip_path,
                                           fill_rule,
                                           blend_mode,
                                           String::new()))
    }

    // Transformations

    #[inline]
    pub fn current_transform(&self) -> Transform2F {
        self.current_state.transform
    }

    #[inline]
    pub fn set_current_transform(&mut self, new_transform: &Transform2F) {
        self.current_state.transform = *new_transform;
    }

    #[inline]
    pub fn reset_transform(&mut self) {
        self.current_state.transform = Transform2F::default();
    }

    // Compositing

    #[inline]
    pub fn global_alpha(&self) -> f32 {
        self.current_state.global_alpha
    }

    #[inline]
    pub fn set_global_alpha(&mut self, new_global_alpha: f32) {
        self.current_state.global_alpha = new_global_alpha;
    }

    #[inline]
    pub fn global_composite_operation(&self) -> CompositeOperation {
        self.current_state.global_composite_operation
    }

    #[inline]
    pub fn set_global_composite_operation(&mut self, new_composite_operation: CompositeOperation) {
        self.current_state.global_composite_operation = new_composite_operation;
    }

    // The canvas state

    #[inline]
    pub fn save(&mut self) {
        self.saved_states.push(self.current_state.clone());
    }

    #[inline]
    pub fn restore(&mut self) {
        if let Some(state) = self.saved_states.pop() {
            self.current_state = state;
        }
    }
}

#[derive(Clone)]
struct State {
    transform: Transform2F,
    font_collection: Arc<FontCollection>,
    font_size: f32,
    line_width: f32,
    line_cap: LineCap,
    line_join: LineJoin,
    miter_limit: f32,
    line_dash: Vec<f32>,
    line_dash_offset: f32,
    fill_paint: Paint,
    stroke_paint: Paint,
    shadow_paint: Paint,
    shadow_offset: Vector2F,
    text_align: TextAlign,
    global_alpha: f32,
    global_composite_operation: CompositeOperation,
    clip_path: Option<ClipPathId>,
}

impl State {
    fn default(default_font_collection: Arc<FontCollection>) -> State {
        State {
            transform: Transform2F::default(),
            font_collection: default_font_collection,
            font_size: DEFAULT_FONT_SIZE,
            line_width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 10.0,
            line_dash: vec![],
            line_dash_offset: 0.0,
            fill_paint: Paint::black(),
            stroke_paint: Paint::black(),
            shadow_paint: Paint::transparent_black(),
            shadow_offset: Vector2F::default(),
            text_align: TextAlign::Left,
            global_alpha: 1.0,
            global_composite_operation: CompositeOperation::SourceOver,
            clip_path: None,
        }
    }

    fn resolve_paint<'a>(&self, paint: &'a Paint) -> Cow<'a, Paint> {
        if self.global_alpha == 1.0 && (paint.is_color() || self.transform.is_identity()) {
            return Cow::Borrowed(paint);
        }

        let mut paint = (*paint).clone();
        paint.set_opacity(self.global_alpha);
        paint.apply_transform(&self.transform);
        Cow::Owned(paint)
    }

    fn resolve_stroke_style(&self) -> StrokeStyle {
        StrokeStyle {
            line_width: self.line_width,
            line_cap: self.line_cap,
            line_join: match self.line_join {
                LineJoin::Miter => StrokeLineJoin::Miter(self.miter_limit),
                LineJoin::Bevel => StrokeLineJoin::Bevel,
                LineJoin::Round => StrokeLineJoin::Round,
            },
        }
    }
}

#[derive(Clone)]
pub struct Path2D {
    outline: Outline,
    current_contour: Contour,
}

// TODO(pcwalton): `ellipse`
impl Path2D {
    #[inline]
    pub fn new() -> Path2D {
        Path2D { outline: Outline::new(), current_contour: Contour::new() }
    }

    #[inline]
    pub fn close_path(&mut self) {
        self.current_contour.close();
    }

    #[inline]
    pub fn move_to(&mut self, to: Vector2F) {
        // TODO(pcwalton): Cull degenerate contours.
        self.flush_current_contour();
        self.current_contour.push_endpoint(to);
    }

    #[inline]
    pub fn line_to(&mut self, to: Vector2F) {
        self.current_contour.push_endpoint(to);
    }

    #[inline]
    pub fn quadratic_curve_to(&mut self, ctrl: Vector2F, to: Vector2F) {
        self.current_contour.push_quadratic(ctrl, to);
    }

    #[inline]
    pub fn bezier_curve_to(&mut self, ctrl0: Vector2F, ctrl1: Vector2F, to: Vector2F) {
        self.current_contour.push_cubic(ctrl0, ctrl1, to);
    }

    #[inline]
    pub fn arc(&mut self,
               center: Vector2F,
               radius: f32,
               start_angle: f32,
               end_angle: f32,
               direction: ArcDirection) {
        let transform = Transform2F::from_scale(Vector2F::splat(radius)).translate(center);
        self.current_contour.push_arc(&transform, start_angle, end_angle, direction);
    }

    #[inline]
    pub fn arc_to(&mut self, ctrl: Vector2F, to: Vector2F, radius: f32) {
        // FIXME(pcwalton): What should we do if there's no initial point?
        let from = self.current_contour.last_position().unwrap_or_default();
        let (v0, v1) = (from - ctrl, to - ctrl);
        let (vu0, vu1) = (v0.normalize(), v1.normalize());
        let hypot = radius / f32::sqrt(0.5 * (1.0 - vu0.dot(vu1)));
        let bisector = vu0 + vu1;
        let center = ctrl + bisector.scale(hypot / bisector.length());

        let transform = Transform2F::from_scale(Vector2F::splat(radius)).translate(center);

        let chord = LineSegment2F::new(vu0.yx().scale_xy(Vector2F::new(-1.0, 1.0)),
                                      vu1.yx().scale_xy(Vector2F::new(1.0, -1.0)));

        // FIXME(pcwalton): Is clockwise direction correct?
        self.current_contour.push_arc_from_unit_chord(&transform, chord, ArcDirection::CW);
    }

    pub fn rect(&mut self, rect: RectF) {
        self.flush_current_contour();
        self.current_contour.push_endpoint(rect.origin());
        self.current_contour.push_endpoint(rect.upper_right());
        self.current_contour.push_endpoint(rect.lower_right());
        self.current_contour.push_endpoint(rect.lower_left());
        self.current_contour.close();
    }

    pub fn ellipse(&mut self,
                   center: Vector2F,
                   axes: Vector2F,
                   rotation: f32,
                   start_angle: f32,
                   end_angle: f32) {
        self.flush_current_contour();

        let transform = Transform2F::from_scale(axes).rotate(rotation).translate(center);
        self.current_contour.push_arc(&transform, start_angle, end_angle, ArcDirection::CW);

        if end_angle - start_angle >= 2.0 * PI {
            self.current_contour.close();
        }
    }

    fn into_outline(mut self) -> Outline {
        self.flush_current_contour();
        self.outline
    }

    fn flush_current_contour(&mut self) {
        if !self.current_contour.is_empty() {
            self.outline.push_contour(mem::replace(&mut self.current_contour, Contour::new()));
        }
    }
}

#[derive(Clone)]
pub enum FillStyle {
    Color(ColorU),
    Gradient(Gradient),
    Pattern(Pattern),
}

impl FillStyle {
    fn into_paint(self) -> Paint {
        match self {
            FillStyle::Color(color) => Paint::Color(color),
            FillStyle::Gradient(gradient) => Paint::Gradient(gradient),
            FillStyle::Pattern(pattern) => Paint::Pattern(pattern),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlign {
    Left,
    Right,
    Center,
}

// We duplicate `pathfinder_content::stroke::LineJoin` here because the HTML canvas API treats the
// miter limit as part of the canvas state, while the native Pathfinder API treats the miter limit
// as part of the line join. Pathfinder's choice is more logical, because the miter limit is
// specific to miter joins. In this API, however, for compatibility we go with the HTML canvas
// semantics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineJoin {
    Miter,
    Bevel,
    Round,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CompositeOperation {
    SourceOver,
    DestinationOver,
    DestinationOut,
    SourceAtop,
    Xor,
    Lighter,
    Lighten,
    Darken,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl CompositeOperation {
    fn to_blend_mode(self) -> BlendMode {
        match self {
            CompositeOperation::SourceOver => BlendMode::SrcOver,
            CompositeOperation::DestinationOver => BlendMode::DestOver,
            CompositeOperation::DestinationOut => BlendMode::DestOut,
            CompositeOperation::SourceAtop => BlendMode::SrcAtop,
            CompositeOperation::Xor => BlendMode::Xor,
            CompositeOperation::Lighter => BlendMode::Lighter,
            CompositeOperation::Lighten => BlendMode::Lighten,
            CompositeOperation::Darken => BlendMode::Darken,
            CompositeOperation::Hue => BlendMode::Hue,
            CompositeOperation::Saturation => BlendMode::Saturation,
            CompositeOperation::Color => BlendMode::Color,
            CompositeOperation::Luminosity => BlendMode::Luminosity,
        }
    }
}
