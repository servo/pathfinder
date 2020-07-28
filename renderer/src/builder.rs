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
use crate::gpu::blend::BlendModeExt;
use crate::gpu::options::RendererLevel;
use crate::gpu_data::{AlphaTileId, BackdropInfoD3D11, Clip, ClippedPathInfo, DiceMetadataD3D11};
use crate::gpu_data::{DrawTileBatch, DrawTileBatchD3D9, DrawTileBatchD3D11, Fill, GlobalPathId};
use crate::gpu_data::{PathBatchIndex, PathSource, PrepareTilesInfoD3D11, PropagateMetadataD3D11};
use crate::gpu_data::{RenderCommand, SegmentIndicesD3D11, SegmentsD3D11, TileBatchDataD3D11};
use crate::gpu_data::{TileBatchId, TileBatchTexture, TileObjectPrimitive, TilePathInfoD3D11};
use crate::options::{PrepareMode, PreparedBuildOptions, PreparedRenderTransform};
use crate::paint::{PaintId, PaintInfo, PaintMetadata};
use crate::scene::{ClipPathId, DisplayItem, DrawPath, DrawPathId, LastSceneInfo, PathId};
use crate::scene::{Scene, SceneSink};
use crate::tile_map::DenseTileMap;
use crate::tiler::Tiler;
use crate::tiles::{self, DrawTilingPathInfo, TILE_HEIGHT, TILE_WIDTH, TilingPathInfo};
use fxhash::FxHashMap;
use instant::Instant;
use pathfinder_content::effects::{BlendMode, Filter};
use pathfinder_content::fill::FillRule;
use pathfinder_content::outline::{Outline, PointFlags};
use pathfinder_geometry::line_segment::{LineSegment2F, LineSegmentU16};
use pathfinder_geometry::rect::{RectF, RectI};
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::{Vector2I, vec2i};
use pathfinder_gpu::TextureSamplingFlags;
use pathfinder_simd::default::F32x4;
use std::borrow::Cow;
use std::ops::Range;
use std::sync::atomic::AtomicUsize;
use std::u32;

pub(crate) const ALPHA_TILE_LEVEL_COUNT: usize = 2;
pub(crate) const ALPHA_TILES_PER_LEVEL: usize = 1 << (32 - ALPHA_TILE_LEVEL_COUNT + 1);

const CURVE_IS_QUADRATIC: u32 = 0x80000000;
const CURVE_IS_CUBIC:     u32 = 0x40000000;

const MAX_CLIP_BATCHES: u32 = 32;

pub(crate) struct SceneBuilder<'a, 'b, 'c, 'd> {
    pub(crate) scene: &'a mut Scene,
    built_options: &'b PreparedBuildOptions,
    next_alpha_tile_indices: [AtomicUsize; ALPHA_TILE_LEVEL_COUNT],
    pub(crate) sink: &'c mut SceneSink<'d>,
}

#[derive(Debug)]
pub(crate) struct ObjectBuilder {
    pub built_path: BuiltPath,
    pub fills: Vec<Fill>,
    pub bounds: RectF,
}

// Derives `Clone` just so we can use `Cow`, not because we actually want to clone it.
#[derive(Clone, Debug)]
struct BuiltDrawPath {
    path: BuiltPath,
    clip_path_id: Option<ClipPathId>,
    blend_mode: BlendMode,
    filter: Filter,
    color_texture: Option<TileBatchTexture>,
    sampling_flags_1: TextureSamplingFlags,
    mask_0_fill_rule: FillRule,
    occludes: bool,
}

impl BuiltDrawPath {
    fn new(built_path: BuiltPath, path_object: &DrawPath, paint_metadata: &PaintMetadata)
           -> BuiltDrawPath {
        let blend_mode = path_object.blend_mode();
        let occludes = paint_metadata.is_opaque && blend_mode.occludes_backdrop();
        BuiltDrawPath {
            path: built_path,
            clip_path_id: path_object.clip_path(),
            filter: paint_metadata.filter(),
            color_texture: paint_metadata.tile_batch_texture(),
            sampling_flags_1: TextureSamplingFlags::empty(),
            mask_0_fill_rule: path_object.fill_rule(),
            blend_mode,
            occludes,
        }
    }
}

// Derives `Clone` just so we can use `Cow`, not because we actually want to clone it.
#[derive(Clone, Debug)]
pub(crate) struct BuiltPath {
    pub data: BuiltPathData,
    pub tile_bounds: RectI,
    pub fill_rule: FillRule,
    pub clip_path_id: Option<ClipPathId>,
    pub ctrl_byte: u8,
    pub paint_id: PaintId,
}

#[derive(Clone, Debug)]
pub(crate) enum BuiltPathData {
    CPU(BuiltPathBinCPUData),
    TransformCPUBinGPU(BuiltPathTransformCPUBinGPUData),
    GPU,
}

#[derive(Clone, Debug)]
pub(crate) struct BuiltPathBinCPUData {
    /// During tiling, or if backdrop computation is done on GPU, this stores the sum of backdrops
    /// for tile columns above the viewport.
    pub backdrops: Vec<i32>,
    pub tiles: DenseTileMap<TileObjectPrimitive>,
    pub clip_tiles: Option<DenseTileMap<Clip>>,
}

