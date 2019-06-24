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

use font_kit::family_name::FamilyName;
use font_kit::handle::Handle;
use font_kit::hinting::HintingOptions;
use font_kit::loaders::default::Font;
use font_kit::properties::Properties;
use font_kit::source::{Source, SystemSource};
use font_kit::sources::mem::MemSource;
use pathfinder_geometry::basic::line_segment::LineSegment2F;
use pathfinder_geometry::basic::vector::Vector2F;
use pathfinder_geometry::basic::rect::RectF;
use pathfinder_geometry::basic::transform2d::Transform2DF;
use pathfinder_geometry::color::ColorU;
use pathfinder_geometry::dash::OutlineDash;
use pathfinder_geometry::outline::{ArcDirection, Contour, Outline};
use pathfinder_geometry::stroke::{LineCap, LineJoin as StrokeLineJoin};
use pathfinder_geometry::stroke::{OutlineStrokeToFill, StrokeStyle};
use pathfinder_renderer::paint::{Paint, PaintId};
use pathfinder_renderer::scene::{PathObject, Scene};
use pathfinder_text::{SceneExt, TextRenderMode};
use skribo::{FontCollection, FontFamily, Layout, TextStyle};
use std::default::Default;
use std::f32::consts::PI;
use std::iter;
use std::mem;
use std::sync::Arc;

