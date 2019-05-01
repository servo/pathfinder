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

use crate::concurrent::executor::Executor;
use crate::gpu_data::{AlphaTileBatchPrimitive, RenderCommand};
use crate::options::{PreparedRenderOptions, RenderCommandListener};
use crate::scene::Scene;
use crate::tiles::Tiler;
use crate::z_buffer::ZBuffer;
use pathfinder_geometry::basic::rect::RectF32;
use std::sync::atomic::AtomicUsize;
use std::time::Instant;
use std::u16;

pub struct SceneBuilder<'a> {
    scene: &'a Scene,
    built_options: &'a PreparedRenderOptions,

    pub(crate) next_alpha_tile_index: AtomicUsize,
    pub(crate) z_buffer: ZBuffer,
    pub(crate) listener: Box<dyn RenderCommandListener>,
}

impl<'a> SceneBuilder<'a> {
    pub fn new(
        scene: &'a Scene,
        built_options: &'a PreparedRenderOptions,
        listener: Box<dyn RenderCommandListener>,
    ) -> SceneBuilder<'a> {
        let effective_view_box = scene.effective_view_box(built_options);
        SceneBuilder {
            scene,
            built_options,

            next_alpha_tile_index: AtomicUsize::new(0),
            z_buffer: ZBuffer::new(effective_view_box),
            listener,
        }
    }

    pub fn build<E>(&mut self, executor: &E) where E: Executor {
        let start_time = Instant::now();

        let bounding_quad = self.built_options.bounding_quad();
        let object_count = self.scene.objects.len();
        self.listener.send(RenderCommand::Start { bounding_quad, object_count });

        self.listener.send(RenderCommand::AddShaders(self.scene.build_shaders()));

        let effective_view_box = self.scene.effective_view_box(self.built_options);
        let alpha_tiles = executor.flatten_into_vector(object_count, |object_index| {
            self.build_object(object_index, effective_view_box, &self.built_options, &self.scene)
        });

        self.finish_building(alpha_tiles);

        let build_time = Instant::now() - start_time;
        self.listener.send(RenderCommand::Finish { build_time });
    }

    fn build_object(
        &self,
        object_index: usize,
        view_box: RectF32,
        built_options: &PreparedRenderOptions,
        scene: &Scene,
    ) -> Vec<AlphaTileBatchPrimitive> {
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
            if self
                .z_buffer
                .test(alpha_tile_coords, alpha_tile.object_index as u32)
            {
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

#[derive(Clone, Copy, Debug, Default)]
pub struct TileStats {
    pub solid_tile_count: u32,
    pub alpha_tile_count: u32,
}
