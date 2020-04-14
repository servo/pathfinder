// pathfinder/text/src/lib.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use font_kit::error::GlyphLoadingError;
use font_kit::hinting::HintingOptions;
use font_kit::loader::Loader;
use font_kit::outline::OutlineSink;
use pathfinder_content::effects::BlendMode;
use pathfinder_content::outline::{Contour, Outline};
use pathfinder_content::stroke::{OutlineStrokeToFill, StrokeStyle};
use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::Vector2F;
use pathfinder_renderer::paint::PaintId;
use pathfinder_renderer::scene::{ClipPathId, DrawPath, Scene};
use skribo::{FontCollection, Layout, TextStyle};
use std::mem;

// FIXME(pcwalton): Too many parameters!
pub trait SceneExt {
    // TODO(pcwalton): Support stroked glyphs.
    fn push_glyph<F>(&mut self,
                     font: &F,
                     glyph_id: u32,
                     transform: &Transform2F,
                     render_mode: TextRenderMode,
                     hinting_options: HintingOptions,
                     clip_path: Option<ClipPathId>,
                     blend_mode: BlendMode,
                     paint_id: PaintId)
                     -> Result<(), GlyphLoadingError>
                     where F: Loader;

    fn push_layout(&mut self,
                   layout: &Layout,
                   style: &TextStyle,
                   transform: &Transform2F,
                   render_mode: TextRenderMode,
                   hinting_options: HintingOptions,
                   clip_path: Option<ClipPathId>,
                   blend_mode: BlendMode,
                   paint_id: PaintId)
                   -> Result<(), GlyphLoadingError>;

    fn push_text(&mut self,
                 text: &str,
                 style: &TextStyle,
                 collection: &FontCollection,
                 transform: &Transform2F,
                 render_mode: TextRenderMode,
                 hinting_options: HintingOptions,
                 clip_path: Option<ClipPathId>,
                 blend_mode: BlendMode,
                 paint_id: PaintId)
                 -> Result<(), GlyphLoadingError>;
}

impl SceneExt for Scene {
    #[inline]
    fn push_glyph<F>(&mut self,
                     font: &F,
                     glyph_id: u32,
                     transform: &Transform2F,
                     render_mode: TextRenderMode,
                     hinting_options: HintingOptions,
                     clip_path: Option<ClipPathId>,
                     blend_mode: BlendMode,
                     paint_id: PaintId)
                     -> Result<(), GlyphLoadingError>
                     where F: Loader {
        let mut outline_builder = OutlinePathBuilder::new(transform);
        font.outline(glyph_id, hinting_options, &mut outline_builder)?;
        let mut outline = outline_builder.build();

        if let TextRenderMode::Stroke(stroke_style) = render_mode {
            let mut stroke_to_fill = OutlineStrokeToFill::new(&outline, stroke_style);
            stroke_to_fill.offset();
            outline = stroke_to_fill.into_outline();
        }

        let mut path = DrawPath::new(outline, paint_id);
        path.set_clip_path(clip_path);
        path.set_blend_mode(blend_mode);

        self.push_path(path);
        Ok(())
    }

    fn push_layout(&mut self,
                   layout: &Layout,
                   style: &TextStyle,
                   transform: &Transform2F,
                   render_mode: TextRenderMode,
                   hinting_options: HintingOptions,
                   clip_path: Option<ClipPathId>,
                   blend_mode: BlendMode,
                   paint_id: PaintId)
                   -> Result<(), GlyphLoadingError> {
        for glyph in &layout.glyphs {
            let offset = glyph.offset;
            let font = &*glyph.font.font;
            // FIXME(pcwalton): Cache this!
            let scale = style.size / (font.metrics().units_per_em as f32);
            let scale = Vector2F::new(scale, -scale);
            let transform = *transform * Transform2F::from_scale(scale).translate(offset);
            self.push_glyph(font,
                            glyph.glyph_id,
                            &transform,
                            render_mode,
                            hinting_options,
                            clip_path,
                            blend_mode,
                            paint_id)?;
        }
        Ok(())
    }

    #[inline]
    fn push_text(&mut self,
                 text: &str,
                 style: &TextStyle,
                 collection: &FontCollection,
                 transform: &Transform2F,
                 render_mode: TextRenderMode,
                 hinting_options: HintingOptions,
                 clip_path: Option<ClipPathId>,
                 blend_mode: BlendMode,
                 paint_id: PaintId)
                 -> Result<(), GlyphLoadingError> {
        let layout = skribo::layout(style, collection, text);
        self.push_layout(&layout,
                         style,
                         &transform,
                         render_mode,
                         hinting_options,
                         clip_path,
                         blend_mode,
                         paint_id)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TextRenderMode {
    Fill,
    Stroke(StrokeStyle),
}

struct OutlinePathBuilder {
    outline: Outline,
    current_contour: Contour,
    transform: Transform2F,
}

impl OutlinePathBuilder {
    fn new(transform: &Transform2F) -> OutlinePathBuilder {
        OutlinePathBuilder {
            outline: Outline::new(),
            current_contour: Contour::new(),
            transform: *transform,
        }
    }

    fn flush_current_contour(&mut self) {
        if !self.current_contour.is_empty() {
            self.outline.push_contour(mem::replace(&mut self.current_contour, Contour::new()));
        }
    }

    fn build(mut self) -> Outline {
        self.flush_current_contour();
        self.outline
    }
}

impl OutlineSink for OutlinePathBuilder {
    fn move_to(&mut self, to: Vector2F) {
        self.flush_current_contour();
        self.current_contour.push_endpoint(self.transform * to);
    }

    fn line_to(&mut self, to: Vector2F) {
        self.current_contour.push_endpoint(self.transform * to);
    }

    fn quadratic_curve_to(&mut self, ctrl: Vector2F, to: Vector2F) {
        self.current_contour.push_quadratic(self.transform * ctrl, self.transform * to);
    }

    fn cubic_curve_to(&mut self, ctrl: LineSegment2F, to: Vector2F) {
        self.current_contour.push_cubic(self.transform * ctrl.from(),
                                        self.transform * ctrl.to(),
                                        self.transform * to);
    }

    fn close(&mut self) {
        self.current_contour.close();
    }
}