const HAIRLINE_STROKE_WIDTH: f32 = 0.0333;
const DEFAULT_FONT_SIZE: f32 = 10.0;

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
        CanvasRenderingContext2D {
            scene,
            current_state: State::default(font_context.default_font_collection.clone()),
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
        self.fill_path(path);
    }

    #[inline]
    pub fn stroke_rect(&mut self, rect: RectF) {
        let mut path = Path2D::new();
        path.rect(rect);
        self.stroke_path(path);
    }

    // Drawing text

    pub fn fill_text(&mut self, string: &str, position: Vector2F) {
        let paint_id = self.scene.push_paint(&self.current_state.fill_paint);
        self.fill_or_stroke_text(string, position, paint_id, TextRenderMode::Fill);
    }

    pub fn stroke_text(&mut self, string: &str, position: Vector2F) {
        let paint_id = self.scene.push_paint(&self.current_state.stroke_paint);
        let render_mode = TextRenderMode::Stroke(self.current_state.resolve_stroke_style());
        self.fill_or_stroke_text(string, position, paint_id, render_mode);
    }

    pub fn measure_text(&self, string: &str) -> TextMetrics {
        TextMetrics { width: self.layout_text(string).width() }
    }
    
    pub fn fill_layout(&mut self, layout: &Layout, transform: Transform2DF) {
        let paint_id = self.scene.push_paint(&self.current_state.fill_paint);
        drop(self.scene.push_layout(&layout,
                                    &TextStyle { size: self.current_state.font_size },
                                    &transform.post_mul(&self.current_state.transform),
                                    TextRenderMode::Fill,
                                    HintingOptions::None,
                                    paint_id));
    }
    
    fn fill_or_stroke_text(&mut self,
                           string: &str,
                           mut position: Vector2F,
                           paint_id: PaintId,
                           render_mode: TextRenderMode) {
        let layout = self.layout_text(string);

        match self.current_state.text_align {
            TextAlign::Left => {},
            TextAlign::Right => position.set_x(position.x() - layout.width()),
            TextAlign::Center => position.set_x(position.x() - layout.width() * 0.5),
        }
        
        let transform = Transform2DF::from_translation(position).post_mul(&self.current_state
                                                                               .transform);

        // TODO(pcwalton): Report errors.
        drop(self.scene.push_layout(&layout,
                                    &TextStyle { size: self.current_state.font_size },
                                    &transform,
                                    render_mode,
                                    HintingOptions::None,
                                    paint_id));
    }

    pub fn layout_text(&self, string: &str) -> Layout {
        skribo::layout(&TextStyle { size: self.current_state.font_size },
                       &self.current_state.font_collection,
                       string)
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

    // Text styles

    #[inline]
    pub fn set_font_collection(&mut self, font_collection: Arc<FontCollection>) {
        self.current_state.font_collection = font_collection;
    }

    #[inline]
    pub fn set_font_families<I>(&mut self, font_families: I) where I: Iterator<Item = FontFamily> {
        let mut font_collection = FontCollection::new();
        for font_family in font_families {
            font_collection.add_family(font_family);
        }
        self.current_state.font_collection = Arc::new(font_collection);
    }

    /// A convenience method to set a single font family.
    #[inline]
    pub fn set_font_family(&mut self, font_family: FontFamily) {
        self.set_font_families(iter::once(font_family))
    }

    /// A convenience method to set a single font family consisting of a single font.
    #[inline]
    pub fn set_font(&mut self, font: Font) {
        self.set_font_family(FontFamily::new_from_font(font))
    }

    /// A convenience method to set a single font family consisting of a font
    /// described by a PostScript name.
    #[inline]
    pub fn set_font_by_postscript_name(&mut self, postscript_name: &str) {
        let font = self.font_context.font_source.select_by_postscript_name(postscript_name);
        self.set_font(font.expect("Didn't find the font!").load().unwrap());
    }

    #[inline]
    pub fn set_font_size(&mut self, new_font_size: f32) {
        self.current_state.font_size = new_font_size;
    }

    #[inline]
    pub fn set_text_align(&mut self, new_text_align: TextAlign) {
        self.current_state.text_align = new_text_align;
    }

    // Fill and stroke styles

    #[inline]
    pub fn set_fill_style(&mut self, new_fill_style: FillStyle) {
        self.current_state.fill_paint = new_fill_style.to_paint();
    }

    #[inline]
    pub fn set_stroke_style(&mut self, new_stroke_style: FillStyle) {
        self.current_state.stroke_paint = new_stroke_style.to_paint();
    }

    // Shadows

    #[inline]
    pub fn set_shadow_color(&mut self, new_shadow_color: ColorU) {
        self.current_state.shadow_paint = Paint { color: new_shadow_color };
    }

    #[inline]
    pub fn set_shadow_offset(&mut self, new_shadow_offset: Vector2F) {
        self.current_state.shadow_offset = new_shadow_offset;
    }

    // Drawing paths

    #[inline]
    pub fn fill_path(&mut self, path: Path2D) {
        let mut outline = path.into_outline();
        outline.transform(&self.current_state.transform);

        let paint = self.current_state.resolve_paint(self.current_state.fill_paint);
        let paint_id = self.scene.push_paint(&paint);

        self.push_path(outline, paint_id);
    }

    #[inline]
    pub fn stroke_path(&mut self, path: Path2D) {
        let paint = self.current_state.resolve_paint(self.current_state.stroke_paint);
        let paint_id = self.scene.push_paint(&paint);

        let mut stroke_style = self.current_state.resolve_stroke_style();
        stroke_style.line_width = f32::max(stroke_style.line_width, HAIRLINE_STROKE_WIDTH);

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
        self.push_path(outline, paint_id);
    }

    fn push_path(&mut self, outline: Outline, paint_id: PaintId) {
        if !self.current_state.shadow_paint.is_fully_transparent() {
            let paint = self.current_state.resolve_paint(self.current_state.shadow_paint);
            let paint_id = self.scene.push_paint(&paint);

            let mut outline = outline.clone();
            outline.transform(&Transform2DF::from_translation(self.current_state.shadow_offset));
            self.scene.push_path(PathObject::new(outline, paint_id, String::new()))
        }

        self.scene.push_path(PathObject::new(outline, paint_id, String::new()))
    }

    // Transformations

    #[inline]
    pub fn current_transform(&self) -> Transform2DF {
        self.current_state.transform
    }

    #[inline]
    pub fn set_current_transform(&mut self, new_transform: &Transform2DF) {
        self.current_state.transform = *new_transform;
    }

    #[inline]
    pub fn reset_transform(&mut self) {
        self.current_state.transform = Transform2DF::default();
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
    transform: Transform2DF,
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
}

impl State {
    fn default(default_font_collection: Arc<FontCollection>) -> State {
        State {
            transform: Transform2DF::default(),
            font_collection: default_font_collection,
            font_size: DEFAULT_FONT_SIZE,
            line_width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 10.0,
            line_dash: vec![],
            line_dash_offset: 0.0,
            fill_paint: Paint { color: ColorU::black() },
            stroke_paint: Paint { color: ColorU::black() },
            shadow_paint: Paint { color: ColorU::transparent_black() },
            shadow_offset: Vector2F::default(),
            text_align: TextAlign::Left,
            global_alpha: 1.0,
        }
    }

    fn resolve_paint(&self, mut paint: Paint) -> Paint {
        paint.color.a = (paint.color.a as f32 * self.global_alpha).round() as u8;
        paint
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
        let mut transform = Transform2DF::from_scale(Vector2F::splat(radius));
        transform = transform.post_mul(&Transform2DF::from_translation(center));
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

        let mut transform = Transform2DF::from_scale(Vector2F::splat(radius));
        transform = transform.post_mul(&Transform2DF::from_translation(center));

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

        let mut transform = Transform2DF::from_rotation(rotation);
        transform = transform.post_mul(&Transform2DF::from_scale(axes));
        transform = transform.post_mul(&Transform2DF::from_translation(center));
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

// TODO(pcwalton): Gradients.
#[derive(Clone, Copy)]
pub enum FillStyle {
    Color(ColorU),
}

impl FillStyle {
    #[inline]
    fn to_paint(&self) -> Paint {
        match *self { FillStyle::Color(color) => Paint { color } }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlign {
    Left,
    Right,
    Center,
}

// We duplicate `pathfinder_geometry::stroke::LineJoin` here because the HTML canvas API treats the
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

// TODO(pcwalton): Support other fields.
#[derive(Clone, Copy, Debug)]
pub struct TextMetrics {
    pub width: f32,
}

#[derive(Clone)]
pub struct CanvasFontContext {
    #[allow(dead_code)]
    font_source: Arc<dyn Source>,
    #[allow(dead_code)]
    default_font_collection: Arc<FontCollection>,
}

impl CanvasFontContext {
    pub fn new(font_source: Arc<dyn Source>) -> CanvasFontContext {
        let mut default_font_collection = FontCollection::new();
        if let Ok(default_font) = font_source.select_best_match(&[FamilyName::SansSerif],
                                                                &Properties::new()) {
            if let Ok(default_font) = default_font.load() {
                default_font_collection.add_family(FontFamily::new_from_font(default_font));
            }
        }

        CanvasFontContext {
            font_source,
            default_font_collection: Arc::new(default_font_collection),
        }
    }

    /// A convenience method to create a font context with the system source.
    /// This allows usage of fonts installed on the system.
    pub fn from_system_source() -> CanvasFontContext {
        CanvasFontContext::new(Arc::new(SystemSource::new()))
    }

    /// A convenience method to create a font context with a set of in-memory fonts.
    pub fn from_fonts<I>(fonts: I) -> CanvasFontContext where I: Iterator<Item = Handle> {
        CanvasFontContext::new(Arc::new(MemSource::from_fonts(fonts).unwrap()))
    }
}

// Text layout utilities

trait LayoutExt {
    fn width(&self) -> f32;
}

impl LayoutExt for Layout {
    fn width(&self) -> f32 {
        let last_glyph = match self.glyphs.last() {
            None => return 0.0,
            Some(last_glyph) => last_glyph,
        };

        let glyph_id = last_glyph.glyph_id;
        let font_metrics = last_glyph.font.font.metrics();
        let glyph_rect = last_glyph.font.font.typographic_bounds(glyph_id).unwrap();
        let scale_factor = self.size / font_metrics.units_per_em as f32;
        last_glyph.offset.x + glyph_rect.max_x() * scale_factor
    }
}
