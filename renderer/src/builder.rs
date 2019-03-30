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
use crate::scene::{self, Scene};
use crate::sorted_vector::SortedVector;
use crate::tiles::{self, Tiler};
use crate::z_buffer::ZBuffer;
use crossbeam_channel::{self, Receiver, Sender};
use pathfinder_geometry::basic::point::{Point2DF32, Point3DF32};
use pathfinder_geometry::basic::rect::{RectF32, RectI32};
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_geometry::basic::transform3d::Perspective;
use pathfinder_geometry::clip::PolygonClipper3D;
use pathfinder_geometry::distortion::BarrelDistortionCoefficients;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::cmp::{Ordering, PartialOrd};
use std::mem;
use std::ops::Range;
use std::sync::Arc;
use std::thread;
use std::u16;

const MAX_FILLS_PER_BATCH: usize = 0x0002_0000;
const MAX_ALPHA_TILES_PER_BATCH: usize = 0x1000;

pub struct SceneBuilderContext {
    sender: Sender<MainToSceneAssemblyMsg>,
    receiver: Receiver<SceneAssemblyToMainMsg>,
}

struct SceneAssemblyThread {
    receiver: Receiver<MainToSceneAssemblyMsg>,
    sender: Sender<SceneAssemblyToMainMsg>,
    info: Option<SceneAssemblyThreadInfo>,
}

struct SceneAssemblyThreadInfo {
    listener: Box<dyn RenderCommandListener>,
    built_object_queue: SortedVector<IndexedBuiltObject>,
    next_object_index: u32,

    pub(crate) z_buffer: Arc<ZBuffer>,
    tile_rect: RectI32,
    current_pass: Pass,
}

enum MainToSceneAssemblyMsg {
    NewScene {
        listener: Box<dyn RenderCommandListener>,
        effective_view_box: RectF32,
        z_buffer: Arc<ZBuffer>,
    },
    AddObject(IndexedBuiltObject),
    SceneFinished,
    Exit,
}

enum SceneAssemblyToMainMsg {
    FrameFinished,
}

impl Drop for SceneBuilderContext {
    #[inline]
    fn drop(&mut self) {
        self.sender.send(MainToSceneAssemblyMsg::Exit).unwrap();
    }
}

pub trait RenderCommandListener: Send {
    fn send(&mut self, command: RenderCommand);
}

pub struct SceneBuilder<'a> {
    context: &'a SceneBuilderContext,
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
        let (main_to_scene_assembly_sender,
             main_to_scene_assembly_receiver) = crossbeam_channel::unbounded();
        let (scene_assembly_to_main_sender,
             scene_assembly_to_main_receiver) = crossbeam_channel::unbounded();
        thread::spawn(move || {
            SceneAssemblyThread::new(main_to_scene_assembly_receiver,
                                     scene_assembly_to_main_sender).run()
        });
        SceneBuilderContext {
            sender: main_to_scene_assembly_sender,
            receiver: scene_assembly_to_main_receiver,
        }
    }
}

impl SceneAssemblyThread {
    #[inline]
    fn new(receiver: Receiver<MainToSceneAssemblyMsg>, sender: Sender<SceneAssemblyToMainMsg>)
           -> SceneAssemblyThread {
        SceneAssemblyThread { receiver, sender, info: None }
    }

    fn run(&mut self) {
        while let Ok(msg) = self.receiver.recv() {
            match msg {
                MainToSceneAssemblyMsg::Exit => break,
                MainToSceneAssemblyMsg::NewScene { listener, effective_view_box, z_buffer } => {
                    self.info = Some(SceneAssemblyThreadInfo {
                        listener,
                        built_object_queue: SortedVector::new(),
                        next_object_index: 0,

                        z_buffer,
                        tile_rect: tiles::round_rect_out_to_tile_bounds(effective_view_box),
                        current_pass: Pass::new(),
                    })
                }
                MainToSceneAssemblyMsg::AddObject(indexed_built_object) => {
                    self.info.as_mut().unwrap().built_object_queue.push(indexed_built_object);

                    loop {
                        let next_object_index = self.info.as_ref().unwrap().next_object_index;
                        match self.info.as_mut().unwrap().built_object_queue.peek() {
                            Some(ref indexed_object) if
                                    next_object_index == indexed_object.index => {}
                            _ => break,
                        }
                        let indexed_object = self.info.as_mut().unwrap().built_object_queue.pop();
                        self.add_object(indexed_object.unwrap().object);
                        self.info.as_mut().unwrap().next_object_index += 1;
                    }
                }
                MainToSceneAssemblyMsg::SceneFinished => {
                    self.flush_current_pass();
                    self.sender.send(SceneAssemblyToMainMsg::FrameFinished).unwrap();
                }
            }
        }
    }

