// pathfinder/renderer/src/builder.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Packs data onto the GPU.

use crate::gpu_data::{AlphaTileBatchPrimitive, RenderCommand};
use crate::scene::Scene;
use crate::tiles::Tiler;
use crate::z_buffer::ZBuffer;
use pathfinder_geometry::basic::point::{Point2DF32, Point3DF32};
use pathfinder_geometry::basic::rect::RectF32;
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_geometry::basic::transform3d::Perspective;
use pathfinder_geometry::clip::PolygonClipper3D;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::sync::atomic::AtomicUsize;
use std::u16;

pub trait RenderCommandListener: Send + Sync {
    fn send(&self, command: RenderCommand);
}

pub struct SceneBuilder<'a> {
    scene: &'a Scene,
    built_options: &'a PreparedRenderOptions,

    pub(crate) next_alpha_tile_index: AtomicUsize,
    pub(crate) z_buffer: ZBuffer,
    pub(crate) listener: Box<dyn RenderCommandListener>,
}

impl<'a> SceneBuilder<'a> {
    pub fn new(scene: &'a Scene,
               built_options: &'a PreparedRenderOptions,
               listener: Box<dyn RenderCommandListener>)
               -> SceneBuilder<'a> {
        let effective_view_box = scene.effective_view_box(built_options);
        SceneBuilder {
            scene,
            built_options,

            next_alpha_tile_index: AtomicUsize::new(0),
            z_buffer: ZBuffer::new(effective_view_box),
            listener,
        }
    }

    pub fn build_sequentially(&mut self) {
        let effective_view_box = self.scene.effective_view_box(self.built_options);
        let object_count = self.scene.objects.len();
        let alpha_tiles: Vec<_> = (0..object_count).into_iter().flat_map(|object_index| {
            self.build_object(object_index,
                              effective_view_box,
                              &self.built_options,
                              &self.scene)
        }).collect();

        self.finish_building(alpha_tiles)
    }

    pub fn build_in_parallel(&mut self) {
        let effective_view_box = self.scene.effective_view_box(self.built_options);
        let object_count = self.scene.objects.len();
        let alpha_tiles: Vec<_> = (0..object_count).into_par_iter().flat_map(|object_index| {
            self.build_object(object_index,
                              effective_view_box,
                              &self.built_options,
                              &self.scene)
        }).collect();

        self.finish_building(alpha_tiles)
    }

    fn build_object(&self,
                    object_index: usize,
                    view_box: RectF32,
                    built_options: &PreparedRenderOptions,
                    scene: &Scene)
                    -> Vec<AlphaTileBatchPrimitive> {
        let object = &scene.objects[object_index];
        let outline = scene.apply_render_options(object.outline(), built_options);

        let mut tiler = Tiler::new(self, &outline, view_box, object_index as u16);
        tiler.generate_tiles();

        self.listener.send(RenderCommand::AddFills(tiler.built_object.fills));
        tiler.built_object.alpha_tiles
    }

    fn cull_alpha_tiles(&self, alpha_tiles: &mut Vec<AlphaTileBatchPrimitive>) {
        for alpha_tile in alpha_tiles {
            let alpha_tile_coords = alpha_tile.tile_coords();
            if self.z_buffer.test(alpha_tile_coords, alpha_tile.object_index as u32) {
                continue;
            }

            // FIXME(pcwalton): Clean this up.
            alpha_tile.tile_x_lo = 0xff;
            alpha_tile.tile_y_lo = 0xff;
            alpha_tile.tile_hi = 0xff;
        }
    }

    fn pack_alpha_tiles(&mut self, alpha_tiles: Vec<AlphaTileBatchPrimitive>) {
        let object_count = self.scene.objects.len() as u32;
        let solid_tiles = self.z_buffer.build_solid_tiles(0..object_count);
        if !solid_tiles.is_empty() {
            self.listener.send(RenderCommand::SolidTile(solid_tiles));
        }
        if !alpha_tiles.is_empty() {
            self.listener.send(RenderCommand::AlphaTile(alpha_tiles));
        }
    }

    fn finish_building(&mut self, mut alpha_tiles: Vec<AlphaTileBatchPrimitive>) {
        self.listener.send(RenderCommand::FlushFills);
        self.cull_alpha_tiles(&mut alpha_tiles);
        self.pack_alpha_tiles(alpha_tiles);
    }
}

#[derive(Clone, Default)]
pub struct RenderOptions {
    pub transform: RenderTransform,
    pub dilation: Point2DF32,
    pub subpixel_aa_enabled: bool,
}

impl RenderOptions {
    pub fn prepare(self, bounds: RectF32) -> PreparedRenderOptions {
        PreparedRenderOptions {
            transform: self.transform.prepare(bounds),
            dilation: self.dilation,
            subpixel_aa_enabled: self.subpixel_aa_enabled,
        }
    }
}

#[derive(Clone)]
pub enum RenderTransform {
    Transform2D(Transform2DF32),
    Perspective(Perspective),
}

impl Default for RenderTransform {
    #[inline]
    fn default() -> RenderTransform {
        RenderTransform::Transform2D(Transform2DF32::default())
    }
}

impl RenderTransform {
    fn prepare(&self, bounds: RectF32) -> PreparedRenderTransform {
        let perspective = match self {
            RenderTransform::Transform2D(ref transform) => {
                if transform.is_identity() {
                    return PreparedRenderTransform::None;
                }
                return PreparedRenderTransform::Transform2D(*transform);
            }
            RenderTransform::Perspective(ref perspective) => *perspective,
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

        // Compute depth.
        let quad = [
            points[0].perspective_divide(),
            points[1].perspective_divide(),
            points[2].perspective_divide(),
            points[3].perspective_divide(),
        ];
        //println!("... PERSPECTIVE-DIVIDED points = {:?}", quad);

        points = PolygonClipper3D::new(points).clip();
        //println!("... CLIPPED quad={:?}", points);
        for point in &mut points {
            *point = point.perspective_divide()
        }

        let inverse_transform = perspective.transform.inverse();
        let clip_polygon = points.into_iter().map(|point| {
            inverse_transform.transform_point(point).perspective_divide().to_2d()
        }).collect();
        return PreparedRenderTransform::Perspective { perspective, clip_polygon, quad };
    }
}

pub struct PreparedRenderOptions {
    pub transform: PreparedRenderTransform,
    pub dilation: Point2DF32,
    pub subpixel_aa_enabled: bool,
}

impl PreparedRenderOptions {
    #[inline]
    pub fn bounding_quad(&self) -> [Point3DF32; 4] {
        match self.transform {
            PreparedRenderTransform::Perspective { quad, .. } => quad,
            _ => [Point3DF32::default(); 4],
        }
    }
}

pub enum PreparedRenderTransform {
    None,
    Transform2D(Transform2DF32),
    Perspective { perspective: Perspective, clip_polygon: Vec<Point2DF32>, quad: [Point3DF32; 4] }
}

impl PreparedRenderTransform {
    #[inline]
    pub fn is_2d(&self) -> bool {
        match *self {
            PreparedRenderTransform::Transform2D(_) => true,
            _ => false,
        }
    }
}

impl<F> RenderCommandListener for F where F: Fn(RenderCommand) + Send + Sync {
    #[inline]
    fn send(&self, command: RenderCommand) { (*self)(command) }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TileStats {
    pub solid_tile_count: u32,
    pub alpha_tile_count: u32,
}
