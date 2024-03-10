// pathfinder/canvas/src/text.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{CanvasRenderingContext2D, State, TextAlign, TextBaseline};
use font_kit::canvas::RasterizationOptions;
use font_kit::error::{FontLoadingError, SelectionError};
use font_kit::family_name::FamilyName;
use font_kit::handle::Handle;
use font_kit::hinting::HintingOptions;
use font_kit::loaders::default::Font;
use font_kit::properties::Properties;
use font_kit::source::{Source, SystemSource};
use font_kit::sources::mem::MemSource;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::util;
use pathfinder_geometry::vector::{Vector2F, vec2f};
use pathfinder_renderer::paint::PaintId;
use pathfinder_text::{FontContext, FontRenderOptions, TextRenderMode};
use skribo::{FontCollection, FontFamily, FontRef, Layout as SkriboLayout, TextStyle};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

impl CanvasRenderingContext2D {
    /// Fills the given text using the current style.
    ///
    /// As an extension, you may pass in the `TextMetrics` object returned by `measure_text()` to
    /// fill the text that you passed into `measure_text()` with the layout-related style
    /// properties set at the time you called that function. This allows Pathfinder to skip having
    /// to lay out the text again.
    pub fn fill_text<T>(&mut self, text: &T, position: Vector2F) where T: ToTextLayout + ?Sized {
        let paint = self.current_state.resolve_paint(&self.current_state.fill_paint);
        let paint_id = self.canvas.scene.push_paint(&paint);
        self.fill_or_stroke_text(text, position, paint_id, TextRenderMode::Fill);
    }

    /// Strokes the given text using the current style.
    ///
    /// As an extension, you may pass in the `TextMetrics` object returned by `measure_text()` to
    /// stroke the text that you passed into `measure_text()` with the layout-related style
    /// properties set at the time you called that function. This allows Pathfinder to skip having
    /// to lay out the text again.
    pub fn stroke_text<T>(&mut self, text: &T, position: Vector2F) where T: ToTextLayout + ?Sized {
        let paint = self.current_state.resolve_paint(&self.current_state.stroke_paint);
        let paint_id = self.canvas.scene.push_paint(&paint);
        let render_mode = TextRenderMode::Stroke(self.current_state.resolve_stroke_style());
        self.fill_or_stroke_text(text, position, paint_id, render_mode);
    }

    /// Returns metrics of the given text using the current style.
    ///
    /// As an extension, the returned `TextMetrics` object contains all the layout data for the
    /// string and can be used in its place when calling `fill_text()` and `stroke_text()` to avoid
    /// needlessly performing layout multiple times.
    pub fn measure_text<T>(&self, text: &T) -> TextMetrics where T: ToTextLayout + ?Sized {
        text.layout(CanvasState(&self.current_state)).into_owned()
    }

    fn fill_or_stroke_text<T>(&mut self,
                              text: &T,
                              mut position: Vector2F,
                              paint_id: PaintId,
                              render_mode: TextRenderMode)
                              where T: ToTextLayout + ?Sized {
        let layout = text.layout(CanvasState(&self.current_state));

        let clip_path = self.current_state.clip_path;
        let blend_mode = self.current_state.global_composite_operation.to_blend_mode();

        position += layout.text_origin();
        let transform = self.current_state.transform * Transform2F::from_translation(position);

        // TODO(pcwalton): Report errors.
        drop(self.canvas_font_context
                 .0
                 .borrow_mut()
                 .font_context
                 .push_layout(&mut self.canvas.scene,
                              &layout.skribo_layout,
                              &TextStyle { size: layout.font_size },
                              &FontRenderOptions {
                                  transform,
                                  render_mode,
                                  hinting_options: HintingOptions::None,
                                  clip_path,
                                  blend_mode,
                                  paint_id,
                              }));
    }

    // Text styles

    #[inline]
    pub fn font(&self) -> Arc<FontCollection> {
        self.current_state.font_collection.clone()
    }

    #[inline]
    pub fn set_font<FC>(&mut self, font_collection: FC) -> Result<(), FontError> where FC: IntoFontCollection {
        let font_collection = font_collection.into_font_collection(&self.canvas_font_context)?;
        self.current_state.font_collection = font_collection; 
        Ok(())
    }

    #[inline]
    pub fn font_size(&self) -> f32 {
        self.current_state.font_size
    }

    #[inline]
    pub fn set_font_size(&mut self, new_font_size: f32) {
        self.current_state.font_size = new_font_size;
    }