#[derive(Clone, Debug)]
pub(crate) struct BuiltPathTransformCPUBinGPUData {
    /// The transformed outline.
    pub outline: Outline,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Occluder {
    pub(crate) coords: Vector2I,
}

impl<'a, 'b, 'c, 'd> SceneBuilder<'a, 'b, 'c, 'd> {
    pub(crate) fn new(scene: &'a mut Scene,
                      built_options: &'b PreparedBuildOptions,
                      sink: &'c mut SceneSink<'d>)
                      -> SceneBuilder<'a, 'b, 'c, 'd> {
        SceneBuilder {
            scene,
            built_options,
            next_alpha_tile_indices: [AtomicUsize::new(0), AtomicUsize::new(0)],
            sink,
        }
    }

    pub fn build<E>(&mut self, executor: &E) where E: Executor {
        let start_time = Instant::now();

        // Send the start rendering command.
        let bounding_quad = self.built_options.bounding_quad();

        let clip_path_count = self.scene.clip_paths().len();
        let draw_path_count = self.scene.draw_paths().len();
        let total_path_count = clip_path_count + draw_path_count;

        let needs_readable_framebuffer = self.needs_readable_framebuffer();

        self.sink.listener.send(RenderCommand::Start {
            bounding_quad,
            path_count: total_path_count,
            needs_readable_framebuffer,
        });

        let prepare_mode = self.built_options.to_prepare_mode(self.sink.renderer_level);

        let render_transform = match self.built_options.transform {
            PreparedRenderTransform::Transform2D(transform) => transform.inverse(),
            _ => Transform2F::default()
        };

        // Build paint data.
        let PaintInfo {
            render_commands,
            paint_metadata,
        } = self.scene.build_paint_info(&mut self.sink.paint_texture_manager, render_transform);
        for render_command in render_commands {
            self.sink.listener.send(render_command);
        }

        let built_paths = match prepare_mode {
            PrepareMode::CPU | PrepareMode::TransformCPUBinGPU => {
                Some(self.build_paths_on_cpu(executor, &paint_metadata, &prepare_mode))
            }
            PrepareMode::GPU { .. } => None,
        };

        // TODO(pcwalton): Do this earlier?
        let scene_is_dirty = match (&prepare_mode, &self.sink.last_scene) {
            (&PrepareMode::GPU { .. }, &None) => true,
            (&PrepareMode::GPU { .. }, &Some(LastSceneInfo {
                 scene_id: ref last_scene_id,
                 scene_epoch: ref last_scene_epoch,
                 ..
            })) => *last_scene_id != self.scene.id() || *last_scene_epoch != self.scene.epoch(),
            _ => false,
        };

        if scene_is_dirty {
            let built_segments = BuiltSegments::from_scene(&self.scene);
            self.sink.listener.send(RenderCommand::UploadSceneD3D11 {
                draw_segments: built_segments.draw_segments,
                clip_segments: built_segments.clip_segments,
            });
            self.sink.last_scene = Some(LastSceneInfo {
                scene_id: self.scene.id(),
                scene_epoch: self.scene.epoch(),
                draw_segment_ranges: built_segments.draw_segment_ranges,
                clip_segment_ranges: built_segments.clip_segment_ranges,
            });
        }

        self.finish_building(&paint_metadata, built_paths, &prepare_mode);

        let cpu_build_time = Instant::now() - start_time;
        self.sink.listener.send(RenderCommand::Finish { cpu_build_time });
    }

    fn build_paths_on_cpu<E>(&mut self,
                             executor: &E,
                             paint_metadata: &[PaintMetadata],
                             prepare_mode: &PrepareMode)
                             -> BuiltPaths
                             where E: Executor {
        let clip_path_count = self.scene.clip_paths().len();
        let draw_path_count = self.scene.draw_paths().len();
        let effective_view_box = self.scene.effective_view_box(self.built_options);

        let built_clip_paths = executor.build_vector(clip_path_count, |path_index| {
            self.build_clip_path_on_cpu(PathBuildParams {
                path_id: PathId(path_index as u32),
                view_box: effective_view_box,
                prepare_mode: *prepare_mode,
                built_options: &self.built_options,
                scene: &self.scene,
            })
        });

        let built_draw_paths = executor.build_vector(draw_path_count, |path_index| {
            self.build_draw_path_on_cpu(DrawPathBuildParams {
                path_build_params: PathBuildParams {
                    path_id: PathId(path_index as u32),
                    view_box: effective_view_box,
                    prepare_mode: *prepare_mode,
                    built_options: &self.built_options,
                    scene: &self.scene,
                },
                paint_metadata: &paint_metadata,
                built_clip_paths: &built_clip_paths,
            })
        });

        BuiltPaths { draw: built_draw_paths }
    }

    fn build_clip_path_on_cpu(&self, params: PathBuildParams) -> BuiltPath {
        let PathBuildParams { path_id, view_box, built_options, scene, prepare_mode } = params;
        let path_object = &scene.get_clip_path(path_id.to_clip_path_id());
        let outline = scene.apply_render_options(path_object.outline(), built_options);

        let mut tiler = Tiler::new(self,
                                   path_id,
                                   &outline,
                                   path_object.fill_rule(),
                                   view_box,
                                   &prepare_mode,
                                   path_object.clip_path(),
                                   &[],
                                   TilingPathInfo::Clip);

        tiler.generate_tiles();
        self.send_fills(tiler.object_builder.fills);
        tiler.object_builder.built_path
    }

