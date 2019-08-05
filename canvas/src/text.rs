// pathfinder/canvas/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use pathfinder_geometry::vector::Vector2F;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_renderer::paint::PaintId;
use std::sync::Arc;
use std::iter;
use font_kit::family_name::FamilyName;
use font_kit::handle::Handle;
use font_kit::hinting::HintingOptions;
use font_kit::properties::Properties;
use font_kit::source::{Source, SystemSource};
use font_kit::sources::mem::MemSource;
pub use skribo::{FontCollection, FontFamily, Layout, TextStyle};
use pathfinder_text::{SceneExt, TextRenderMode};
pub use font_kit::loaders::default::Font;
pub use crate::{CanvasRenderingContext2D, TextAlign};

impl CanvasRenderingContext2D {
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

    pub fn fill_layout(&mut self, layout: &Layout, transform: Transform2F) {
        let paint_id = self.scene.push_paint(&self.current_state.fill_paint);
        drop(self.scene.push_layout(&layout,
                                    &TextStyle { size: self.current_state.font_size },
                                    &(transform * self.current_state.transform),
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

        let transform = self.current_state.transform * Transform2F::from_translation(position);

        // TODO(pcwalton): Report errors.
        drop(self.scene.push_layout(&layout,
                                    &TextStyle { size: self.current_state.font_size },
                                    &transform,
                                    render_mode,
                                    HintingOptions::None,
                                    paint_id));
    }

    fn layout_text(&self, string: &str) -> Layout {
        skribo::layout(&TextStyle { size: self.current_state.font_size },
                       &self.current_state.font_collection,
                       string)
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
}

// TODO(pcwalton): Support other fields.
#[derive(Clone, Copy, Debug)]
pub struct TextMetrics {
    pub width: f32,
}

#[derive(Clone)]
pub struct CanvasFontContext {
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

pub trait LayoutExt {
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
