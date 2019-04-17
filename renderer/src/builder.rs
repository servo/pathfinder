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

use crate::cca_vec::ConcurrentCopyableArrayVec;
use crate::gpu_data::{AlphaTileBatchPrimitive, BuiltObject, FillBatchPrimitive, RenderCommand};
use crate::scene::Scene;
use crate::tiles::Tiler;
use crate::z_buffer::ZBuffer;
use pathfinder_geometry::basic::point::{Point2DF32, Point3DF32};
use pathfinder_geometry::basic::rect::RectF32;
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_geometry::basic::transform3d::Perspective;
use pathfinder_geometry::clip::PolygonClipper3D;
use pathfinder_geometry::distortion::BarrelDistortionCoefficients;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::sync::Arc;
use std::u16;

// Must be a power of two.
pub const MAX_FILLS_PER_BATCH: u32 = 0x1000;

const MAX_FILLS_PER_RUN: u32 = 0x100000;
const MAX_ALPHA_TILES_PER_RUN: u32 = 0x80000;

pub struct SceneBuilderContext {
    pub alpha_tiles: ConcurrentCopyableArrayVec<AlphaTileBatchPrimitive>,
}

pub trait RenderCommandListener: Send + Sync {
    fn send(&self, command: RenderCommand);
}

impl SceneBuilderContext {
    #[inline]
    pub fn new() -> SceneBuilderContext {
        SceneBuilderContext {
            alpha_tiles: ConcurrentCopyableArrayVec::new(MAX_ALPHA_TILES_PER_RUN),
        }
    }
}

pub struct SceneBuilder<'ctx, 'a> {
    context: &'ctx mut SceneBuilderContext,
    scene: &'a Scene,
    built_options: &'a PreparedRenderOptions,
}

impl<'ctx, 'a> SceneBuilder<'ctx, 'a> {
    pub fn new(context: &'ctx mut SceneBuilderContext,
               scene: &'a Scene,
               built_options: &'a PreparedRenderOptions)
               -> SceneBuilder<'ctx, 'a> {
        SceneBuilder { context, scene, built_options }
    }

    pub fn build_sequentially(&mut self, listener: Box<dyn RenderCommandListener>) {
        let effective_view_box = self.scene.effective_view_box(self.built_options);
        let z_buffer = ZBuffer::new(effective_view_box);

        listener.send(RenderCommand::ClearMaskFramebuffer);

        let object_count = self.scene.objects.len();
        for object_index in 0..object_count {
            build_object(object_index,
                         effective_view_box,
                         self.context,
                         &z_buffer,
                         &*listener,
                         &self.built_options,
                         &self.scene);
        }

        self.cull_alpha_tiles(&z_buffer);
        self.pack_alpha_tiles(&z_buffer, listener);
    }

    pub fn build_in_parallel(&mut self, listener: Box<dyn RenderCommandListener>) {
        let effective_view_box = self.scene.effective_view_box(self.built_options);
        let z_buffer = ZBuffer::new(effective_view_box);

        listener.send(RenderCommand::ClearMaskFramebuffer);

        let object_count = self.scene.objects.len();
        (0..object_count).into_par_iter().for_each(|object_index| {
            build_object(object_index,
                         effective_view_box,
                         self.context,
                         &z_buffer,
                         &*listener,
                         &self.built_options,
                         &self.scene);
        });

        self.cull_alpha_tiles(&z_buffer);
        self.pack_alpha_tiles(&z_buffer, listener);
    }

    fn pack_alpha_tiles(&mut self,
                        z_buffer: &ZBuffer,
                        listener: Box<dyn RenderCommandListener>) {
        let alpha_tiles = &self.context.alpha_tiles;

        let object_count = self.scene.objects.len() as u32;
        let solid_tiles = z_buffer.build_solid_tiles(0..object_count);

        if !solid_tiles.is_empty() {
            listener.send(RenderCommand::SolidTile(solid_tiles));
        }

        if !alpha_tiles.is_empty() {
            let mut tiles = alpha_tiles.to_vec();
            tiles.sort_unstable_by(|tile_a, tile_b| tile_a.object_index.cmp(&tile_b.object_index));

            listener.send(RenderCommand::AlphaTile(tiles));
            alpha_tiles.clear();
        }
    }

    fn cull_alpha_tiles(&mut self, z_buffer: &ZBuffer) {
        for alpha_tile_index in 0..self.context.alpha_tiles.committed_len() {
            let mut alpha_tile = self.context.alpha_tiles.get(alpha_tile_index);
            let alpha_tile_coords = alpha_tile.tile_coords();
            if z_buffer.test(alpha_tile_coords, alpha_tile.object_index as u32) {
                continue;
            }

            // FIXME(pcwalton): Hack!
            alpha_tile.tile_x_lo = 0xff;
            alpha_tile.tile_y_lo = 0xff;
            alpha_tile.tile_hi = 0xff;
            self.context.alpha_tiles.set(alpha_tile_index, alpha_tile);
        }
    }
}

fn build_object(object_index: usize,
                view_box: RectF32,
                context: &SceneBuilderContext,
                z_buffer: &ZBuffer,
                listener: &dyn RenderCommandListener,
                built_options: &PreparedRenderOptions,
                scene: &Scene) {
    let object = &scene.objects[object_index];
    let outline = scene.apply_render_options(object.outline(), built_options);

    let mut tiler = Tiler::new(context,
                               z_buffer,
                               listener,
                               &outline,
                               view_box,
                               object_index as u16);
    tiler.generate_tiles();
    listener.send(RenderCommand::Fill(tiler.built_object.fills));
}

#[derive(Clone, Default)]
pub struct RenderOptions {
    pub transform: RenderTransform,
    pub dilation: Point2DF32,
    pub barrel_distortion: Option<BarrelDistortionCoefficients>,
    pub subpixel_aa_enabled: bool,
}

impl RenderOptions {
    pub fn prepare(self, bounds: RectF32) -> PreparedRenderOptions {
        PreparedRenderOptions {
            transform: self.transform.prepare(bounds),
            dilation: self.dilation,
            barrel_distortion: self.barrel_distortion,
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
    pub barrel_distortion: Option<BarrelDistortionCoefficients>,
    pub subpixel_aa_enabled: bool,
}

impl PreparedRenderOptions {
    #[inline]
    pub fn quad(&self) -> [Point3DF32; 4] {
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
