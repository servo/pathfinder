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

use crate::gpu_data::{Batch, BuiltObject, FillBatchPrimitive};
use crate::gpu_data::{MaskTileBatchPrimitive, SolidTileScenePrimitive};
use crate::scene;
use crate::tiles;
use crate::z_buffer::ZBuffer;
use pathfinder_geometry::basic::point::{Point2DF32, Point3DF32};
use pathfinder_geometry::basic::rect::{RectF32, RectI32};
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_geometry::basic::transform3d::Perspective;
use pathfinder_geometry::clip::PolygonClipper3D;
use std::iter;
use std::u16;

const MAX_FILLS_PER_BATCH: usize = 0x0002_0000;
const MAX_MASKS_PER_BATCH: u16 = 0xffff;

pub struct SceneBuilder {
    objects: Vec<BuiltObject>,
    z_buffer: ZBuffer,
    tile_rect: RectI32,

    current_object_index: usize,
}

impl SceneBuilder {
    pub fn new(objects: Vec<BuiltObject>, z_buffer: ZBuffer, view_box: RectF32) -> SceneBuilder {
        SceneBuilder {
            objects,
            z_buffer,
            tile_rect: tiles::round_rect_out_to_tile_bounds(view_box),
            current_object_index: 0,
        }
    }

    pub fn build_solid_tiles(&self) -> Vec<SolidTileScenePrimitive> {
        self.z_buffer
            .build_solid_tiles(&self.objects, self.tile_rect)
    }

    pub fn build_batch(&mut self) -> Option<Batch> {
        let mut batch = Batch::new();

        let mut object_tile_index_to_batch_mask_tile_index = vec![];
        while self.current_object_index < self.objects.len() {
            let object = &self.objects[self.current_object_index];

            if batch.fills.len() + object.fills.len() > MAX_FILLS_PER_BATCH {
                break;
            }

            object_tile_index_to_batch_mask_tile_index.clear();
            object_tile_index_to_batch_mask_tile_index
                .extend(iter::repeat(u16::MAX).take(object.tiles.len()));

            // Copy mask tiles.
            for (tile_index, tile) in object.tiles.iter().enumerate() {
                // Skip solid tiles, since we handled them above already.
                if object.solid_tiles[tile_index] {
                    continue;
                }

                // Cull occluded tiles.
                let scene_tile_index = scene::scene_tile_index(tile.tile_x as i32,
                                                               tile.tile_y as i32,
                                                               self.tile_rect);
                if !self
                    .z_buffer
                    .test(scene_tile_index, self.current_object_index as u32)
                {
                    continue;
                }

                // Visible mask tile.
                let batch_mask_tile_index = batch.mask_tiles.len() as u16;
                if batch_mask_tile_index == MAX_MASKS_PER_BATCH {
                    break;
                }

                object_tile_index_to_batch_mask_tile_index[tile_index] = batch_mask_tile_index;

                batch.mask_tiles.push(MaskTileBatchPrimitive {
                    tile: *tile,
                    shader: object.shader,
                });
            }

            // Remap and copy fills, culling as necessary.
            for fill in &object.fills {
                let object_tile_index = object.tile_coords_to_index(fill.tile_x as i32,
                                                                    fill.tile_y as i32);
                let mask_tile_index =
                    object_tile_index_to_batch_mask_tile_index[object_tile_index as usize];
                if mask_tile_index < u16::MAX {
                    batch.fills.push(FillBatchPrimitive {
                        px: fill.px,
                        subpx: fill.subpx,
                        mask_tile_index,
                    });
                }
            }

            self.current_object_index += 1;
        }

        if batch.is_empty() {
            None
        } else {
            Some(batch)
        }
    }
}

#[derive(Clone, Default)]
pub struct RenderOptions {
    pub transform: RenderTransform,
    pub dilation: Point2DF32,
}

impl RenderOptions {
    pub fn prepare(self, bounds: RectF32) -> PreparedRenderOptions {
        PreparedRenderOptions {
            transform: self.transform.prepare(bounds),
            dilation: self.dilation,
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
        println!("... PERSPECTIVE quad={:?}", points);

        // Compute depth.
        let quad = [
            points[0].perspective_divide(),
            points[1].perspective_divide(),
            points[2].perspective_divide(),
            points[3].perspective_divide(),
        ];
        println!("barycentric(0, 0) = {:?}", compute_barycentric(&[
            quad[0].to_2d(),
            quad[1].to_2d(),
            quad[2].to_2d(),
            quad[3].to_2d(),
        ], Point2DF32::new(0.0, 0.0)));

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

        fn compute_barycentric(quad: &[Point2DF32], point: Point2DF32) -> [f32; 4] {
            let (s0, s1) = (quad[0] - point, quad[1] - point);
            let (s2, s3) = (quad[2] - point, quad[3] - point);
            let (a0, a1, a2, a3) = (s0.det(s1), s1.det(s2), s2.det(s3), s3.det(s0));
            let (d0, d1, d2, d3) = (s0.dot(s1), s1.dot(s2), s2.dot(s3), s3.dot(s0));
            let (r0, r1, r2, r3) = (s0.length(), s1.length(), s2.length(), s3.length());
            let (t0, t1) = ((r0 * r1 - d0) / a0, (r1 * r2 - d1) / a1);
            let (t2, t3) = ((r2 * r3 - d2) / a2, (r3 * r0 - d3) / a3);
            let (u0, u1) = ((t3 + t0) / r0, (t2 + t1) / r1);
            let (u2, u3) = ((t0 + t2) / r2, (t0 + t3) / r3);
            let sum = u0 + u1 + u2 + u3;
            [u0 / sum, u1 / sum, u2 / sum, u3 / sum]
        }
    }
}

pub struct PreparedRenderOptions {
    pub transform: PreparedRenderTransform,
    pub dilation: Point2DF32,
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