    fn build_draw_path_on_cpu(&self, params: DrawPathBuildParams) -> BuiltDrawPath {
        let DrawPathBuildParams {
            path_build_params: PathBuildParams {
                path_id,
                view_box,
                built_options,
                prepare_mode,
                scene,
            },
            paint_metadata,
            built_clip_paths,
        } = params;

        let path_object = scene.get_draw_path(path_id.to_draw_path_id());
        let outline = scene.apply_render_options(path_object.outline(), built_options);

        let paint_id = path_object.paint();
        let paint_metadata = &paint_metadata[paint_id.0 as usize];

        let mut tiler = Tiler::new(self,
                                   path_id,
                                   &outline,
                                   path_object.fill_rule(),
                                   view_box,
                                   &prepare_mode,
                                   path_object.clip_path(),
                                   &built_clip_paths,
                                   TilingPathInfo::Draw(DrawTilingPathInfo {
            paint_id,
            blend_mode: path_object.blend_mode(),
            fill_rule: path_object.fill_rule(),
        }));

        tiler.generate_tiles();
        self.send_fills(tiler.object_builder.fills);

        BuiltDrawPath::new(tiler.object_builder.built_path, path_object, paint_metadata)
    }

    fn send_fills(&self, fills: Vec<Fill>) {
        if !fills.is_empty() {
            self.sink.listener.send(RenderCommand::AddFillsD3D9(fills));
        }
    }

    fn build_tile_batches(&mut self,
                          paint_metadata: &[PaintMetadata],
                          prepare_mode: &PrepareMode,
                          built_paths: Option<BuiltPaths>) {
        let mut tile_batch_builder = TileBatchBuilder::new(built_paths);

        // Prepare display items.
        for display_item in self.scene.display_list() {
            match *display_item {
                DisplayItem::PushRenderTarget(render_target_id) => {
                    tile_batch_builder.draw_commands
                                      .push(RenderCommand::PushRenderTarget(render_target_id))
                }
                DisplayItem::PopRenderTarget => {
                    tile_batch_builder.draw_commands.push(RenderCommand::PopRenderTarget)
                }
                DisplayItem::DrawPaths(ref path_id_range) => {
                    tile_batch_builder.build_tile_batches_for_draw_path_display_item(
                        &self.scene,
                        &self.sink,
                        self.built_options,
                        path_id_range.start..path_id_range.end,
                        paint_metadata,
                        prepare_mode);
                }
            }
        }

        // Send commands.
        tile_batch_builder.send_to(&self.sink);
    }

    fn finish_building(&mut self,
                       paint_metadata: &[PaintMetadata],
                       built_paths: Option<BuiltPaths>,
                       prepare_mode: &PrepareMode) {
        match self.sink.renderer_level {
            RendererLevel::D3D9 => self.sink.listener.send(RenderCommand::FlushFillsD3D9),
            RendererLevel::D3D11 => {}
        }

        self.build_tile_batches(paint_metadata, prepare_mode, built_paths);
    }

