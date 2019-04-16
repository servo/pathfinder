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

use crate::gpu_data::{BuiltObject, ConcurrentBuffer, MAX_ALPHA_TILES};
use crate::gpu_data::{MAX_FILLS, RenderCommand, SharedBuffers, SolidTileBatchPrimitive};
use crate::scene::Scene;
use crate::sorted_vector::SortedVector;
use crate::tiles::Tiler;
use crate::z_buffer::ZBuffer;
use parking_lot::Mutex;
use pathfinder_geometry::basic::point::{Point2DF32, Point2DI32, Point3DF32};
use pathfinder_geometry::basic::rect::RectF32;
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_geometry::basic::transform3d::Perspective;
use pathfinder_geometry::clip::PolygonClipper3D;
use pathfinder_geometry::distortion::BarrelDistortionCoefficients;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::cmp::{Ordering, PartialOrd};
use std::mem;
use std::sync::Arc;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::u16;

const MAX_FILLS_PER_BATCH: usize = 0xffffffff;
const MAX_ALPHA_TILES_PER_BATCH: usize = 0xffff;
const MAX_CHANNEL_MESSAGES: usize = 16;

pub struct SceneBuilderContext {
    info: Option<SceneAssemblyThreadInfo>,
}

struct SceneAssemblyThreadInfo {
    listener: Box<dyn RenderCommandListener>,
    built_object_queue: SortedVector<IndexedBuiltObject>,
    object_count: u32,

    buffers: Arc<SharedBuffers>,
    solid_tiles: Vec<SolidTileBatchPrimitive>,
}

pub trait RenderCommandListener: Send + Sync {
    fn send(&mut self, command: RenderCommand);
}

pub struct SceneBuilder<'ctx, 'a> {
    context: &'ctx mut SceneBuilderContext,
    scene: &'a Scene,
    built_options: &'a PreparedRenderOptions,
}

struct IndexedBuiltObject {
    object: BuiltObject,
    index: u32,
}

impl SceneBuilderContext {
    #[inline]
    pub fn new() -> SceneBuilderContext {
        SceneBuilderContext { info: None }
    }

    fn new_scene(&mut self,
                 listener: Box<dyn RenderCommandListener>,
                 buffers: Arc<SharedBuffers>,
                 object_count: u32) {
        self.info = Some(SceneAssemblyThreadInfo {
            listener,
            built_object_queue: SortedVector::new(),
            object_count,

            buffers,
            solid_tiles: vec![],
        })
    }

    /*
    fn add_indexed_object(&mut self, indexed_built_object: IndexedBuiltObject) {
        self.info.as_mut().unwrap().built_object_queue.push(indexed_built_object);

        loop {
            let next_object_index = self.info.as_ref().unwrap().next_object_index;
            match self.info.as_mut().unwrap().built_object_queue.peek() {
                Some(ref indexed_object) if
                        next_object_index == indexed_object.index => {}
                _ => break,
            }
            let indexed_object = self.info.as_mut().unwrap().built_object_queue.pop();
            self.info.as_mut().unwrap().next_object_index += 1;
        }
    }
    */