    #[inline]
    pub fn text_align(&self) -> TextAlign {
        self.current_state.text_align
    }

    #[inline]
    pub fn set_text_align(&mut self, new_text_align: TextAlign) {
        self.current_state.text_align = new_text_align;
    }

    #[inline]
    pub fn text_baseline(&self) -> TextBaseline {
        self.current_state.text_baseline
    }

    #[inline]
    pub fn set_text_baseline(&mut self, new_text_baseline: TextBaseline) {
        self.current_state.text_baseline = new_text_baseline;
    }
}

// Avoids leaking `State` to the outside.
#[doc(hidden)]
pub struct CanvasState<'a>(&'a State);

/// A trait that encompasses both text that has been laid out (i.e. `TextMetrics` or skribo's
/// `Layout`) and text that has not yet been laid out.
pub trait ToTextLayout {
    #[doc(hidden)]
    fn layout(&self, state: CanvasState) -> Cow<TextMetrics>;
}

impl ToTextLayout for str {
    fn layout(&self, state: CanvasState) -> Cow<TextMetrics> {
        let skribo_layout = Rc::new(skribo::layout(&TextStyle { size: state.0.font_size },
                                                   &state.0.font_collection,
                                                   self));
        Cow::Owned(TextMetrics::new(skribo_layout,
                                    state.0.font_size,
                                    state.0.text_align,
                                    state.0.text_baseline))
    }
}

impl ToTextLayout for String {
    fn layout(&self, state: CanvasState) -> Cow<TextMetrics> {
        let this: &str = self;
        this.layout(state)
    }
}

impl ToTextLayout for Rc<SkriboLayout> {
    fn layout(&self, state: CanvasState) -> Cow<TextMetrics> {
        Cow::Owned(TextMetrics::new((*self).clone(),
                                    state.0.font_size,
                                    state.0.text_align,
                                    state.0.text_baseline))
    }
}

impl ToTextLayout for TextMetrics {
    fn layout(&self, _: CanvasState) -> Cow<TextMetrics> {
        Cow::Borrowed(self)
    }
}

#[cfg(feature = "pf-text")]
#[derive(Clone)]
pub struct CanvasFontContext(pub(crate) Rc<RefCell<CanvasFontContextData>>);

/// The reason a font could not be loaded
#[derive(Debug)]
pub enum FontError {
    NotFound(SelectionError),
    LoadError(FontLoadingError),
}

pub(super) struct CanvasFontContextData {
    pub(super) font_context: FontContext<Font>,
    #[allow(dead_code)]
    pub(super) font_source: Arc<dyn Source>,
    #[allow(dead_code)]
    pub(super) default_font_collection: Arc<FontCollection>,
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

        CanvasFontContext(Rc::new(RefCell::new(CanvasFontContextData {
            font_source,
            default_font_collection: Arc::new(default_font_collection),
            font_context: FontContext::new(),
        })))
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

    fn get_font_by_postscript_name(&self, postscript_name: &str) -> Result<Font, FontError> {
        let this = self.0.borrow();
        if let Some(cached_font) = this.font_context.get_cached_font(postscript_name) {
            return Ok((*cached_font).clone());
        }
        this.font_source
            .select_by_postscript_name(postscript_name)
            .map_err(FontError::NotFound)?
            .load().map_err(FontError::LoadError)
    }
}

// Text layout utilities

/// A laid-out run of text. Text metrics can be queried from this structure, or it can be directly
/// passed into `fill_text()` and/or `stroke_text()` to draw the text without having to lay it out
/// again.
///
/// Internally, this structure caches most of its layout queries.
#[derive(Clone)]
pub struct TextMetrics {
    skribo_layout: Rc<SkriboLayout>,
    font_size: f32,
    align: TextAlign,
    baseline: TextBaseline,
    text_x_offset: Cell<Option<f32>>,
    text_y_offset: Cell<Option<f32>>,
    vertical_metrics: Cell<Option<VerticalMetrics>>,
    // The calculated width of a segment of inline text in pixels.
    width: Cell<Option<f32>>,
    // The distance from the typographic left side of the text to the left side of the bounding
    // rectangle of the given text, in pixels. The distance is measured parallel to the baseline.
    actual_left_extent: Cell<Option<f32>>,
    // The distance from the typographic right side of the text to the right side of the bounding
    // rectangle of the given text, in pixels. The distance is measured parallel to the baseline.
    actual_right_extent: Cell<Option<f32>>,
}