    fn needs_readable_framebuffer(&self) -> bool {
        let mut framebuffer_nesting = 0;
        for display_item in self.scene.display_list() {
            match *display_item {
                DisplayItem::PushRenderTarget(_) => framebuffer_nesting += 1,
                DisplayItem::PopRenderTarget => framebuffer_nesting -= 1,
                DisplayItem::DrawPaths(ref draw_path_id_range) => {
                    if framebuffer_nesting > 0 {
                        continue;
                    }
                    for draw_path_id in draw_path_id_range.start.0..draw_path_id_range.end.0 {
                        let draw_path_id = DrawPathId(draw_path_id);
                        let blend_mode = self.scene.get_draw_path(draw_path_id).blend_mode();
                        if blend_mode.needs_readable_framebuffer() {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

struct BuiltPaths {
    draw: Vec<BuiltDrawPath>,
}

struct PathBuildParams<'a> {
    path_id: PathId,
    view_box: RectF,
    built_options: &'a PreparedBuildOptions,
    prepare_mode: PrepareMode,
    scene: &'a Scene,
}

struct DrawPathBuildParams<'a> {
    path_build_params: PathBuildParams<'a>,
    paint_metadata: &'a [PaintMetadata],
    built_clip_paths: &'a [BuiltPath],
}

impl BuiltPath {
    fn new(path_id: PathId,
           path_bounds: RectF,
           view_box_bounds: RectF,
           fill_rule: FillRule,
           prepare_mode: &PrepareMode,
           clip_path_id: Option<ClipPathId>,
           tiling_path_info: &TilingPathInfo)
           -> BuiltPath {
        let paint_id = match *tiling_path_info {
            TilingPathInfo::Draw(ref draw_tiling_path_info) => draw_tiling_path_info.paint_id,
            TilingPathInfo::Clip => PaintId(0),
        };

        let ctrl_byte = tiling_path_info.to_ctrl();

        let tile_map_bounds = if tiling_path_info.has_destructive_blend_mode() {
            view_box_bounds
        } else {
            path_bounds
        };

        let tile_bounds = tiles::round_rect_out_to_tile_bounds(tile_map_bounds);

        let data = match *prepare_mode {
            PrepareMode::CPU => {
                BuiltPathData::CPU(BuiltPathBinCPUData {
                    backdrops: vec![0; tile_bounds.width() as usize],
                    tiles: DenseTileMap::from_builder(|tile_coord| {
                            TileObjectPrimitive {
                                tile_x: tile_coord.x() as i16,
                                tile_y: tile_coord.y() as i16,
                                alpha_tile_id: AlphaTileId(!0),
                                path_id,
                                color: paint_id.0,
                                backdrop: 0,
                                ctrl: ctrl_byte,
                            }
                        }, tile_bounds),
                    clip_tiles: match *tiling_path_info {
                        TilingPathInfo::Draw(_) if clip_path_id.is_some() => {
                            Some(DenseTileMap::from_builder(|_| {
                                Clip {
                                    dest_tile_id: AlphaTileId(!0),
                                    dest_backdrop: 0,
                                    src_tile_id: AlphaTileId(!0),
                                    src_backdrop: 0,
                                }
                            }, tile_bounds))
                        }
                        _ => None,
                    },
                })
            }
            PrepareMode::TransformCPUBinGPU => {
                BuiltPathData::TransformCPUBinGPU(BuiltPathTransformCPUBinGPUData {
                    outline: Outline::new(),
                })
            }
            PrepareMode::GPU { .. } => BuiltPathData::GPU,
        };

        BuiltPath {
            data,
            tile_bounds,
            clip_path_id,
            fill_rule,
            ctrl_byte,
            paint_id,
        }
    }
}

// Utilities for built objects

impl ObjectBuilder {
    // If `outline` is `None`, then tiling is being done on CPU. Otherwise, it's done on GPU.
    pub(crate) fn new(path_id: PathId,
                      path_bounds: RectF,
                      view_box_bounds: RectF,
                      fill_rule: FillRule,
                      prepare_mode: &PrepareMode,
                      clip_path_id: Option<ClipPathId>,
                      tiling_path_info: &TilingPathInfo)
                      -> ObjectBuilder {
        let built_path = BuiltPath::new(path_id,
                                        path_bounds,
                                        view_box_bounds,
                                        fill_rule,
                                        prepare_mode,
                                        clip_path_id,
                                        tiling_path_info);
        ObjectBuilder { built_path, bounds: path_bounds, fills: vec![] }
    }

    pub(crate) fn add_fill(&mut self,
                           scene_builder: &SceneBuilder,
                           segment: LineSegment2F,
                           tile_coords: Vector2I) {
        debug!("add_fill({:?} ({:?}))", segment, tile_coords);

        // Ensure this fill is in bounds. If not, cull it.
        if self.tile_coords_to_local_index(tile_coords).is_none() {
            return;
        }

        debug_assert_eq!(TILE_WIDTH, TILE_HEIGHT);

        // Compute the upper left corner of the tile.
        let tile_size = F32x4::splat(TILE_WIDTH as f32);
        let tile_upper_left = tile_coords.to_f32().0.to_f32x4().xyxy() * tile_size;

        // Convert to 8.8 fixed point.
        let segment = (segment.0 - tile_upper_left) * F32x4::splat(256.0);
        let (min, max) = (F32x4::default(), F32x4::splat((TILE_WIDTH * 256 - 1) as f32));
        let segment = segment.clamp(min, max).to_i32x4();
        let (from_x, from_y, to_x, to_y) = (segment[0], segment[1], segment[2], segment[3]);

        // Cull degenerate fills.
        if from_x == to_x {
            debug!("... culling!");
            return;
        }

        // Allocate a global tile if necessary.
        let alpha_tile_id = self.get_or_allocate_alpha_tile_index(scene_builder, tile_coords);

        // Pack instance data.
        debug!("... OK, pushing");
        self.fills.push(Fill {
            line_segment: LineSegmentU16 {
                from_x: from_x as u16,
                from_y: from_y as u16,
                to_x: to_x as u16,
                to_y: to_y as u16,
            },
            // If fills are being done with compute, then this value will be overwritten later.
            link: alpha_tile_id.0,
        });
    }

    fn get_or_allocate_alpha_tile_index(&mut self,
                                        scene_builder: &SceneBuilder,
                                        tile_coords: Vector2I)
                                        -> AlphaTileId {
        let local_tile_index = self.tile_coords_to_local_index_unchecked(tile_coords) as usize;

        let tiles = match self.built_path.data {
            BuiltPathData::CPU(ref mut cpu_data) => &mut cpu_data.tiles,
            BuiltPathData::GPU | BuiltPathData::TransformCPUBinGPU(_) => {
                panic!("Can't allocate alpha tile index on CPU if not doing building on CPU!")
            }
        };

        let alpha_tile_id = tiles.data[local_tile_index].alpha_tile_id;
        if alpha_tile_id.is_valid() {
            return alpha_tile_id;
        }

        let alpha_tile_id = AlphaTileId::new(&scene_builder.next_alpha_tile_indices, 0);
        tiles.data[local_tile_index].alpha_tile_id = alpha_tile_id;
        alpha_tile_id
    }

    #[inline]
    pub(crate) fn tile_coords_to_local_index_unchecked(&self, coords: Vector2I) -> u32 {
        let tile_rect = self.built_path.tile_bounds;
        let offset = coords - tile_rect.origin();
        (offset.x() + tile_rect.width() * offset.y()) as u32
    }

    #[inline]
    pub(crate) fn tile_coords_to_local_index(&self, coords: Vector2I) -> Option<u32> {
        if self.built_path.tile_bounds.contains_point(coords) {
            Some(self.tile_coords_to_local_index_unchecked(coords))
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn adjust_alpha_tile_backdrop(&mut self, tile_coords: Vector2I, delta: i8) {
        let (tiles, backdrops) = match self.built_path.data {
            BuiltPathData::CPU(ref mut tiled_data) => {
                (&mut tiled_data.tiles, &mut tiled_data.backdrops)
            }
            BuiltPathData::TransformCPUBinGPU(_) | BuiltPathData::GPU => unreachable!(),
        };

        let tile_offset = tile_coords - tiles.rect.origin();
        if tile_offset.x() < 0 || tile_offset.x() >= tiles.rect.width() ||
                tile_offset.y() >= tiles.rect.height() {
            return;
        }

        if tile_offset.y() < 0 {
            backdrops[tile_offset.x() as usize] += delta as i32;
            return;
        }

        let local_tile_index = tiles.coords_to_index_unchecked(tile_coords);
        tiles.data[local_tile_index].backdrop += delta;
    }
}

impl TileBatchDataD3D11 {
    fn new(batch_id: TileBatchId, mode: &PrepareMode, path_source: PathSource)
           -> TileBatchDataD3D11 {
        TileBatchDataD3D11 {
            batch_id,
            path_count: 0,
            tile_count: 0,
            segment_count: 0,
            path_source,
            prepare_info: match *mode {
                PrepareMode::CPU => unimplemented!(),
                PrepareMode::TransformCPUBinGPU => {
                    PrepareTilesInfoD3D11 {
                        backdrops: vec![],
                        propagate_metadata: vec![],
                        dice_metadata: vec![],
                        tile_path_info: vec![],
                        transform: Transform2F::default(),
                    }
                }
                PrepareMode::GPU { ref transform } => {
                    PrepareTilesInfoD3D11 {
                        backdrops: vec![],
                        propagate_metadata: vec![],
                        dice_metadata: vec![],
                        tile_path_info: vec![],
                        transform: *transform,
                    }
                }
            },
            clipped_path_info: None,
        }
    }

    fn push(&mut self,
            path: &BuiltPath,
            global_path_id: PathId,
            batch_clip_path_id: Option<GlobalPathId>,
            z_write: bool,
            sink: &SceneSink)
            -> PathBatchIndex {
        let batch_path_index = PathBatchIndex(self.path_count);
        self.path_count += 1;

        self.prepare_info.propagate_metadata.push(PropagateMetadataD3D11 {
            tile_rect: path.tile_bounds,
            tile_offset: self.tile_count,
            path_index: batch_path_index,
            z_write: z_write as u32,
            clip_path_index: match batch_clip_path_id {
                None => PathBatchIndex::none(),
                Some(batch_clip_path_id) => batch_clip_path_id.path_index,
            },
            backdrop_offset: self.prepare_info.backdrops.len() as u32,
            pad0: 0,
            pad1: 0,
            pad2: 0,
        });

        match path.data {
            BuiltPathData::CPU(ref data) => {
                self.prepare_info.backdrops.reserve(data.backdrops.len());
                for (tile_x_offset, backdrop) in data.backdrops.iter().enumerate() {
                    self.prepare_info.backdrops.push(BackdropInfoD3D11 {
                        initial_backdrop: *backdrop as i32,
                        tile_x_offset: tile_x_offset as i32,
                        path_index: batch_path_index,
                    });
                }
            }
            BuiltPathData::TransformCPUBinGPU(_) | BuiltPathData::GPU => {
                init_backdrops(&mut self.prepare_info.backdrops,
                               batch_path_index,
                               path.tile_bounds);
            }
        }

        // Add tiles.
        let last_scene = sink.last_scene.as_ref().unwrap();
        let segment_ranges = match self.path_source {
            PathSource::Draw => &last_scene.draw_segment_ranges,
            PathSource::Clip => &last_scene.clip_segment_ranges,
        };
        let segment_range = &segment_ranges[global_path_id.0 as usize];
        self.prepare_info.dice_metadata.push(DiceMetadataD3D11 {
            first_batch_segment_index: self.segment_count, 
            first_global_segment_index: segment_range.start,
            global_path_id,
            pad: 0,
        });
        self.prepare_info.tile_path_info.push(TilePathInfoD3D11 {
            tile_min_x: path.tile_bounds.min_x() as i16,
            tile_min_y: path.tile_bounds.min_y() as i16,
            tile_max_x: path.tile_bounds.max_x() as i16,
            tile_max_y: path.tile_bounds.max_y() as i16,
            first_tile_index: self.tile_count,
            color: path.paint_id.0,
            ctrl: path.ctrl_byte,
            backdrop: 0,
        });

        self.tile_count += path.tile_bounds.area() as u32;
        self.segment_count += segment_range.end - segment_range.start;

        // Handle clip.

        let clip_batch_id = match batch_clip_path_id {
            None => return batch_path_index,
            Some(batch_clip_path_id) => batch_clip_path_id.batch_id,
        };

        if self.clipped_path_info.is_none() {
            self.clipped_path_info = Some(ClippedPathInfo {
                clip_batch_id,
                clipped_path_count: 0,
                max_clipped_tile_count: 0,
                clips: match sink.renderer_level {
                    RendererLevel::D3D9 => Some(vec![]),
                    RendererLevel::D3D11 => None,
                },
            });
        }

        let clipped_path_info = self.clipped_path_info.as_mut().unwrap();
        clipped_path_info.clipped_path_count += 1;
        clipped_path_info.max_clipped_tile_count += path.tile_bounds.area() as u32;

        // If clips are computed on CPU, add them to this batch.
        if let Some(ref mut dest_clips) = clipped_path_info.clips {
            let src_tiles = match path.data {
                BuiltPathData::CPU(BuiltPathBinCPUData {
                    clip_tiles: Some(ref src_tiles),
                    ..
                }) => src_tiles,
                _ => panic!("Clip tiles weren't computed on CPU!"),
            };
            dest_clips.extend_from_slice(&src_tiles.data);
        }

        batch_path_index
    }
}

fn init_backdrops(backdrops: &mut Vec<BackdropInfoD3D11>,
                  path_index: PathBatchIndex,
                  tile_rect: RectI) {
    for tile_x_offset in 0..tile_rect.width() {
        backdrops.push(BackdropInfoD3D11 { initial_backdrop: 0, path_index, tile_x_offset });
    }
}

struct BuiltSegments {
    draw_segments: SegmentsD3D11,
    clip_segments: SegmentsD3D11,
    draw_segment_ranges: Vec<Range<u32>>,
    clip_segment_ranges: Vec<Range<u32>>,
}

impl BuiltSegments {
    fn from_scene(scene: &Scene) -> BuiltSegments {
        let mut built_segments = BuiltSegments {
            draw_segments: SegmentsD3D11::new(),
            clip_segments: SegmentsD3D11::new(),
            draw_segment_ranges: Vec::with_capacity(scene.draw_paths().len()),
            clip_segment_ranges: Vec::with_capacity(scene.clip_paths().len()),
        };

        for clip_path in scene.clip_paths() {
            let range = built_segments.clip_segments.add_path(clip_path.outline());
            built_segments.clip_segment_ranges.push(range);
        }
        for draw_path in scene.draw_paths() {
            let range = built_segments.draw_segments.add_path(draw_path.outline());
            built_segments.draw_segment_ranges.push(range);
        }

        built_segments
    }
}

impl SegmentsD3D11 {
    fn new() -> SegmentsD3D11 {
        SegmentsD3D11 { points: vec![], indices: vec![] }
    }

    fn add_path(&mut self, outline: &Outline) -> Range<u32> {
        let first_segment_index = self.indices.len() as u32;
        for contour in outline.contours() {
            let point_count = contour.len() as u32;
            self.points.reserve(point_count as usize);

            for point_index in 0..point_count {
                if !contour.flags_of(point_index).intersects(PointFlags::CONTROL_POINT_0 |
                                                             PointFlags::CONTROL_POINT_1) {
                    let mut flags = 0;
                    if point_index + 1 < point_count &&
                            contour.flags_of(point_index + 1)
                                   .contains(PointFlags::CONTROL_POINT_0) {
                        if point_index + 2 < point_count &&
                                contour.flags_of(point_index + 2)
                                       .contains(PointFlags::CONTROL_POINT_1) {
                            flags = CURVE_IS_CUBIC
                        } else {
                            flags = CURVE_IS_QUADRATIC
                        }
                    }

                    self.indices.push(SegmentIndicesD3D11 {
                        first_point_index: self.points.len() as u32,
                        flags,
                    });
                }

                self.points.push(contour.position_of(point_index));
            }

            self.points.push(contour.position_of(0));
        }

        let last_segment_index = self.indices.len() as u32;
        first_segment_index..last_segment_index
    }
}

struct TileBatchBuilder {
    prepare_commands: Vec<RenderCommand>,
    draw_commands: Vec<RenderCommand>,
    clip_batches_d3d11: Option<ClipBatchesD3D11>,
    next_batch_id: TileBatchId,
    level: TileBatchBuilderLevel,
}

enum TileBatchBuilderLevel {
    D3D9 { built_paths: BuiltPaths },
    D3D11,
}

impl TileBatchBuilder {
    fn new(built_paths: Option<BuiltPaths>) -> TileBatchBuilder {
        TileBatchBuilder {
            prepare_commands: vec![],
            draw_commands: vec![],
            next_batch_id: TileBatchId(MAX_CLIP_BATCHES),
            clip_batches_d3d11: match built_paths {
                None => {
                    Some(ClipBatchesD3D11 {
                        prepare_batches: vec![],
                        clip_id_to_path_batch_index: FxHashMap::default(),
                    })
                }
                Some(_) => None,
            },
            level: match built_paths {
                None => TileBatchBuilderLevel::D3D11,
                Some(built_paths) => TileBatchBuilderLevel::D3D9 { built_paths },
            },
        }
    }

    fn build_tile_batches_for_draw_path_display_item(&mut self,
                                                     scene: &Scene,
                                                     sink: &SceneSink,
                                                     built_options: &PreparedBuildOptions,
                                                     draw_path_id_range: Range<DrawPathId>,
                                                     paint_metadata: &[PaintMetadata],
                                                     prepare_mode: &PrepareMode) {
        let mut draw_tile_batch = None;
        for draw_path_id in draw_path_id_range.start.0..draw_path_id_range.end.0 {
            let draw_path_id = DrawPathId(draw_path_id);
            let draw_path = match self.level {
                TileBatchBuilderLevel::D3D11 { .. } => {
                    match self.prepare_draw_path_for_gpu_binning(scene,
                                                                 built_options,
                                                                 draw_path_id,
                                                                 prepare_mode,
                                                                 paint_metadata) {
                        None => continue,
                        Some(built_draw_path) => Cow::Owned(built_draw_path),
                    }
                }
                TileBatchBuilderLevel::D3D9 { ref built_paths } => {
                    Cow::Borrowed(&built_paths.draw[draw_path_id.0 as usize])
                }
            };

            // Try to reuse the current batch if we can.
            let flush_needed = match draw_tile_batch {
                Some(DrawTileBatch::D3D11(ref mut existing_batch)) => {
                    !fixup_batch_for_new_path_if_possible(&mut existing_batch.color_texture,
                                                          &draw_path)
                }
                Some(DrawTileBatch::D3D9(ref mut existing_batch)) => {
                    !fixup_batch_for_new_path_if_possible(&mut existing_batch.color_texture,
                                                          &draw_path)
                }
                None => false,
            };

            // If we couldn't reuse the batch, flush it.
            if flush_needed {
                match draw_tile_batch.take() {
                    Some(DrawTileBatch::D3D11(batch_to_flush)) => {
                        self.draw_commands.push(RenderCommand::DrawTilesD3D11(batch_to_flush));
                    }
                    Some(DrawTileBatch::D3D9(batch_to_flush)) => {
                        self.draw_commands.push(RenderCommand::DrawTilesD3D9(batch_to_flush));
                    }
                    _ => {}
                }
            }

            // Create a new batch if necessary.
            if draw_tile_batch.is_none() {
                draw_tile_batch = match self.level {
                    TileBatchBuilderLevel::D3D9 { .. } => {
                        let tile_bounds = tiles::round_rect_out_to_tile_bounds(scene.view_box());
                        Some(DrawTileBatch::D3D9(DrawTileBatchD3D9 {
                            tiles: vec![],
                            clips: vec![],
                            z_buffer_data: DenseTileMap::from_builder(|_| 0, tile_bounds),
                            color_texture: draw_path.color_texture,
                            filter: draw_path.filter,
                            blend_mode: draw_path.blend_mode,
                        }))
                    }
                    TileBatchBuilderLevel::D3D11 { .. } => {
                        Some(DrawTileBatch::D3D11(DrawTileBatchD3D11 {
                            tile_batch_data: TileBatchDataD3D11::new(self.next_batch_id,
                                                                     &prepare_mode,
                                                                     PathSource::Draw),
                            color_texture: draw_path.color_texture,
                        }))
                    }
                };
                self.next_batch_id.0 += 1;
            }

            // Add clip path if necessary.
            let clip_path = match self.clip_batches_d3d11 {
                None => None,
                Some(ref mut clip_batches_d3d11) => {
                    add_clip_path_to_batch(scene,
                                           sink,
                                           built_options,
                                           draw_path.clip_path_id,
                                           prepare_mode,
                                           0,
                                           clip_batches_d3d11)
                }
            };

            let draw_tile_batch = draw_tile_batch.as_mut().unwrap();
            match *draw_tile_batch {
                DrawTileBatch::D3D11(ref mut draw_tile_batch) => {
                    draw_tile_batch.tile_batch_data.push(&draw_path.path,
                                                         draw_path_id.to_path_id(),
                                                         clip_path,
                                                         draw_path.occludes,
                                                         sink);
                }
                DrawTileBatch::D3D9(ref mut draw_tile_batch) => {
                    let built_paths = match self.level {
                        TileBatchBuilderLevel::D3D9 { ref built_paths } => built_paths,
                        TileBatchBuilderLevel::D3D11 { .. } => unreachable!(),
                    };

                    let cpu_data = match built_paths.draw[draw_path_id.0 as usize].path.data {
                        BuiltPathData::CPU(ref cpu_data) => cpu_data,
                        BuiltPathData::GPU | BuiltPathData::TransformCPUBinGPU(_) => {
                            unreachable!()
                        }
                    };

                    for tile in &cpu_data.tiles.data {
                        if tile.alpha_tile_id == AlphaTileId(!0) && tile.backdrop == 0 {
                            continue;
                        }

                        draw_tile_batch.tiles.push(*tile);

                        if !draw_path.occludes || tile.alpha_tile_id != AlphaTileId(!0) {
                            continue;
                        }

                        let tile_coords = vec2i(tile.tile_x as i32, tile.tile_y as i32);
                        let z_value = draw_tile_batch.z_buffer_data
                                                     .get_mut(tile_coords)
                                                     .expect("Z value out of bounds!");
                        *z_value = (*z_value).max(draw_path_id.0 as i32);
                    }

                    let clip_tiles = match cpu_data.clip_tiles {
                        None => continue,
                        Some(ref clip_tiles) => clip_tiles,
                    };
                    for clip_tile in &clip_tiles.data {
                        if clip_tile.dest_tile_id != AlphaTileId(!0) &&
                                clip_tile.src_tile_id != AlphaTileId(!0) {
                            draw_tile_batch.clips.push(*clip_tile);
                        }
                    }
                }
            }
        }

        match draw_tile_batch {
            Some(DrawTileBatch::D3D11(draw_tile_batch)) => {
                self.draw_commands.push(RenderCommand::DrawTilesD3D11(draw_tile_batch));
            }
            Some(DrawTileBatch::D3D9(draw_tile_batch)) => {
                self.draw_commands.push(RenderCommand::DrawTilesD3D9(draw_tile_batch));
            }
            None => {}
        }
    }

    fn prepare_draw_path_for_gpu_binning(&self,
                                         scene: &Scene,
                                         built_options: &PreparedBuildOptions,
                                         draw_path_id: DrawPathId,
                                         prepare_mode: &PrepareMode,
                                         paint_metadata: &[PaintMetadata])
                                         -> Option<BuiltDrawPath> {
        let transform = match *prepare_mode {
            PrepareMode::GPU { transform } => transform,
            PrepareMode::CPU | PrepareMode::TransformCPUBinGPU => {
                panic!("`prepare_draw_path_for_gpu_binning()` requires a GPU prepare mode!")
            }
        };

        let effective_view_box = scene.effective_view_box(built_options);
        let draw_path = scene.get_draw_path(draw_path_id);

        let mut path_bounds = transform * draw_path.outline().bounds();
        match path_bounds.intersection(effective_view_box) {
            Some(intersection) => path_bounds = intersection,
            None => return None,
        }

        let paint_id = draw_path.paint();
        let paint_metadata = &paint_metadata[paint_id.0 as usize];
        let built_path = BuiltPath::new(draw_path_id.to_path_id(),
                                        path_bounds,
                                        effective_view_box,
                                        draw_path.fill_rule(),
                                        &prepare_mode,
                                        draw_path.clip_path(),
                                        &TilingPathInfo::Draw(DrawTilingPathInfo {
                                            paint_id,
                                            blend_mode: draw_path.blend_mode(),
                                            fill_rule: draw_path.fill_rule(),
                                        }));
        Some(BuiltDrawPath::new(built_path, draw_path, paint_metadata))
    }

    fn send_to(self, sink: &SceneSink) {
        if let Some(clip_batches_d3d11) = self.clip_batches_d3d11 {
            for prepare_batch in clip_batches_d3d11.prepare_batches.into_iter().rev() {
                if prepare_batch.path_count > 0 {
                    sink.listener.send(RenderCommand::PrepareClipTilesD3D11(prepare_batch));
                }
            }
        }

        for command in self.prepare_commands {
            sink.listener.send(command);
        }
        for command in self.draw_commands {
            sink.listener.send(command);
        }
    }
}

struct ClipBatchesD3D11 {
    // Will be submitted in reverse (LIFO) order.
    prepare_batches: Vec<TileBatchDataD3D11>,
    clip_id_to_path_batch_index: FxHashMap<ClipPathId, PathBatchIndex>,
}

fn add_clip_path_to_batch(scene: &Scene,
                          sink: &SceneSink,
                          built_options: &PreparedBuildOptions,
                          clip_path_id: Option<ClipPathId>,
                          prepare_mode: &PrepareMode,
                          clip_level: usize,
                          clip_batches_d3d11: &mut ClipBatchesD3D11)
                          -> Option<GlobalPathId> {
    match clip_path_id {
        None => None,
        Some(clip_path_id) => {
            match clip_batches_d3d11.clip_id_to_path_batch_index.get(&clip_path_id) {
                Some(&clip_path_batch_index) => {
                    Some(GlobalPathId {
                        batch_id: TileBatchId(clip_level as u32),
                        path_index: clip_path_batch_index,
                    })
                }
                None => {
                    let PreparedClipPath {
                        built_path: clip_path,
                        subclip_id
                    } = prepare_clip_path_for_gpu_binning(scene,
                                                          sink,
                                                          built_options,
                                                          clip_path_id,
                                                          prepare_mode,
                                                          clip_level,
                                                          clip_batches_d3d11);
                    while clip_level >= clip_batches_d3d11.prepare_batches.len() {
                        let clip_tile_batch_id =
                            TileBatchId(clip_batches_d3d11.prepare_batches.len() as u32);
                        clip_batches_d3d11.prepare_batches
                                          .push(TileBatchDataD3D11::new(clip_tile_batch_id,
                                                                        &prepare_mode,
                                                                        PathSource::Clip));
                    }
                    let clip_path_batch_index =
                        clip_batches_d3d11.prepare_batches[clip_level]
                                          .push(&clip_path,
                                                clip_path_id.to_path_id(),
                                                subclip_id,
                                                true,
                                                sink);
                    clip_batches_d3d11.clip_id_to_path_batch_index.insert(clip_path_id,
                                                                          clip_path_batch_index);
                    Some(GlobalPathId {
                        batch_id: TileBatchId(clip_level as u32),
                        path_index: clip_path_batch_index,
                    })
                }
            }
        }
    }
}

fn prepare_clip_path_for_gpu_binning(scene: &Scene,
                                     sink: &SceneSink,
                                     built_options: &PreparedBuildOptions,
                                     clip_path_id: ClipPathId,
                                     prepare_mode: &PrepareMode,
                                     clip_level: usize,
                                     clip_batches_d3d11: &mut ClipBatchesD3D11)
                                     -> PreparedClipPath {
    let transform = match *prepare_mode {
        PrepareMode::GPU { transform } => transform,
        PrepareMode::CPU | PrepareMode::TransformCPUBinGPU => {
            panic!("`prepare_clip_path_for_gpu_binning()` requires a GPU prepare mode!")
        }
    };
    let effective_view_box = scene.effective_view_box(built_options);
    let clip_path = scene.get_clip_path(clip_path_id);

    // Add subclip path if necessary.
    let subclip_id = add_clip_path_to_batch(scene,
                                            sink,
                                            built_options,
                                            clip_path.clip_path(),
                                            prepare_mode,
                                            clip_level + 1,
                                            clip_batches_d3d11);

    let path_bounds = transform * clip_path.outline().bounds();

    // TODO(pcwalton): Clip to view box!

    let built_path = BuiltPath::new(clip_path_id.to_path_id(),
                                    path_bounds,
                                    effective_view_box,
                                    clip_path.fill_rule(),
                                    &prepare_mode,
                                    clip_path.clip_path(),
                                    &TilingPathInfo::Clip);

    PreparedClipPath { built_path, subclip_id }
}

struct PreparedClipPath {
    built_path: BuiltPath,
    subclip_id: Option<GlobalPathId>,
}

fn fixup_batch_for_new_path_if_possible(batch_color_texture: &mut Option<TileBatchTexture>,
                                        draw_path: &BuiltDrawPath)
                                        -> bool {
    if draw_path.color_texture.is_some() {
        if batch_color_texture.is_none() {
            *batch_color_texture = draw_path.color_texture;
            return true;
        }
        if draw_path.color_texture != *batch_color_texture {
            debug!("batch break: path color texture {:?} batch color texture {:?}",
                   draw_path.color_texture,
                   batch_color_texture);
            return false;
        }
    }
    true
}