    fn add_object(&mut self, object: BuiltObject) {
        // Flush current pass if necessary.
        if self.info.as_ref().unwrap().current_pass.fills.len() + object.fills.len() >
                MAX_FILLS_PER_BATCH {
            self.flush_current_pass();
        }

        // See whether we have room for the alpha tiles. If we don't, then flush.
        let mut alpha_tile_count = 0;
        for tile_index in 0..object.tiles.len() {
            if !object.solid_tiles[tile_index] {
                alpha_tile_count += 1;
            }
        }
        if self.info.as_ref().unwrap().current_pass.alpha_tiles.len() + alpha_tile_count >
                MAX_ALPHA_TILES_PER_BATCH {
            self.flush_current_pass();
        }

        // Copy alpha tiles.
        let mut current_pass = &mut self.info.as_mut().unwrap().current_pass;
        let mut object_tile_index_to_batch_alpha_tile_index = vec![u16::MAX; object.tiles.len()];
        for (tile_index, tile) in object.tiles.iter().enumerate() {
            // Skip solid tiles.
            if object.solid_tiles[tile_index] {
                continue;
            }

            let batch_alpha_tile_index = current_pass.alpha_tiles.len() as u16;
            object_tile_index_to_batch_alpha_tile_index[tile_index] = batch_alpha_tile_index;

            current_pass.alpha_tiles.push(AlphaTileBatchPrimitive {
                tile: *tile,
                object_index: current_pass.object_range.end as u16,
            });
        }

        // Remap and copy fills, culling as necessary.
        for fill in &object.fills {
            let object_tile_index = object.tile_coords_to_index(fill.tile_x as i32,
                                                                fill.tile_y as i32).unwrap();
            let object_tile_index = object_tile_index as usize;
            let alpha_tile_index = object_tile_index_to_batch_alpha_tile_index[object_tile_index];
            current_pass.fills.push(FillBatchPrimitive {
                px: fill.px,
                subpx: fill.subpx,
                alpha_tile_index,
            });
        }

        current_pass.object_range.end += 1;
    }

    fn flush_current_pass(&mut self) {
        self.cull_alpha_tiles();

        let mut info = self.info.as_mut().unwrap();
        info.current_pass.solid_tiles =
            info.z_buffer.build_solid_tiles(info.tile_rect,
                                            info.current_pass.object_range.clone());

        let have_solid_tiles = !info.current_pass.solid_tiles.is_empty();
        let have_alpha_tiles = !info.current_pass.alpha_tiles.is_empty();
        let have_fills = !info.current_pass.fills.is_empty();
        if !have_solid_tiles && !have_alpha_tiles && !have_fills {
            return
        }

        info.listener.send(RenderCommand::ClearMaskFramebuffer);
        if have_solid_tiles {
            let tiles = mem::replace(&mut info.current_pass.solid_tiles, vec![]);
            info.listener.send(RenderCommand::SolidTile(tiles));
        }
        if have_fills {
            let fills = mem::replace(&mut info.current_pass.fills, vec![]);
            info.listener.send(RenderCommand::Fill(fills));
        }
        if have_alpha_tiles {
            let tiles = mem::replace(&mut info.current_pass.alpha_tiles, vec![]);
            info.listener.send(RenderCommand::AlphaTile(tiles));
        }

        info.current_pass.object_range.start = info.current_pass.object_range.end;
    }