#[derive(Clone, Copy)]
struct VerticalMetrics {
    // The distance from the horizontal line indicated by the `text_baseline` state to the top of
    // the highest bounding rectangle of all the fonts used to render the text, in pixels.
    font_bounding_box_ascent: f32,
    // The distance from the horizontal line indicated by the `text_baseline` state to the bottom
    // of the highest bounding rectangle of all the fonts used to render the text, in pixels.
    font_bounding_box_descent: f32,
    // The distance from the horizontal line indicated by the `text_baseline` state to the top of
    // the bounding rectangle used to render the text, in pixels.
    actual_bounding_box_ascent: f32,
    // The distance from the horizontal line indicated by the `text_baseline` state to the bottom
    // of the bounding rectangle used to render the text, in pixels.
    actual_bounding_box_descent: f32,
    // The distance from the horizontal line indicated by the `text_baseline` state to the top of
    // the em square in the line box, in pixels.
    em_height_ascent: f32,
    // The distance from the horizontal line indicated by the `text_baseline` state to the bottom
    // of the em square in the line box, in pixels.
    em_height_descent: f32,
    // The distance from the horizontal line indicated by the `text_baseline` state to the hanging
    // baseline of the line box, in pixels.
    hanging_baseline: f32,
    // The distance from the horizontal line indicated by the `text_baseline` state to the
    // alphabetic baseline of the line box, in pixels.
    alphabetic_baseline: f32,
    // The distance from the horizontal line indicated by the `text_baseline` state to the
    // ideographic baseline of the line box, in pixels.
    ideographic_baseline: f32,
}

impl TextMetrics {
    pub fn new(skribo_layout: Rc<SkriboLayout>,
               font_size: f32,
               align: TextAlign,
               baseline: TextBaseline)
               -> TextMetrics {
        TextMetrics {
            skribo_layout,
            font_size,
            align,
            baseline,
            text_x_offset: Cell::new(None),
            text_y_offset: Cell::new(None),
            vertical_metrics: Cell::new(None),
            width: Cell::new(None),
            actual_left_extent: Cell::new(None),
            actual_right_extent: Cell::new(None),
        }
    }

    pub fn text_x_offset(&self) -> f32 {
        if self.text_x_offset.get().is_none() {
            self.text_x_offset.set(Some(match self.align {
                TextAlign::Left => 0.0,
                TextAlign::Right => -self.width(),
                TextAlign::Center => -0.5 * self.width(),
            }));
        }
        self.text_x_offset.get().unwrap()
    }

    pub fn text_y_offset(&self) -> f32 {
        self.populate_vertical_metrics_if_necessary();
        if self.text_y_offset.get().is_none() {
            let vertical_metrics = self.vertical_metrics.get().unwrap();
            self.text_y_offset.set(Some(match self.baseline {
                TextBaseline::Alphabetic => 0.0,
                TextBaseline::Top => vertical_metrics.em_height_ascent,
                TextBaseline::Middle => {
                    util::lerp(vertical_metrics.em_height_ascent,
                               vertical_metrics.em_height_descent,
                               0.5)
                }
                TextBaseline::Bottom => vertical_metrics.em_height_descent,
                TextBaseline::Ideographic => vertical_metrics.ideographic_baseline,
                TextBaseline::Hanging => vertical_metrics.hanging_baseline,
            }));
        }
        self.text_y_offset.get().unwrap()
    }

    fn text_origin(&self) -> Vector2F {
        vec2f(self.text_x_offset(), self.text_y_offset())
    }

    pub fn width(&self) -> f32 {
        if self.width.get().is_none() {
            match self.skribo_layout.glyphs.last() {
                None => self.width.set(Some(0.0)),
                Some(last_glyph) => {
                    let glyph_id = last_glyph.glyph_id;
                    let font_metrics = last_glyph.font.font.metrics();
                    let scale_factor = self.skribo_layout.size / font_metrics.units_per_em as f32;
                    let glyph_rect = last_glyph.font.font.typographic_bounds(glyph_id).unwrap();
                    self.width.set(Some(last_glyph.offset.x() +
                                        glyph_rect.max_x() * scale_factor));
                }
            }

        }
        self.width.get().unwrap()
    }

    fn populate_vertical_metrics_if_necessary(&self) {
        if self.vertical_metrics.get().is_none() {
            self.vertical_metrics.set(Some(VerticalMetrics::measure(&self.skribo_layout)));
        }
    }