    /*
    fn add_object(&mut self, object: BuiltObject) {
        // See whether we have room for the alpha tiles. If we don't, then flush.
        let (tile_count, mut alpha_tile_count) = (object.tile_count() as usize, 0);
        for local_tile_index in 0..(object.tile_count() as usize) {
            if !object.tiles.data[local_tile_index].is_solid() {
                alpha_tile_count += 1;
            }
        }
        if self.info.as_ref().unwrap().buffers.alpha_tiles.lock().unwrap().len() +
                alpha_tile_count > MAX_ALPHA_TILES_PER_BATCH {
            self.flush_current_pass();
        }

        /*
        // Unpack.
        let info = &self.info.as_ref().unwrap();

        // Copy alpha tiles.
        for (local_tile_index, tile) in object.tiles.data.iter().cloned().enumerate() {
            // Skip solid tiles.
            if tile.is_solid() {
                continue;
            }

            let batch_alpha_tile_index = alpha_tiles.len() as u16;
            object_tile_index_to_batch_alpha_tile_index[tile_index] = batch_alpha_tile_index;

            let tile_coords = object.local_tile_index_to_coords(tile_index as u32);
            alpha_tiles.push(AlphaTileBatchPrimitive {
                tile_x: tile_coords.x() as i16,
                tile_y: tile_coords.y() as i16,
                object_index,
                backdrop: tile.backdrop,
            });
        }

        // Remap and copy fills, culling as necessary.
        for fill in &object.fills {
            let tile_coords = Point2DI32::new(fill.tile_x as i32, fill.tile_y as i32);
            let object_tile_index = object.tile_coords_to_index(tile_coords).unwrap();
            let object_tile_index = object_tile_index as usize;
            let alpha_tile_index = object_tile_index_to_batch_alpha_tile_index[object_tile_index];
            fills.push(FillBatchPrimitive {
                px: fill.px,
                subpx: fill.subpx,
                alpha_tile_index,
            });
        }
        */
    }
    */

    fn flush_current_pass(&mut self) {
        self.cull_alpha_tiles();

        let mut info = self.info.as_mut().unwrap();
        let fills = &info.buffers.fills;
        let alpha_tiles = &info.buffers.alpha_tiles;
        info.solid_tiles = info.buffers.z_buffer.build_solid_tiles(0..info.object_count);

        let have_solid_tiles = !info.solid_tiles.is_empty();
        let have_alpha_tiles = !alpha_tiles.is_empty();
        let have_fills = !fills.is_empty();
        if !have_solid_tiles && !have_alpha_tiles && !have_fills {
            return
        }

        info.listener.send(RenderCommand::ClearMaskFramebuffer);
        if have_solid_tiles {
            let tiles = mem::replace(&mut info.solid_tiles, vec![]);
            info.listener.send(RenderCommand::SolidTile(tiles));
        }
        if have_fills {
            info.listener.send(RenderCommand::Fill(fills.to_vec()));
            fills.clear();
        }
        if have_alpha_tiles {
            info.listener.send(RenderCommand::AlphaTile(alpha_tiles.to_vec()));
            alpha_tiles.clear();
        }
    }

    fn cull_alpha_tiles(&mut self) {
        let info = self.info.as_mut().unwrap();
        for alpha_tile_index in 0..info.buffers.alpha_tiles.len() {
            let mut alpha_tile = info.buffers.alpha_tiles.get(alpha_tile_index);
            let alpha_tile_coords = Point2DI32::new(alpha_tile.tile_x as i32,
                                                    alpha_tile.tile_y as i32);
            if info.buffers.z_buffer.test(alpha_tile_coords, alpha_tile.object_index as u32) {
                continue;
            }

            // FIXME(pcwalton): Hack!
            alpha_tile.tile_x = -1;
            alpha_tile.tile_y = -1;
            info.buffers.alpha_tiles.set(alpha_tile_index, alpha_tile);
        }
    }
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
        let buffers = Arc::new(SharedBuffers::new(effective_view_box));
        let object_count = self.scene.objects.len() as u32;
        self.send_new_scene_message_to_assembly_thread(listener, &buffers, object_count);

        for object_index in 0..self.scene.objects.len() {
            build_object(object_index,
                         effective_view_box,
                         &buffers,
                         &self.built_options,
                         &self.scene);
        }

        self.finish_and_wait_for_scene_assembly_thread();
    }

    pub fn build_in_parallel(&mut self, listener: Box<dyn RenderCommandListener>) {
        let effective_view_box = self.scene.effective_view_box(self.built_options);
        let buffers = Arc::new(SharedBuffers::new(effective_view_box));
        let object_count = self.scene.objects.len() as u32;
        self.send_new_scene_message_to_assembly_thread(listener, &buffers, object_count);

        (0..self.scene.objects.len()).into_par_iter().for_each(|object_index| {
            build_object(object_index,
                         effective_view_box,
                         &buffers,
                         &self.built_options,
                         &self.scene);
        });

        self.finish_and_wait_for_scene_assembly_thread();
    }

    fn send_new_scene_message_to_assembly_thread(&mut self,
                                                 listener: Box<dyn RenderCommandListener>,
                                                 buffers: &Arc<SharedBuffers>,
                                                 object_count: u32) {
        self.context.new_scene(listener, (*buffers).clone(), object_count)
    }

    fn finish_and_wait_for_scene_assembly_thread(&mut self) {
        self.context.flush_current_pass();
    }
}

fn build_object(object_index: usize,
                effective_view_box: RectF32,
                buffers: &SharedBuffers,
                built_options: &PreparedRenderOptions,
                scene: &Scene)
                -> BuiltObject {
    let object = &scene.objects[object_index];
    let outline = scene.apply_render_options(object.outline(), built_options);

    let mut tiler = Tiler::new(&outline, effective_view_box, object_index as u16, buffers);
    tiler.generate_tiles();
    tiler.built_object
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

impl PartialEq for IndexedBuiltObject {
    #[inline]
    fn eq(&self, other: &IndexedBuiltObject) -> bool {
        other.index == self.index
    }
}

impl PartialOrd for IndexedBuiltObject {
    #[inline]
    fn partial_cmp(&self, other: &IndexedBuiltObject) -> Option<Ordering> {
        other.index.partial_cmp(&self.index)
    }
}

impl<F> RenderCommandListener for F where F: FnMut(RenderCommand) + Send + Sync {
    #[inline]
    fn send(&mut self, command: RenderCommand) { (*self)(command) }
}
