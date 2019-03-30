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

use crate::gpu_data::{AlphaTileBatchPrimitive, BuiltObject, FillBatchPrimitive};
use crate::gpu_data::{RenderCommand, SolidTileBatchPrimitive};
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
use std::mem;
use std::u16;

const MAX_FILLS_PER_BATCH: usize = 0x0002_0000;
const MAX_ALPHA_TILES_PER_BATCH: u16 = 0xffff;

pub struct SceneBuilder {
    objects: Vec<BuiltObject>,
    z_buffer: ZBuffer,
    tile_rect: RectI32,

    current_object_index: usize,
    current_pass: Pass,
}

impl SceneBuilder {
    pub fn new(objects: Vec<BuiltObject>, z_buffer: ZBuffer, view_box: RectF32) -> SceneBuilder {
        SceneBuilder {
            objects,
            z_buffer,
            tile_rect: tiles::round_rect_out_to_tile_bounds(view_box),

            current_object_index: 0,
            current_pass: Pass::new(),
        }
    }

    pub fn build_render_command(&mut self) -> Option<RenderCommand> {
        match self.current_pass.state {
            PassState::Building => {}
            PassState::Done => {
                self.current_pass.state = PassState::Cleared;
                return Some(RenderCommand::ClearMaskFramebuffer)
            }
            PassState::Cleared if !self.current_pass.solid_tiles.is_empty() => {
                let tiles = mem::replace(&mut self.current_pass.solid_tiles, vec![]);
                return Some(RenderCommand::SolidTile(tiles));
            }
            PassState::Cleared if !self.current_pass.fills.is_empty() => {
                let fills = mem::replace(&mut self.current_pass.fills, vec![]);
                return Some(RenderCommand::Fill(fills));
            }
            PassState::Cleared if !self.current_pass.alpha_tiles.is_empty() => {
                let tiles = mem::replace(&mut self.current_pass.alpha_tiles, vec![]);
                return Some(RenderCommand::AlphaTile(tiles));
            }
            PassState::Cleared if self.current_object_index == self.objects.len() => return None,
            PassState::Cleared => self.current_pass.state = PassState::Building,
        }

        // FIXME(pcwalton): Figure out what to do here with multiple batches.
        self.current_pass.solid_tiles = self.z_buffer.build_solid_tiles(&self.objects,
                                                                        self.tile_rect);

        let mut object_tile_index_to_batch_alpha_tile_index = vec![];
        loop {
            if self.current_object_index == self.objects.len() {
                self.current_pass.state = PassState::Done;
                break;
            }

            let object = &self.objects[self.current_object_index];

            if self.current_pass.fills.len() + object.fills.len() > MAX_FILLS_PER_BATCH {
                self.current_pass.state = PassState::Done;
                break;
            }

            object_tile_index_to_batch_alpha_tile_index.clear();
            object_tile_index_to_batch_alpha_tile_index
                .extend(iter::repeat(u16::MAX).take(object.tiles.len()));

            // Copy alpha tiles.
            for (tile_index, tile) in object.tiles.iter().enumerate() {
                // Skip solid tiles.
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
                let batch_alpha_tile_index = self.current_pass.alpha_tiles.len() as u16;
                if batch_alpha_tile_index == MAX_ALPHA_TILES_PER_BATCH {
                    self.current_pass.state = PassState::Done;
                    break;
                }

                object_tile_index_to_batch_alpha_tile_index[tile_index] = batch_alpha_tile_index;

                self.current_pass.alpha_tiles.push(AlphaTileBatchPrimitive {
                    tile: *tile,
                    shader: object.shader,
                });
            }

            // Remap and copy fills, culling as necessary.
            for fill in &object.fills {
                let object_tile_index =
                    object.tile_coords_to_index(fill.tile_x as i32, fill.tile_y as i32).unwrap();
                let object_tile_index = object_tile_index as usize;
                let alpha_tile_index =
                    object_tile_index_to_batch_alpha_tile_index[object_tile_index];
                if alpha_tile_index < u16::MAX {
                    self.current_pass.fills.push(FillBatchPrimitive {
                        px: fill.px,
                        subpx: fill.subpx,
                        alpha_tile_index,
                    });
                }
            }

            self.current_object_index += 1;
        }

        self.build_render_command()
    }
}

struct Pass {
    solid_tiles: Vec<SolidTileBatchPrimitive>,
    alpha_tiles: Vec<AlphaTileBatchPrimitive>,
    fills: Vec<FillBatchPrimitive>,
    state: PassState,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum PassState {
    Building,
    Done,
    Cleared,
}

impl Pass {
    fn new() -> Pass {
        Pass {
            solid_tiles: vec![],
            alpha_tiles: vec![],
            fills: vec![],
            state: PassState::Building,
        }
    }
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
