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

use crate::gpu_data::BuiltObject;
use crate::paint::{ObjectShader, Paint, PaintId, ShaderId};
use crate::tiles::Tiler;
use crate::z_buffer::ZBuffer;
use euclid::Rect;
use hashbrown::HashMap;
use pathfinder_geometry::basic::point::Point2DF32;
use pathfinder_geometry::basic::rect::RectF32;
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_geometry::basic::transform3d::Perspective;
use pathfinder_geometry::clip::PolygonClipper3D;
use pathfinder_geometry::outline::Outline;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
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

    pub fn build_shaders(&self) -> Vec<ObjectShader> {
        self.paints
            .iter()
            .map(|paint| ObjectShader {
                fill_color: paint.color,
            })
            .collect()
    }

    pub fn build_objects_sequentially(&self, build_transform: &BuildTransform, z_buffer: &ZBuffer)
                                      -> Vec<BuiltObject> {
        let build_transform = build_transform.prepare(self.bounds);
        self.objects
            .iter()
            .enumerate()
            .map(|(object_index, object)| {
                let outline = self.apply_build_transform(&object.outline, &build_transform);
                let mut tiler = Tiler::new(
                    &outline,
                    self.view_box,
                    object_index as u16,
                    ShaderId(object.paint.0),
                    z_buffer,
                );
                tiler.generate_tiles();
                tiler.built_object
            })
            .collect()
    }

    pub fn build_objects(&self, build_transform: &BuildTransform, z_buffer: &ZBuffer)
                         -> Vec<BuiltObject> {
        let build_transform = build_transform.prepare(self.bounds);
        self.objects
            .par_iter()
            .enumerate()
            .map(|(object_index, object)| {
                let outline = self.apply_build_transform(&object.outline, &build_transform);
                let mut tiler = Tiler::new(
                    &outline,
                    self.view_box,
                    object_index as u16,
                    ShaderId(object.paint.0),
                    z_buffer,
                );
                tiler.generate_tiles();
                tiler.built_object
            })
            .collect()
    }

    fn apply_build_transform(&self, outline: &Outline, build_transform: &PreparedBuildTransform)
                             -> Outline {
        // FIXME(pcwalton): Don't clone?
        let mut outline = (*outline).clone();
        match *build_transform {
            PreparedBuildTransform::Perspective(ref perspective, ref quad) => {
                outline.clip_against_polygon(quad);
                outline.apply_perspective(perspective);
                outline.prepare_for_tiling(self.view_box);
            }
            PreparedBuildTransform::Transform2D(ref transform) => {
                outline.transform(transform);
                outline.clip_against_rect(self.view_box);
            }
            PreparedBuildTransform::None => {
                outline.clip_against_rect(self.view_box);
            }
        }
        outline.prepare_for_tiling(self.view_box);
        outline
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
        PathObject {
            outline,
            paint,
            name,
            kind,
        }
    }

    #[inline]
    pub fn outline(&self) -> &Outline {
        &self.outline
    }
}

#[inline]
pub fn scene_tile_index(tile_x: i16, tile_y: i16, tile_rect: Rect<i16>) -> u32 {
    (tile_y - tile_rect.origin.y) as u32 * tile_rect.size.width as u32
        + (tile_x - tile_rect.origin.x) as u32
}

pub enum BuildTransform {
    None,
    Transform2D(Transform2DF32),
    Perspective(Perspective),
}

impl BuildTransform {
    fn prepare(&self, bounds: RectF32) -> PreparedBuildTransform {
        let perspective = match self {
            BuildTransform::None => return PreparedBuildTransform::None,
            BuildTransform::Transform2D(ref transform) => {
                return PreparedBuildTransform::Transform2D(*transform)
            }
            BuildTransform::Perspective(ref perspective) => *perspective,
        };

        let mut points = vec![
            bounds.origin().to_3d(),
            bounds.upper_right().to_3d(),
            bounds.lower_right().to_3d(),
            bounds.lower_left().to_3d(),
        ];
        //println!("-----");
        //println!("bounds={:?} ORIGINAL quad={:?}", self.bounds, points);
        for point in &mut points {
            *point = perspective.transform.transform_point(*point);
        }
        //println!("... PERSPECTIVE quad={:?}", points);
        points = PolygonClipper3D::new(points).clip();
        //println!("... CLIPPED quad={:?}", points);
        for point in &mut points {
            *point = point.perspective_divide()
        }
        let inverse_transform = perspective.transform.inverse();
        let points = points.into_iter().map(|point| {
            inverse_transform.transform_point(point).perspective_divide().to_2d()
        }).collect();
        PreparedBuildTransform::Perspective(perspective, points)
    }
}

enum PreparedBuildTransform {
    None,
    Transform2D(Transform2DF32),
    Perspective(Perspective, Vec<Point2DF32>),
}
