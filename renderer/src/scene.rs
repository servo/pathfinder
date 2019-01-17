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
use pathfinder_geometry::outline::Outline;
use pathfinder_geometry::transform3d::Perspective;
use pathfinder_geometry::transform::Transform2DF32;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

#[derive(Clone, Debug)]
pub struct Scene {
    pub objects: Vec<PathObject>,
    pub paints: Vec<Paint>,
    pub paint_cache: HashMap<Paint, PaintId>,
    pub bounds: Rect<f32>,
    pub view_box: Rect<f32>,
}

impl Scene {
    #[inline]
    pub fn new() -> Scene {
        Scene {
            objects: vec![],
            paints: vec![],
            paint_cache: HashMap::new(),
            bounds: Rect::zero(),
            view_box: Rect::zero(),
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

    pub fn build_objects_sequentially(&self, z_buffer: &ZBuffer) -> Vec<BuiltObject> {
        self.objects
            .iter()
            .enumerate()
            .map(|(object_index, object)| {
                let mut tiler = Tiler::new(
                    &object.outline,
                    &self.view_box,
                    object_index as u16,
                    ShaderId(object.paint.0),
                    z_buffer,
                );
                tiler.generate_tiles();
                tiler.built_object
            })
            .collect()
    }

    pub fn build_objects(&self, z_buffer: &ZBuffer) -> Vec<BuiltObject> {
        self.objects
            .par_iter()
            .enumerate()
            .map(|(object_index, object)| {
                let mut tiler = Tiler::new(
                    &object.outline,
                    &self.view_box,
                    object_index as u16,
                    ShaderId(object.paint.0),
                    z_buffer,
                );
                tiler.generate_tiles();
                tiler.built_object
            })
            .collect()
    }

    pub fn transform(&mut self, transform: &Transform2DF32) {
        let mut bounds = Rect::zero();
        for (object_index, object) in self.objects.iter_mut().enumerate() {
            object.outline.transform(transform);
            object.outline.clip_against_rect(&self.view_box);

            if object_index == 0 {
                bounds = *object.outline.bounds();
            } else {
                bounds = bounds.union(object.outline.bounds());
            }
        }

        //println!("new bounds={:?}", bounds);
        self.bounds = bounds;
    }

    pub fn apply_perspective(&mut self, perspective: &Perspective) {
        let mut bounds = Rect::zero();
        for (object_index, object) in self.objects.iter_mut().enumerate() {
            object.outline.apply_perspective(perspective);
            object.outline.clip_against_rect(&self.view_box);

            if object_index == 0 {
                bounds = *object.outline.bounds();
            } else {
                bounds = bounds.union(object.outline.bounds());
            }
        }

        //println!("new bounds={:?}", bounds);
        self.bounds = bounds;
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
}

#[inline]
pub fn scene_tile_index(tile_x: i16, tile_y: i16, tile_rect: Rect<i16>) -> u32 {
    (tile_y - tile_rect.origin.y) as u32 * tile_rect.size.width as u32
        + (tile_x - tile_rect.origin.x) as u32
}
