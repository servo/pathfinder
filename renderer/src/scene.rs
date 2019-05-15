// pathfinder/renderer/src/scene.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A set of paths to be rendered.

use crate::builder::SceneBuilder;
use crate::concurrent::executor::Executor;
use crate::options::{PreparedRenderOptions, PreparedRenderTransform};
use crate::options::{RenderCommandListener, RenderOptions};
use crate::paint::{Paint, PaintId};
use hashbrown::HashMap;
use pathfinder_geometry::basic::point::Point2DF32;
use pathfinder_geometry::basic::rect::RectF32;
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_geometry::color::ColorU;
use pathfinder_geometry::outline::Outline;
use std::io::{self, Write};

#[derive(Clone)]
pub struct Scene {
    pub(crate) paths: Vec<PathObject>,
    pub(crate) paints: Vec<Paint>,
    paint_cache: HashMap<Paint, PaintId>,
    bounds: RectF32,
    view_box: RectF32,
}

impl Scene {
    #[inline]
    pub fn new() -> Scene {
        Scene {
            paths: vec![],
            paints: vec![],
            paint_cache: HashMap::new(),
            bounds: RectF32::default(),
            view_box: RectF32::default(),
        }
    }

    pub fn push_path(&mut self, path: PathObject) {
        self.bounds = self.bounds.union_rect(path.outline.bounds());
        self.paths.push(path);
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn push_paint(&mut self, paint: &Paint) -> PaintId {
        if let Some(paint_id) = self.paint_cache.get(paint) {
            return *paint_id;
        }

        let paint_id = PaintId(self.paints.len() as u16);
        self.paint_cache.insert(*paint, paint_id);
        self.paints.push(*paint);
        paint_id
    }

    #[inline]
    pub fn path_count(&self) -> usize {
        self.paths.len()
    }

    #[inline]
    pub fn bounds(&self) -> RectF32 {
        self.bounds
    }

    #[inline]
    pub fn set_bounds(&mut self, new_bounds: RectF32) {
        self.bounds = new_bounds;
    }

    #[inline]
    pub fn view_box(&self) -> RectF32 {
        self.view_box
    }

    #[inline]
    pub fn set_view_box(&mut self, new_view_box: RectF32) {
        self.view_box = new_view_box;
    }

    pub(crate) fn apply_render_options(
        &self,
        original_outline: &Outline,
        options: &PreparedRenderOptions,
    ) -> Outline {
        let effective_view_box = self.effective_view_box(options);

        let mut outline;
        match options.transform {
            PreparedRenderTransform::Perspective {
                ref perspective,
                ref clip_polygon,
                ..
            } => {
                if original_outline.is_outside_polygon(clip_polygon) {
                    outline = Outline::new();
                } else {
                    outline = (*original_outline).clone();
                    outline.clip_against_polygon(clip_polygon);
                    outline.apply_perspective(perspective);

                    // TODO(pcwalton): Support subpixel AA in 3D.
                }
            }
            _ => {
                // TODO(pcwalton): Short circuit.
                outline = (*original_outline).clone();
                if options.transform.is_2d() || options.subpixel_aa_enabled {
                    let mut transform = match options.transform {
                        PreparedRenderTransform::Transform2D(transform) => transform,
                        PreparedRenderTransform::None => Transform2DF32::default(),
                        PreparedRenderTransform::Perspective { .. } => unreachable!(),
                    };
                    if options.subpixel_aa_enabled {
                        transform = transform
                            .post_mul(&Transform2DF32::from_scale(Point2DF32::new(3.0, 1.0)))
                    }
                    outline.transform(&transform);
                }
                outline.clip_against_rect(effective_view_box);
            }
        }

        if !options.dilation.is_zero() {
            outline.dilate(options.dilation);
        }

        // TODO(pcwalton): Fold this into previous passes to avoid unnecessary clones during
        // monotonic conversion.
        outline.prepare_for_tiling(self.effective_view_box(options));
        outline
    }

    pub fn monochrome_color(&self) -> Option<ColorU> {
        if self.paths.is_empty() {
            return None;
        }

        let first_paint_id = self.paths[0].paint;
        if self
            .paths
            .iter()
            .skip(1)
            .any(|path_object| path_object.paint != first_paint_id) {
            return None;
        }
        Some(self.paints[first_paint_id.0 as usize].color)
    }

    #[inline]
    pub(crate) fn effective_view_box(&self, render_options: &PreparedRenderOptions) -> RectF32 {
        if render_options.subpixel_aa_enabled {
            self.view_box.scale_xy(Point2DF32::new(3.0, 1.0))
        } else {
            self.view_box
        }
    }

    #[inline]
    pub fn build<E>(&self,
                    options: RenderOptions,
                    listener: Box<dyn RenderCommandListener>,
                    executor: &E)
                    where E: Executor {
        let prepared_options = options.prepare(self.bounds);
        SceneBuilder::new(self, &prepared_options, listener).build(executor)
    }

    pub fn write_svg<W>(&self, writer: &mut W) -> io::Result<()> where W: Write {
        writeln!(
            writer,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\">",
            self.view_box.origin().x(),
            self.view_box.origin().y(),
            self.view_box.size().x(),
            self.view_box.size().y()
        )?;
        for path_object in &self.paths {
            let paint = &self.paints[path_object.paint.0 as usize];
            write!(writer, "    <path")?;
            if !path_object.name.is_empty() {
                write!(writer, " id=\"{}\"", path_object.name)?;
            }
            writeln!(
                writer,
                " fill=\"{:?}\" d=\"{:?}\" />",
                paint.color, path_object.outline
            )?;
        }
        writeln!(writer, "</svg>")?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct PathObject {
    outline: Outline,
    paint: PaintId,
    name: String,
}

impl PathObject {
    #[inline]
    pub fn new(outline: Outline, paint: PaintId, name: String) -> PathObject {
        PathObject { outline, paint, name }
    }

    #[inline]
    pub fn outline(&self) -> &Outline {
        &self.outline
    }

    #[inline]
    pub(crate) fn paint(&self) -> PaintId {
        self.paint
    }
}
