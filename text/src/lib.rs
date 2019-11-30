// pathfinder/text/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use euclid::Angle;
use euclid::default::{Point2D, Vector2D};
use font_kit::error::GlyphLoadingError;
use font_kit::hinting::HintingOptions;
use font_kit::loader::Loader;
use lyon_path::builder::{FlatPathBuilder, PathBuilder, Build};
use pathfinder_content::outline::{Contour, Outline};
use pathfinder_content::stroke::{OutlineStrokeToFill, StrokeStyle};
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::Vector2F;
use pathfinder_renderer::paint::PaintId;
use pathfinder_renderer::scene::{PathObject, Scene};
use skribo::{FontCollection, Layout, TextStyle};
use std::mem;

pub trait SceneExt {
    // TODO(pcwalton): Support stroked glyphs.
    fn push_glyph<F>(&mut self,
                     font: &F,
                     glyph_id: u32,
                     transform: &Transform2F,
                     render_mode: TextRenderMode,
                     hinting_options: HintingOptions,
                     paint_id: PaintId)
                     -> Result<(), GlyphLoadingError>
                     where F: Loader;

    fn push_layout(&mut self,
                   layout: &Layout,
                   style: &TextStyle,
                   transform: &Transform2F,
                   render_mode: TextRenderMode,
                   hinting_options: HintingOptions,
                   paint_id: PaintId)
                   -> Result<(), GlyphLoadingError>;

    fn push_text(&mut self,
                 text: &str,
                 style: &TextStyle,
                 collection: &FontCollection,
                 transform: &Transform2F,
                 render_mode: TextRenderMode,
                 hinting_options: HintingOptions,
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

        self.push_path(PathObject::new(outline, paint_id, String::new()));
        Ok(())
    }

    fn push_layout(&mut self,
                   layout: &Layout,
                   style: &TextStyle,
                   transform: &Transform2F,
                   render_mode: TextRenderMode,
                   hinting_options: HintingOptions,
                   paint_id: PaintId)
                   -> Result<(), GlyphLoadingError> {
        for glyph in &layout.glyphs {
            let offset = Vector2F::new(glyph.offset.x, glyph.offset.y);
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
                 paint_id: PaintId)
                 -> Result<(), GlyphLoadingError> {
        let layout = skribo::layout(style, collection, text);
        self.push_layout(&layout, style, &transform, render_mode, hinting_options, paint_id)
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

    fn convert_point(&self, point: Point2D<f32>) -> Vector2F {
        self.transform * Vector2F::new(point.x, point.y)
    }
}

impl PathBuilder for OutlinePathBuilder {
    fn quadratic_bezier_to(&mut self, ctrl: Point2D<f32>, to: Point2D<f32>) {
        let (ctrl, to) = (self.convert_point(ctrl), self.convert_point(to));
        self.current_contour.push_quadratic(ctrl, to);
    }

    fn cubic_bezier_to(&mut self, ctrl0: Point2D<f32>, ctrl1: Point2D<f32>, to: Point2D<f32>) {
        let (ctrl0, ctrl1) = (self.convert_point(ctrl0), self.convert_point(ctrl1));
        let to = self.convert_point(to);
        self.current_contour.push_cubic(ctrl0, ctrl1, to);
    }

    fn arc(&mut self,
           _center: Point2D<f32>,
           _radii: Vector2D<f32>,
           _sweep_angle: Angle<f32>,
           _x_rotation: Angle<f32>) {
        // TODO(pcwalton): Arcs.
    }
}

impl Build for OutlinePathBuilder {
    type PathType = Outline;
    fn build(mut self) -> Outline {
        self.flush_current_contour();
        self.outline
    }
    
    fn build_and_reset(&mut self) -> Outline {
        self.flush_current_contour();
        mem::replace(&mut self.outline, Outline::new())
    }

}

impl FlatPathBuilder for OutlinePathBuilder {

    fn move_to(&mut self, to: Point2D<f32>) {
        self.flush_current_contour();
        let to = self.convert_point(to);
        self.current_contour.push_endpoint(to);
    }

    fn line_to(&mut self, to: Point2D<f32>) {
        let to = self.convert_point(to);
        self.current_contour.push_endpoint(to);
    }

    fn close(&mut self) {
        self.current_contour.close();
    }

    fn current_position(&self) -> Point2D<f32> {
        if self.current_contour.is_empty() {
            return Point2D::new(0.0, 0.0)
        }

        let point_index = if self.current_contour.is_closed() {
            0
        } else {
            self.current_contour.len() - 1
        };

        let point = self.current_contour.position_of(point_index);
        Point2D::new(point.x(), point.y())
    }

    }
