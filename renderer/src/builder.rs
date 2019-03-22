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
use crate::gpu_data::{AlphaTileBatchPrimitive, SolidTileScenePrimitive};
use crate::scene;
use crate::tiles;
use crate::z_buffer::ZBuffer;
use pathfinder_geometry::basic::point::{Point2DF32, Point3DF32};
use pathfinder_geometry::basic::rect::{RectF32, RectI32};
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_geometry::basic::transform3d::Perspective;
use pathfinder_geometry::clip::PolygonClipper3D;
use pathfinder_geometry::distortion::BarrelDistortionCoefficients;
use std::iter;
use std::u16;

const MAX_FILLS_PER_BATCH: usize = 0x0002_0000;
const MAX_ALPHA_TILES_PER_BATCH: u16 = 0xffff;

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

        let mut object_tile_index_to_batch_alpha_tile_index = vec![];
        while self.current_object_index < self.objects.len() {
            let object = &self.objects[self.current_object_index];

            if batch.fills.len() + object.fills.len() > MAX_FILLS_PER_BATCH {
                break;
            }

            object_tile_index_to_batch_alpha_tile_index.clear();
            object_tile_index_to_batch_alpha_tile_index
                .extend(iter::repeat(u16::MAX).take(object.tiles.len()));

            // Copy alpha tiles.
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

                // Visible alpha tile.
                let batch_alpha_tile_index = batch.alpha_tiles.len() as u16;
                if batch_alpha_tile_index == MAX_ALPHA_TILES_PER_BATCH {
                    break;
                }

                object_tile_index_to_batch_alpha_tile_index[tile_index] = batch_alpha_tile_index;

                batch.alpha_tiles.push(AlphaTileBatchPrimitive {
                    tile: *tile,
                    shader: object.shader,
                });
            }

            // Remap and copy fills, culling as necessary.
            for fill in &object.fills {
                let object_tile_index =
                    object.tile_coords_to_index(fill.tile_x as i32, fill.tile_y as i32).unwrap();
                let alpha_tile_index =
                    object_tile_index_to_batch_alpha_tile_index[object_tile_index as usize];
                if alpha_tile_index < u16::MAX {
                    batch.fills.push(FillBatchPrimitive {
                        px: fill.px,
                        subpx: fill.subpx,
                        alpha_tile_index,
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
    pub barrel_distortion: Option<BarrelDistortionCoefficients>,
}

impl RenderOptions {
    pub fn prepare(self, bounds: RectF32) -> PreparedRenderOptions {
        PreparedRenderOptions {
            transform: self.transform.prepare(bounds),
            dilation: self.dilation,
            barrel_distortion: self.barrel_distortion,
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