    pub fn font_bounding_box_ascent(&self) -> f32 {
        self.populate_vertical_metrics_if_necessary();
        self.vertical_metrics.get().unwrap().font_bounding_box_ascent - self.text_y_offset()
    }

    pub fn font_bounding_box_descent(&self) -> f32 {
        self.populate_vertical_metrics_if_necessary();
        self.vertical_metrics.get().unwrap().font_bounding_box_descent - self.text_y_offset()
    }

    pub fn actual_bounding_box_ascent(&self) -> f32 {
        self.populate_vertical_metrics_if_necessary();
        self.vertical_metrics.get().unwrap().actual_bounding_box_ascent - self.text_y_offset()
    }

    pub fn actual_bounding_box_descent(&self) -> f32 {
        self.populate_vertical_metrics_if_necessary();
        self.vertical_metrics.get().unwrap().actual_bounding_box_descent - self.text_y_offset()
    }

    pub fn em_height_ascent(&self) -> f32 {
        self.populate_vertical_metrics_if_necessary();
        self.vertical_metrics.get().unwrap().em_height_ascent - self.text_y_offset()
    }

    pub fn em_height_descent(&self) -> f32 {
        self.populate_vertical_metrics_if_necessary();
        self.vertical_metrics.get().unwrap().em_height_descent - self.text_y_offset()
    }

    pub fn actual_bounding_box_left(&self) -> f32 {
        if self.actual_left_extent.get().is_none() {
            match self.skribo_layout.glyphs.get(0) {
                None => self.actual_left_extent.set(Some(0.0)),
                Some(first_glyph) => {
                    let glyph_id = first_glyph.glyph_id;
                    let font_metrics = first_glyph.font.font.metrics();
                    let scale_factor = self.skribo_layout.size / font_metrics.units_per_em as f32;
                    let glyph_rect = first_glyph.font.font.raster_bounds(
                        glyph_id,
                        font_metrics.units_per_em as f32,
                        Transform2F::default(),
                        HintingOptions::None,
                        RasterizationOptions::GrayscaleAa).unwrap();
                    self.actual_left_extent.set(Some(first_glyph.offset.x() +
                                                     glyph_rect.min_x() as f32 * scale_factor));
                }
            }
        }
        self.actual_left_extent.get().unwrap() + self.text_x_offset()
    }

    pub fn actual_bounding_box_right(&self) -> f32 {
        if self.actual_right_extent.get().is_none() {
            match self.skribo_layout.glyphs.last() {
                None => self.actual_right_extent.set(Some(0.0)),
                Some(last_glyph) => {
                    let glyph_id = last_glyph.glyph_id;
                    let font_metrics = last_glyph.font.font.metrics();
                    let scale_factor = self.skribo_layout.size / font_metrics.units_per_em as f32;
                    let glyph_rect = last_glyph.font.font.raster_bounds(
                        glyph_id,
                        font_metrics.units_per_em as f32,
                        Transform2F::default(),
                        HintingOptions::None,
                        RasterizationOptions::GrayscaleAa).unwrap();
                    self.actual_right_extent.set(Some(last_glyph.offset.x() +
                                                      glyph_rect.max_x() as f32 * scale_factor));
                }
            }
        }
        self.actual_right_extent.get().unwrap() + self.text_x_offset()
    }

    pub fn hanging_baseline(&self) -> f32 {
        self.populate_vertical_metrics_if_necessary();
        self.vertical_metrics.get().unwrap().hanging_baseline - self.text_y_offset()
    }

    pub fn alphabetic_baseline(&self) -> f32 {
        self.populate_vertical_metrics_if_necessary();
        self.vertical_metrics.get().unwrap().alphabetic_baseline - self.text_y_offset()
    }

    pub fn ideographic_baseline(&self) -> f32 {
        self.populate_vertical_metrics_if_necessary();
        self.vertical_metrics.get().unwrap().ideographic_baseline - self.text_y_offset()
    }

}