    fn cull_alpha_tiles(&mut self) {
        let info = self.info.as_mut().unwrap();
        for alpha_tile in &mut info.current_pass.alpha_tiles {
            let scene_tile_index = scene::scene_tile_index(alpha_tile.tile.tile_x as i32,
                                                           alpha_tile.tile.tile_y as i32,
                                                           info.tile_rect);
            if info.z_buffer.test(scene_tile_index, alpha_tile.object_index as u32) {
                continue;
            }
            // FIXME(pcwalton): Hack!
            alpha_tile.tile.tile_x = -1;
            alpha_tile.tile.tile_y = -1;
        }
    }
}

impl<'a> SceneBuilder<'a> {
    pub fn new(context: &'a SceneBuilderContext,
               scene: &'a Scene,
               built_options: &'a PreparedRenderOptions)
               -> SceneBuilder<'a> {
        SceneBuilder { context, scene, built_options }
    }

    pub fn build_sequentially(&mut self, listener: Box<dyn RenderCommandListener>) {
        let effective_view_box = self.scene.effective_view_box(self.built_options);
        let z_buffer = Arc::new(ZBuffer::new(effective_view_box));
        self.send_new_scene_message_to_assembly_thread(listener, effective_view_box, &z_buffer);

        for object_index in 0..self.scene.objects.len() {
            build_object(object_index,
                         effective_view_box,
                         &z_buffer,
                         &self.built_options,
                         &self.scene,
                         &self.context.sender);
        }

        self.finish_and_wait_for_scene_assembly_thread();
    }

    pub fn build_in_parallel(&mut self, listener: Box<dyn RenderCommandListener>) {
        let effective_view_box = self.scene.effective_view_box(self.built_options);
        let z_buffer = Arc::new(ZBuffer::new(effective_view_box));
        self.send_new_scene_message_to_assembly_thread(listener, effective_view_box, &z_buffer);

        (0..self.scene.objects.len()).into_par_iter().for_each(|object_index| {
            build_object(object_index,
                         effective_view_box,
                         &z_buffer,
                         &self.built_options,
                         &self.scene,
                         &self.context.sender);
        });

        self.finish_and_wait_for_scene_assembly_thread();
    }

    fn send_new_scene_message_to_assembly_thread(&mut self,
                                                 listener: Box<dyn RenderCommandListener>,
                                                 effective_view_box: RectF32,
                                                 z_buffer: &Arc<ZBuffer>) {
        self.context.sender.send(MainToSceneAssemblyMsg::NewScene {
            listener,
            effective_view_box,
            z_buffer: z_buffer.clone(),
        }).unwrap();
    }

    fn finish_and_wait_for_scene_assembly_thread(&mut self) {
        self.context.sender.send(MainToSceneAssemblyMsg::SceneFinished).unwrap();
        self.context.receiver.recv().unwrap();
    }
}

fn build_object(object_index: usize,
                effective_view_box: RectF32,
                z_buffer: &ZBuffer,
                built_options: &PreparedRenderOptions,
                scene: &Scene,
                sender: &Sender<MainToSceneAssemblyMsg>) {
    let object = &scene.objects[object_index];
    let outline = scene.apply_render_options(object.outline(), built_options);

    let mut tiler = Tiler::new(&outline, effective_view_box, object_index as u16, z_buffer);
    tiler.generate_tiles();

    sender.send(MainToSceneAssemblyMsg::AddObject(IndexedBuiltObject {
        index: object_index as u32,
        object: tiler.built_object,
    })).unwrap();
}

struct Pass {
    solid_tiles: Vec<SolidTileBatchPrimitive>,
    alpha_tiles: Vec<AlphaTileBatchPrimitive>,
    fills: Vec<FillBatchPrimitive>,
    object_range: Range<u32>,
}

impl Pass {
    fn new() -> Pass {
        Pass { solid_tiles: vec![], alpha_tiles: vec![], fills: vec![], object_range: 0..0 }
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

impl<F> RenderCommandListener for F where F: FnMut(RenderCommand) + Send {
    #[inline]
    fn send(&mut self, command: RenderCommand) { (*self)(command) }
}
