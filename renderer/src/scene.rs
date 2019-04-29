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

use crate::builder::{PreparedRenderOptions, PreparedRenderTransform};
use hashbrown::HashMap;
use pathfinder_geometry::basic::point::{Point2DF32, Point3DF32};
use pathfinder_geometry::basic::rect::RectF32;
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_geometry::color::ColorU;
use pathfinder_geometry::outline::Outline;
use std::fmt::{self, Debug, Formatter};

#[derive(Clone)]
pub struct Scene {
    pub objects: Vec<PathObject>,
    pub paints: Vec<Paint>,
    pub paint_cache: HashMap<Paint, PaintId>,
    pub bounds: RectF32,
    pub view_box: RectF32,
}

impl Scene {
    #[inline]
    pub fn new() -> Scene {
        Scene {
            objects: vec![],
            paints: vec![],
            paint_cache: HashMap::new(),
            bounds: RectF32::default(),
            view_box: RectF32::default(),
        }
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

    pub fn build_descriptor(&self, built_options: &PreparedRenderOptions) -> SceneDescriptor {
        SceneDescriptor {
            shaders: self.build_shaders(),
            bounding_quad: built_options.bounding_quad(),
            object_count: self.objects.len(),
        }
    }

    fn build_shaders(&self) -> Vec<ObjectShader> {
        self.objects.iter().map(|object| {
            let paint = &self.paints[object.paint.0 as usize];
            ObjectShader { fill_color: paint.color }
        }).collect()
    }

    pub(crate) fn apply_render_options(&self,
                                       original_outline: &Outline,
                                       options: &PreparedRenderOptions)
                                       -> Outline {
        let effective_view_box = self.effective_view_box(options);

        let mut outline;
        match options.transform {
            PreparedRenderTransform::Perspective { ref perspective, ref clip_polygon, .. } => {
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
                        transform = transform.post_mul(&Transform2DF32::from_scale(
                            &Point2DF32::new(3.0, 1.0)))
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
        if self.objects.is_empty() {
            return None;
        }
        let first_paint_id = self.objects[0].paint;
        if self.objects.iter().skip(1).any(|object| object.paint != first_paint_id) {
            return None;
        }
        Some(self.paints[first_paint_id.0 as usize].color)
    }

    #[inline]
    pub fn effective_view_box(&self, render_options: &PreparedRenderOptions) -> RectF32 {
        if render_options.subpixel_aa_enabled {
            self.view_box.scale_xy(Point2DF32::new(3.0, 1.0))
        } else {
            self.view_box
        }
    }
}

impl Debug for Scene {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        writeln!(formatter,
                 "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\">",
                 self.view_box.origin().x(),
                 self.view_box.origin().y(),
                 self.view_box.size().x(),
                 self.view_box.size().y())?;
        for object in &self.objects {
            let paint = &self.paints[object.paint.0 as usize];
            write!(formatter, "    <path")?;
            if !object.name.is_empty() {
                write!(formatter, " id=\"{}\"", object.name)?;
            }
            writeln!(formatter, " fill=\"{:?}\" d=\"{:?}\" />", paint.color, object.outline)?;
        }
        writeln!(formatter, "</svg>")?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct SceneDescriptor {
    pub shaders: Vec<ObjectShader>,
    pub bounding_quad: [Point3DF32; 4],
    pub object_count: usize,
}

#[derive(Clone, Debug)]
pub struct PathObject {
    outline: Outline,
    paint: PaintId,
    name: String,
    kind: PathObjectKind,
}

#[derive(Clone, Copy, Debug)]
pub enum PathObjectKind {
    Fill,
    Stroke,
}

impl PathObject {
    #[inline]
    pub fn new(outline: Outline, paint: PaintId, name: String, kind: PathObjectKind)
               -> PathObject {
        PathObject { outline, paint, name, kind }
    }

    #[inline]
    pub fn outline(&self) -> &Outline {
        &self.outline
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Paint {
    pub color: ColorU,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PaintId(pub u16);

#[derive(Clone, Copy, Debug, Default)]
pub struct ObjectShader {
    pub fill_color: ColorU,
}