impl VerticalMetrics {
    fn measure(skribo_layout: &SkriboLayout) -> VerticalMetrics {
        let mut vertical_metrics = VerticalMetrics {
            font_bounding_box_ascent: 0.0,
            font_bounding_box_descent: 0.0,
            actual_bounding_box_ascent: 0.0,
            actual_bounding_box_descent: 0.0,
            em_height_ascent: 0.0,
            em_height_descent: 0.0,
            hanging_baseline: 0.0,
            alphabetic_baseline: 0.0,
            ideographic_baseline: 0.0,
        };

        let mut last_font: Option<Arc<Font>> = None;
        for glyph in &skribo_layout.glyphs {
            match last_font {
                Some(ref last_font) if Arc::ptr_eq(&last_font, &glyph.font.font) => {}
                _ => {
                    let font = glyph.font.font.clone();

                    let font_metrics = font.metrics();
                    let scale_factor = skribo_layout.size / font_metrics.units_per_em as f32;
                    vertical_metrics.em_height_ascent =
                        (font_metrics.ascent *
                         scale_factor).max(vertical_metrics.em_height_ascent);
                    vertical_metrics.em_height_descent =
                        (font_metrics.descent *
                         scale_factor).min(vertical_metrics.em_height_descent);
                    vertical_metrics.font_bounding_box_ascent =
                        (font_metrics.bounding_box.max_y() *
                         scale_factor).max(vertical_metrics.font_bounding_box_ascent);
                    vertical_metrics.font_bounding_box_descent =
                        (font_metrics.bounding_box.min_y() *
                         scale_factor).min(vertical_metrics.font_bounding_box_descent);

                    last_font = Some(font);
                }
            }

            let font = last_font.as_ref().unwrap();
            let glyph_rect = font.raster_bounds(glyph.glyph_id,
                                                skribo_layout.size,
                                                Transform2F::default(),
                                                HintingOptions::None,
                                                RasterizationOptions::GrayscaleAa).unwrap();
            vertical_metrics.actual_bounding_box_ascent =
                (glyph_rect.max_y() as f32).max(vertical_metrics.actual_bounding_box_ascent);
            vertical_metrics.actual_bounding_box_descent =
                (glyph_rect.min_y() as f32).min(vertical_metrics.actual_bounding_box_descent);
        }

        vertical_metrics
    }
}

/// Various things that can be conveniently converted into font collections for use with
/// `CanvasRenderingContext2D::set_font()`.
pub trait IntoFontCollection {
    fn into_font_collection(self, font_context: &CanvasFontContext) -> Result<Arc<FontCollection>, FontError>;
}

impl IntoFontCollection for Arc<FontCollection> {
    #[inline]
    fn into_font_collection(self, _: &CanvasFontContext) -> Result<Arc<FontCollection>, FontError> {
        Ok(self)
    }
}

impl IntoFontCollection for FontFamily {
    #[inline]
    fn into_font_collection(self, _: &CanvasFontContext) -> Result<Arc<FontCollection>, FontError> {
        let mut font_collection = FontCollection::new();
        font_collection.add_family(self);
        Ok(Arc::new(font_collection))
    }
}

impl IntoFontCollection for Vec<FontFamily> {
    #[inline]
    fn into_font_collection(self, _: &CanvasFontContext) -> Result<Arc<FontCollection>, FontError> {
        let mut font_collection = FontCollection::new();
        for family in self {
            font_collection.add_family(family);
        }
        Ok(Arc::new(font_collection))
    }
}

impl IntoFontCollection for Font {
    #[inline]
    fn into_font_collection(self, context: &CanvasFontContext) -> Result<Arc<FontCollection>, FontError> {
        Ok(FontFamily::new_from_font(self).into_font_collection(context)?)
    }
}

impl<'a> IntoFontCollection for &'a [Font] {
    #[inline]
    fn into_font_collection(self, context: &CanvasFontContext) -> Result<Arc<FontCollection>, FontError> {
        let mut family = FontFamily::new();
        for font in self {
            family.add_font(FontRef::new((*font).clone()))
        }
        family.into_font_collection(context)
    }
}

impl<'a> IntoFontCollection for &'a str {
    #[inline]
    fn into_font_collection(self, context: &CanvasFontContext) -> Result<Arc<FontCollection>, FontError> {
        context.get_font_by_postscript_name(self)?.into_font_collection(context)
    }
}

impl<'a, 'b> IntoFontCollection for &'a [&'b str] {
    #[inline]
    fn into_font_collection(self, context: &CanvasFontContext) -> Result<Arc<FontCollection>, FontError> {
        let mut font_collection = FontCollection::new();
        for postscript_name in self {
            let font = context.get_font_by_postscript_name(postscript_name)?;
            font_collection.add_family(FontFamily::new_from_font(font));
        }
        Ok(Arc::new(font_collection))
    }
}
