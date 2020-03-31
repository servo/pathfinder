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
use crate::gpu::renderer::{BlendModeExt, MASK_TILES_ACROSS, MASK_TILES_DOWN};
use crate::gpu_data::{FillBatchPrimitive, RenderCommand, TexturePageId, Tile, TileBatch};
use crate::gpu_data::{TileBatchTexture, TileObjectPrimitive, TileVertex};
use crate::options::{PreparedBuildOptions, RenderCommandListener};
use crate::paint::{PaintInfo, PaintMetadata, RenderTargetMetadata};
use crate::scene::{DisplayItem, Scene};
use crate::tile_map::DenseTileMap;
use crate::tiles::{self, DrawTilingPathInfo, PackedTile, TILE_HEIGHT, TILE_WIDTH};
use crate::tiles::{Tiler, TilingPathInfo};
use crate::z_buffer::{DepthMetadata, ZBuffer};
use pathfinder_content::effects::{BlendMode, Effects};
use pathfinder_content::fill::FillRule;
use pathfinder_content::render_target::RenderTargetId;
use pathfinder_geometry::line_segment::{LineSegment2F, LineSegmentU4, LineSegmentU8};
use pathfinder_geometry::rect::{RectF, RectI};
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::util;
use pathfinder_geometry::vector::{Vector2F, Vector2I, vec2f, vec2i};
use pathfinder_gpu::TextureSamplingFlags;
use pathfinder_simd::default::{F32x4, I32x4};
use std::sync::atomic::{AtomicUsize, Ordering};
use instant::Instant;
use std::u16;

pub(crate) struct SceneBuilder<'a> {
    scene: &'a Scene,
    built_options: &'a PreparedBuildOptions,

    next_alpha_tile_index: AtomicUsize,

    pub(crate) listener: Box<dyn RenderCommandListener>,
}

#[derive(Debug)]
pub(crate) struct ObjectBuilder {
    pub built_path: BuiltPath,
    pub fills: Vec<FillBatchPrimitive>,
    pub bounds: RectF,
}

#[derive(Debug)]
struct BuiltDrawPath {
    path: BuiltPath,
    blend_mode: BlendMode,
    effects: Effects,
    color_texture_page_0: TexturePageId,
    color_texture_page_1: TexturePageId,
    sampling_flags_0: TextureSamplingFlags,
    sampling_flags_1: TextureSamplingFlags,
    mask_0_fill_rule: FillRule,
    mask_1_fill_rule: Option<FillRule>,
}

#[derive(Debug)]
pub(crate) struct BuiltPath {
    pub solid_tiles: SolidTiles,
    pub empty_tiles: Vec<Tile>,
    pub single_mask_tiles: Vec<Tile>,
    pub dual_mask_tiles: Vec<Tile>,
    pub tiles: DenseTileMap<TileObjectPrimitive>,
    pub fill_rule: FillRule,
}

#[derive(Clone, Debug)]
pub(crate) enum SolidTiles {
    Occluders(Vec<Occluder>),
    Regular(Vec<Tile>),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Occluder {
    pub(crate) coords: Vector2I,
}

impl<'a> SceneBuilder<'a> {
    pub(crate) fn new(
        scene: &'a Scene,
        built_options: &'a PreparedBuildOptions,
        listener: Box<dyn RenderCommandListener>,
    ) -> SceneBuilder<'a> {
        SceneBuilder {
            scene,
            built_options,
            next_alpha_tile_index: AtomicUsize::new(0),
            listener,
        }
    }

    pub fn build<E>(&mut self, executor: &E) where E: Executor {
        let start_time = Instant::now();

        // Send the start rendering command.
        let bounding_quad = self.built_options.bounding_quad();

        let clip_path_count = self.scene.clip_paths.len();
        let draw_path_count = self.scene.paths.len();
        let total_path_count = clip_path_count + draw_path_count;

        let needs_readable_framebuffer = self.needs_readable_framebuffer();

        self.listener.send(RenderCommand::Start {
            bounding_quad,
            path_count: total_path_count,
            needs_readable_framebuffer,
        });

        // Build paint data.
        let PaintInfo {
            render_commands,
            paint_metadata,
            render_target_metadata,
            opacity_tile_page,
            opacity_tile_transform,
        } = self.scene.build_paint_info();
        for render_command in render_commands {
            self.listener.send(render_command);
        }

        let effective_view_box = self.scene.effective_view_box(self.built_options);

        let built_clip_paths = executor.build_vector(clip_path_count, |path_index| {
            self.build_clip_path(PathBuildParams {
                path_index,
                view_box: effective_view_box,
                built_options: &self.built_options,
                scene: &self.scene,
            })
        });

        let built_draw_paths = executor.build_vector(draw_path_count, |path_index| {
            self.build_draw_path(DrawPathBuildParams {
                path_build_params: PathBuildParams {
                    path_index,
                    view_box: effective_view_box,
                    built_options: &self.built_options,
                    scene: &self.scene,
                },
                paint_metadata: &paint_metadata,
                opacity_tile_page,
                opacity_tile_transform,
                built_clip_paths: &built_clip_paths,
            })
        });

        self.finish_building(&paint_metadata, &render_target_metadata, built_draw_paths);

        let build_time = Instant::now() - start_time;
        self.listener.send(RenderCommand::Finish { build_time });
    }

    fn build_clip_path(&self, params: PathBuildParams) -> BuiltPath {
        let PathBuildParams { path_index, view_box, built_options, scene } = params;
        let path_object = &scene.clip_paths[path_index];
        let outline = scene.apply_render_options(path_object.outline(), built_options);

        let mut tiler = Tiler::new(self,
                                   &outline,
                                   path_object.fill_rule(),
                                   view_box,
                                   TilingPathInfo::Clip);

        tiler.generate_tiles();

        self.listener.send(RenderCommand::AddFills(tiler.object_builder.fills));
        tiler.object_builder.built_path
    }

    fn build_draw_path(&self, params: DrawPathBuildParams) -> BuiltDrawPath {
        let DrawPathBuildParams {
            path_build_params: PathBuildParams { path_index, view_box, built_options, scene },
            paint_metadata,
            opacity_tile_page,
            opacity_tile_transform,
            built_clip_paths,
        } = params;

        let path_object = &scene.paths[path_index];
        let outline = scene.apply_render_options(path_object.outline(), built_options);

        let paint_id = path_object.paint();
        let paint_metadata = &paint_metadata[paint_id.0 as usize];
        let built_clip_path = path_object.clip_path().map(|clip_path_id| {
            &built_clip_paths[clip_path_id.0 as usize]
        });

        let mut tiler = Tiler::new(self,
                                   &outline,
                                   path_object.fill_rule(),
                                   view_box,
                                   TilingPathInfo::Draw(DrawTilingPathInfo {
            paint_metadata,
            opacity_tile_transform,
            blend_mode: path_object.blend_mode(),
            opacity: path_object.opacity(),
            built_clip_path,
        }));

        tiler.generate_tiles();

        self.listener.send(RenderCommand::AddFills(tiler.object_builder.fills));

        BuiltDrawPath {
            path: tiler.object_builder.built_path,
            blend_mode: path_object.blend_mode(),
            effects: paint_metadata.effects(),
            color_texture_page_0: paint_metadata.location.page,
            sampling_flags_0: paint_metadata.sampling_flags,
            color_texture_page_1: opacity_tile_page,
            sampling_flags_1: TextureSamplingFlags::empty(),
            mask_0_fill_rule: path_object.fill_rule(),
            mask_1_fill_rule: built_clip_path.map(|_| FillRule::Winding),
        }
    }

    fn cull_tiles(&self,
                  paint_metadata: &[PaintMetadata],
                  render_target_metadata: &[RenderTargetMetadata],
                  built_draw_paths: Vec<BuiltDrawPath>)
                  -> CulledTiles {
        let mut culled_tiles = CulledTiles { display_list: vec![] };

        let mut remaining_layer_z_buffers = self.build_solid_tiles(&built_draw_paths);
        remaining_layer_z_buffers.reverse();

        // Process first Z-buffer.
        let first_z_buffer = remaining_layer_z_buffers.pop().unwrap();
        let first_solid_tiles = first_z_buffer.build_solid_tiles(paint_metadata);
        for batch in first_solid_tiles.batches {
            culled_tiles.display_list.push(CulledDisplayItem::DrawTiles(batch));
        }

        let mut layer_z_buffers_stack = vec![first_z_buffer];
        let mut current_depth = 1;

        for display_item in &self.scene.display_list {
            match *display_item {
                DisplayItem::PushRenderTarget(render_target_id) => {
                    culled_tiles.display_list
                                .push(CulledDisplayItem::PushRenderTarget(render_target_id));

                    let z_buffer = remaining_layer_z_buffers.pop().unwrap();
                    let solid_tiles = z_buffer.build_solid_tiles(paint_metadata);
                    for batch in solid_tiles.batches {
                        culled_tiles.display_list.push(CulledDisplayItem::DrawTiles(batch));
                    }
                    layer_z_buffers_stack.push(z_buffer);
                }

                DisplayItem::PopRenderTarget => {
                    culled_tiles.display_list.push(CulledDisplayItem::PopRenderTarget);
                    layer_z_buffers_stack.pop();
                }

                DisplayItem::DrawRenderTarget { render_target, effects } => {
                    let effective_view_box = self.scene.effective_view_box(self.built_options);
                    let tile_rect = tiles::round_rect_out_to_tile_bounds(effective_view_box);
                    let layer_z_buffer = layer_z_buffers_stack.last().unwrap();
                    let mut tiles = vec![];
                    let uv_scale = vec2f(1.0, 1.0) / tile_rect.lower_right().to_f32();
                    let metadata = &render_target_metadata[render_target.0 as usize];
                    for tile_y in tile_rect.min_y()..tile_rect.max_y() {
                        for tile_x in tile_rect.min_x()..tile_rect.max_x() {
                            let tile_coords = vec2i(tile_x, tile_y);
                            if !layer_z_buffer.test(tile_coords, current_depth) {
                                continue;
                            }

                            let uv_rect =
                                RectI::new(tile_coords, Vector2I::splat(1)).to_f32()
                                                                           .scale_xy(uv_scale);
                            tiles.push(Tile::new_solid_from_texture_rect(tile_coords, uv_rect));
                        }
                    }
                    let batch = TileBatch {
                        tiles,
                        color_texture_0: Some(TileBatchTexture {
                            page: metadata.location.page,
                            sampling_flags: TextureSamplingFlags::empty(),
                        }),
                        color_texture_1: None,
                        effects,
                        blend_mode: BlendMode::SrcOver,
                        mask_0_fill_rule: None,
                        mask_1_fill_rule: None,
                    };
                    culled_tiles.display_list.push(CulledDisplayItem::DrawTiles(batch));
                    current_depth += 1;
                }

                DisplayItem::DrawPaths {
                    start_index: start_draw_path_index,
                    end_index: end_draw_path_index,
                } => {
                    for draw_path_index in start_draw_path_index..end_draw_path_index {
                        let built_draw_path = &built_draw_paths[draw_path_index as usize];
                        let layer_z_buffer = layer_z_buffers_stack.last().unwrap();
                        let color_texture_0 = Some(TileBatchTexture {
                            page: built_draw_path.color_texture_page_0,
                            sampling_flags: built_draw_path.sampling_flags_0,
                        });
                        let color_texture_1 = Some(TileBatchTexture {
                            page: built_draw_path.color_texture_page_1,
                            sampling_flags: built_draw_path.sampling_flags_1,
                        });

                        debug_assert!(built_draw_path.path.empty_tiles.is_empty() ||
                                      built_draw_path.blend_mode.is_destructive());
                        self.add_alpha_tiles(&mut culled_tiles,
                                             layer_z_buffer,
                                             &built_draw_path.path.empty_tiles,
                                             current_depth,
                                             None,
                                             None,
                                             built_draw_path.blend_mode,
                                             built_draw_path.effects,
                                             None,
                                             None);

                        self.add_alpha_tiles(&mut culled_tiles,
                                             layer_z_buffer,
                                             &built_draw_path.path.single_mask_tiles,
                                             current_depth,
                                             color_texture_0,
                                             color_texture_1,
                                             built_draw_path.blend_mode,
                                             built_draw_path.effects,
                                             Some(built_draw_path.mask_0_fill_rule),
                                             None);

                        if let Some(mask_1_fill_rule) = built_draw_path.mask_1_fill_rule {
                            self.add_alpha_tiles(&mut culled_tiles,
                                                 layer_z_buffer,
                                                 &built_draw_path.path.dual_mask_tiles,
                                                 current_depth,
                                                 color_texture_0,
                                                 color_texture_1,
                                                 built_draw_path.blend_mode,
                                                 built_draw_path.effects,
                                                 Some(built_draw_path.mask_0_fill_rule),
                                                 Some(mask_1_fill_rule));
                        }

                        match built_draw_path.path.solid_tiles {
                            SolidTiles::Regular(ref tiles) => {
                                self.add_alpha_tiles(&mut culled_tiles,
                                                     layer_z_buffer,
                                                     tiles,
                                                     current_depth,
                                                     color_texture_0,
                                                     color_texture_1,
                                                     built_draw_path.blend_mode,
                                                     built_draw_path.effects,
                                                     None,
                                                     built_draw_path.mask_1_fill_rule);
                            }
                            SolidTiles::Occluders(_) => {}
                        }

                        current_depth += 1;
                    }
                }
            }
        }

        culled_tiles
    }

    fn build_solid_tiles(&self, built_draw_paths: &[BuiltDrawPath]) -> Vec<ZBuffer> {
        let effective_view_box = self.scene.effective_view_box(self.built_options);
        let mut z_buffers = vec![ZBuffer::new(effective_view_box)];
        let mut z_buffer_index_stack = vec![0];
        let mut current_depth = 1;

        // Create Z-buffers.
        for display_item in &self.scene.display_list {
            match *display_item {
                DisplayItem::PushRenderTarget { .. } => {
                    z_buffer_index_stack.push(z_buffers.len());
                    z_buffers.push(ZBuffer::new(effective_view_box));
                }
                DisplayItem::PopRenderTarget => {
                    z_buffer_index_stack.pop();
                }
                DisplayItem::DrawPaths { start_index, end_index } => {
                    let (start_index, end_index) = (start_index as usize, end_index as usize);
                    let z_buffer = &mut z_buffers[*z_buffer_index_stack.last().unwrap()];
                    for (path_subindex, built_draw_path) in
                            built_draw_paths[start_index..end_index].iter().enumerate() {
                        let path_index = (path_subindex + start_index) as u32;
                        let path = &self.scene.paths[path_index as usize];
                        let metadata = DepthMetadata { paint_id: path.paint() };
                        match built_draw_path.path.solid_tiles {
                            SolidTiles::Regular(_) => {
                                z_buffer.update(&[], current_depth, metadata);
                            }
                            SolidTiles::Occluders(ref occluders) => {
                                z_buffer.update(occluders, current_depth, metadata);
                            }
                        }
                        current_depth += 1;
                    }
                }
                DisplayItem::DrawRenderTarget { .. } => {
                    // FIXME(pcwalton): Not great that this doesn't participate in Z-buffering!
                    current_depth += 1;
                }
            }
        }
        debug_assert_eq!(z_buffer_index_stack.len(), 1);

        z_buffers
    }

    fn add_alpha_tiles(&self,
                       culled_tiles: &mut CulledTiles,
                       layer_z_buffer: &ZBuffer,
                       alpha_tiles: &[Tile],
                       current_depth: u32,
                       color_texture_0: Option<TileBatchTexture>,
                       color_texture_1: Option<TileBatchTexture>,
                       blend_mode: BlendMode,
                       effects: Effects,
                       mask_0_fill_rule: Option<FillRule>,
                       mask_1_fill_rule: Option<FillRule>) {
        if alpha_tiles.is_empty() {
            return;
        }

        // Create a new `DrawTiles` display item if we don't have one or if we have to break a
        // batch due to blend mode or paint page. Note that every path with a blend mode that
        // requires a readable framebuffer needs its own batch.
        //
        // TODO(pcwalton): If we really wanted to, we could use tile maps to avoid
        // batch breaks in some cases…
        match culled_tiles.display_list.last() {
            Some(&CulledDisplayItem::DrawTiles(TileBatch {
                tiles: _,
                color_texture_0: ref batch_color_texture_0,
                color_texture_1: ref batch_color_texture_1,
                blend_mode: batch_blend_mode,
                effects: batch_effects,
                mask_0_fill_rule: batch_mask_0_fill_rule,
                mask_1_fill_rule: batch_mask_1_fill_rule,
            })) if *batch_color_texture_0 == color_texture_0 &&
                *batch_color_texture_1 == color_texture_1 &&
                batch_blend_mode == blend_mode &&
                batch_effects == effects &&
                batch_mask_0_fill_rule == mask_0_fill_rule &&
                batch_mask_1_fill_rule == mask_1_fill_rule &&
                !batch_blend_mode.needs_readable_framebuffer() => {}
            _ => {
                let batch = TileBatch {
                    tiles: vec![],
                    color_texture_0,
                    color_texture_1,
                    blend_mode,
                    effects,
                    mask_0_fill_rule,
                    mask_1_fill_rule,
                };
                culled_tiles.display_list.push(CulledDisplayItem::DrawTiles(batch))
            }
        }

        // Fetch the destination alpha tiles buffer.
        let culled_alpha_tiles = match *culled_tiles.display_list.last_mut().unwrap() {
            CulledDisplayItem::DrawTiles(TileBatch { tiles: ref mut culled_alpha_tiles, .. }) => {
                culled_alpha_tiles
            }
            _ => unreachable!(),
        };

        for alpha_tile in alpha_tiles {
            let alpha_tile_coords = alpha_tile.upper_left.tile_position();
            if layer_z_buffer.test(alpha_tile_coords, current_depth) {
                culled_alpha_tiles.push(*alpha_tile);
            }
        }
    }

    fn pack_tiles(&mut self, culled_tiles: CulledTiles) {
        for display_item in culled_tiles.display_list {
            match display_item {
                CulledDisplayItem::DrawTiles(batch) => {
                    self.listener.send(RenderCommand::DrawTiles(batch))
                }
                CulledDisplayItem::PushRenderTarget(render_target_id) => {
                    self.listener.send(RenderCommand::PushRenderTarget(render_target_id))
                }
                CulledDisplayItem::PopRenderTarget => {
                    self.listener.send(RenderCommand::PopRenderTarget)
                }
            }
        }
    }

    fn finish_building(&mut self,
                       paint_metadata: &[PaintMetadata],
                       render_target_metadata: &[RenderTargetMetadata],
                       built_draw_paths: Vec<BuiltDrawPath>) {
        self.listener.send(RenderCommand::FlushFills);
        let culled_tiles = self.cull_tiles(paint_metadata,
                                           render_target_metadata,
                                           built_draw_paths);
        self.pack_tiles(culled_tiles);
    }

    fn needs_readable_framebuffer(&self) -> bool {
        let mut framebuffer_nesting = 0;
        for display_item in &self.scene.display_list {
            match *display_item {
                DisplayItem::DrawRenderTarget { .. } => {}
                DisplayItem::PushRenderTarget(_) => framebuffer_nesting += 1,
                DisplayItem::PopRenderTarget => framebuffer_nesting -= 1,
                DisplayItem::DrawPaths { start_index, end_index } => {
                    if framebuffer_nesting > 0 {
                        continue;
                    }
                    for path_index in start_index..end_index {
                        let blend_mode = self.scene.paths[path_index as usize].blend_mode();
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

struct PathBuildParams<'a> {
    path_index: usize,
    view_box: RectF,
    built_options: &'a PreparedBuildOptions,
    scene: &'a Scene,
}

struct DrawPathBuildParams<'a> {
    path_build_params: PathBuildParams<'a>,
    paint_metadata: &'a [PaintMetadata],
    opacity_tile_page: TexturePageId,
    opacity_tile_transform: Transform2F,
    built_clip_paths: &'a [BuiltPath],
}

impl BuiltPath {
    fn new(path_bounds: RectF,
           view_box_bounds: RectF,
           fill_rule: FillRule,
           tiling_path_info: &TilingPathInfo)
           -> BuiltPath {
        let occludes = match *tiling_path_info {
            TilingPathInfo::Draw(ref draw_tiling_path_info) => {
                draw_tiling_path_info.paint_metadata.is_opaque &&
                    draw_tiling_path_info.blend_mode.occludes_backdrop() &&
                    draw_tiling_path_info.opacity == !0
            }
            TilingPathInfo::Clip => true,
        };

        let tile_map_bounds = if tiling_path_info.has_destructive_blend_mode() {
            view_box_bounds
        } else {
            path_bounds
        };

        BuiltPath {
            single_mask_tiles: vec![],
            dual_mask_tiles: vec![],
            empty_tiles: vec![],
            solid_tiles: if occludes {
                SolidTiles::Occluders(vec![])
            } else {
                SolidTiles::Regular(vec![])
            },
            tiles: DenseTileMap::new(tiles::round_rect_out_to_tile_bounds(tile_map_bounds)),
            fill_rule,
        }
    }
}

impl Occluder {
    #[inline]
    pub(crate) fn new(coords: Vector2I) -> Occluder {
        Occluder { coords }
    }
}

struct CulledTiles {
    display_list: Vec<CulledDisplayItem>,
}

enum CulledDisplayItem {
    DrawTiles(TileBatch),
    PushRenderTarget(RenderTargetId),
    PopRenderTarget,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TileStats {
    pub solid_tile_count: u32,
    pub alpha_tile_count: u32,
}

// Utilities for built objects

impl ObjectBuilder {
    pub(crate) fn new(path_bounds: RectF,
                      view_box_bounds: RectF,
                      fill_rule: FillRule,
                      tiling_path_info: &TilingPathInfo)
                      -> ObjectBuilder {
        ObjectBuilder {
            built_path: BuiltPath::new(path_bounds, view_box_bounds, fill_rule, tiling_path_info),
            bounds: path_bounds,
            fills: vec![],
        }
    }

    #[inline]
    pub(crate) fn tile_rect(&self) -> RectI {
        self.built_path.tiles.rect
    }

    fn add_fill(
        &mut self,
        scene_builder: &SceneBuilder,
        segment: LineSegment2F,
        tile_coords: Vector2I,
    ) {
        debug!("add_fill({:?} ({:?}))", segment, tile_coords);

        // Ensure this fill is in bounds. If not, cull it.
        if self.tile_coords_to_local_index(tile_coords).is_none() {
            return;
        };

        debug_assert_eq!(TILE_WIDTH, TILE_HEIGHT);

        // Compute the upper left corner of the tile.
        let tile_size = F32x4::splat(TILE_WIDTH as f32);
        let tile_upper_left = tile_coords.to_f32().0.to_f32x4().xyxy() * tile_size;

        // Convert to 4.8 fixed point.
        let segment = (segment.0 - tile_upper_left) * F32x4::splat(256.0);
        let (min, max) = (F32x4::default(), F32x4::splat((TILE_WIDTH * 256 - 1) as f32));
        let segment = segment.clamp(min, max).to_i32x4();
        let (from_x, from_y, to_x, to_y) = (segment[0], segment[1], segment[2], segment[3]);

        // Cull degenerate fills.
        if from_x == to_x {
            debug!("... culling!");
            return;
        }

        // Allocate global tile if necessary.
        let alpha_tile_index = self.get_or_allocate_alpha_tile_index(scene_builder, tile_coords);

        // Pack whole pixels.
        let px = (segment & I32x4::splat(0xf00)).to_u32x4();
        let px = (px >> 8).to_i32x4() | (px >> 4).to_i32x4().yxwz();

        // Pack instance data.
        debug!("... OK, pushing");
        self.fills.push(FillBatchPrimitive {
            px: LineSegmentU4 { from: px[0] as u8, to: px[2] as u8 },
            subpx: LineSegmentU8 {
                from_x: from_x as u8,
                from_y: from_y as u8,
                to_x:   to_x   as u8,
                to_y:   to_y   as u8,
            },
            alpha_tile_index,
        });
    }

    fn get_or_allocate_alpha_tile_index(
        &mut self,
        scene_builder: &SceneBuilder,
        tile_coords: Vector2I,
    ) -> u16 {
        let local_tile_index = self.built_path.tiles.coords_to_index_unchecked(tile_coords);
        let alpha_tile_index = self.built_path.tiles.data[local_tile_index].alpha_tile_index;
        if alpha_tile_index != !0 {
            return alpha_tile_index;
        }

        // FIXME(pcwalton): Check for overflow!
        let alpha_tile_index = scene_builder
            .next_alpha_tile_index
            .fetch_add(1, Ordering::Relaxed) as u16;
        self.built_path.tiles.data[local_tile_index].alpha_tile_index = alpha_tile_index;
        alpha_tile_index
    }

    pub(crate) fn add_active_fill(
        &mut self,
        scene_builder: &SceneBuilder,
        left: f32,
        right: f32,
        mut winding: i32,
        tile_coords: Vector2I,
    ) {
        let tile_origin_y = (tile_coords.y() * TILE_HEIGHT as i32) as f32;
        let left = vec2f(left, tile_origin_y);
        let right = vec2f(right, tile_origin_y);

        let segment = if winding < 0 {
            LineSegment2F::new(left, right)
        } else {
            LineSegment2F::new(right, left)
        };

        debug!(
            "... emitting active fill {} -> {} winding {} @ tile {:?}",
            left.x(),
            right.x(),
            winding,
            tile_coords
        );

        while winding != 0 {
            self.add_fill(scene_builder, segment, tile_coords);
            if winding < 0 {
                winding += 1
            } else {
                winding -= 1
            }
        }
    }

    pub(crate) fn generate_fill_primitives_for_line(
        &mut self,
        scene_builder: &SceneBuilder,
        mut segment: LineSegment2F,
        tile_y: i32,
    ) {
        debug!(
            "... generate_fill_primitives_for_line(): segment={:?} tile_y={} ({}-{})",
            segment,
            tile_y,
            tile_y as f32 * TILE_HEIGHT as f32,
            (tile_y + 1) as f32 * TILE_HEIGHT as f32
        );

        let winding = segment.from_x() > segment.to_x();
        let (segment_left, segment_right) = if !winding {
            (segment.from_x(), segment.to_x())
        } else {
            (segment.to_x(), segment.from_x())
        };

        // FIXME(pcwalton): Optimize this.
        let segment_tile_left = f32::floor(segment_left) as i32 / TILE_WIDTH as i32;
        let segment_tile_right =
            util::alignup_i32(f32::ceil(segment_right) as i32, TILE_WIDTH as i32);
        debug!(
            "segment_tile_left={} segment_tile_right={} tile_rect={:?}",
            segment_tile_left,
            segment_tile_right,
            self.tile_rect()
        );

        for subsegment_tile_x in segment_tile_left..segment_tile_right {
            let (mut fill_from, mut fill_to) = (segment.from(), segment.to());
            let subsegment_tile_right =
                ((i32::from(subsegment_tile_x) + 1) * TILE_HEIGHT as i32) as f32;
            if subsegment_tile_right < segment_right {
                let x = subsegment_tile_right;
                let point = Vector2F::new(x, segment.solve_y_for_x(x));
                if !winding {
                    fill_to = point;
                    segment = LineSegment2F::new(point, segment.to());
                } else {
                    fill_from = point;
                    segment = LineSegment2F::new(segment.from(), point);
                }
            }

            let fill_segment = LineSegment2F::new(fill_from, fill_to);
            let fill_tile_coords = vec2i(subsegment_tile_x, tile_y);
            self.add_fill(scene_builder, fill_segment, fill_tile_coords);
        }
    }

    #[inline]
    pub(crate) fn tile_coords_to_local_index(&self, coords: Vector2I) -> Option<u32> {
        self.built_path.tiles.coords_to_index(coords).map(|index| index as u32)
    }

    #[inline]
    pub(crate) fn local_tile_index_to_coords(&self, tile_index: u32) -> Vector2I {
        self.built_path.tiles.index_to_coords(tile_index as usize)
    }
}

impl<'a> PackedTile<'a> {
    pub(crate) fn add_to(&self,
                         tiles: &mut Vec<Tile>,
                         draw_tiling_path_info: &DrawTilingPathInfo) {
        let fill_tile_index = self.draw_tile.alpha_tile_index as u16;
        let fill_tile_backdrop = self.draw_tile.backdrop as i16;
        let (clip_tile_index, clip_tile_backdrop) = match self.clip_tile {
            None => (0, 0),
            Some(clip_tile) => (clip_tile.alpha_tile_index as u16, clip_tile.backdrop as i16),
        };

        tiles.push(Tile {
            upper_left: TileVertex::new_alpha(self.tile_coords,
                                              fill_tile_index,
                                              fill_tile_backdrop,
                                              clip_tile_index,
                                              clip_tile_backdrop,
                                              Vector2I::zero(),
                                              draw_tiling_path_info),
            upper_right: TileVertex::new_alpha(self.tile_coords,
                                               fill_tile_index,
                                               fill_tile_backdrop,
                                               clip_tile_index,
                                               clip_tile_backdrop,
                                               vec2i(1, 0),
                                               draw_tiling_path_info),
            lower_left: TileVertex::new_alpha(self.tile_coords,
                                              fill_tile_index,
                                              fill_tile_backdrop,
                                              clip_tile_index,
                                              clip_tile_backdrop,
                                              vec2i(0, 1),
                                              draw_tiling_path_info),
            lower_right: TileVertex::new_alpha(self.tile_coords,
                                               fill_tile_index,
                                               fill_tile_backdrop,
                                               clip_tile_index,
                                               clip_tile_backdrop,
                                               vec2i(1, 1),
                                               draw_tiling_path_info),
        });
    }
}

impl TileVertex {
    #[inline]
    fn new_alpha(tile_origin: Vector2I,
                 draw_tile_index: u16,
                 draw_tile_backdrop: i16,
                 clip_tile_index: u16,
                 clip_tile_backdrop: i16,
                 tile_offset: Vector2I,
                 draw_tiling_path_info: &DrawTilingPathInfo)
                 -> TileVertex {
        // TODO(pcwalton): Opacity.
        let tile_position = tile_origin + tile_offset;
        let color_0_uv = draw_tiling_path_info.paint_metadata.calculate_tex_coords(tile_position);
        let color_1_uv = calculate_opacity_uv(draw_tiling_path_info);
        let mask_0_uv = calculate_mask_uv(draw_tile_index, tile_offset);
        let mask_1_uv = calculate_mask_uv(clip_tile_index, tile_offset);
        TileVertex {
            tile_x: tile_position.x() as i16,
            tile_y: tile_position.y() as i16,
            color_0_u: color_0_uv.x(),
            color_0_v: color_0_uv.y(),
            color_1_u: color_1_uv.x(),
            color_1_v: color_1_uv.y(),
            mask_0_u: mask_0_uv.x(),
            mask_0_v: mask_0_uv.y(),
            mask_1_u: mask_1_uv.x(),
            mask_1_v: mask_1_uv.y(),
            mask_0_backdrop: draw_tile_backdrop,
            mask_1_backdrop: clip_tile_backdrop,
        }
    }

    #[inline]
    pub fn tile_position(&self) -> Vector2I {
        vec2i(self.tile_x as i32, self.tile_y as i32)
    }
}

fn calculate_mask_uv(tile_index: u16, tile_offset: Vector2I) -> Vector2F {
    let mask_u = tile_index as i32 % MASK_TILES_ACROSS as i32;
    let mask_v = tile_index as i32 / MASK_TILES_ACROSS as i32;
    let scale = vec2f(1.0 / MASK_TILES_ACROSS as f32, 1.0 / MASK_TILES_DOWN as f32);
    (vec2i(mask_u, mask_v) + tile_offset).to_f32().scale_xy(scale)
}

fn calculate_opacity_uv(draw_tiling_path_info: &DrawTilingPathInfo) -> Vector2F {
    let DrawTilingPathInfo { opacity_tile_transform, opacity, .. } = *draw_tiling_path_info;
    let texel_coord = (vec2i((opacity % 16) as i32, (opacity / 16) as i32).to_f32() +
                       vec2f(0.5, 0.5)).scale(1.0 / 16.0);
    opacity_tile_transform * texel_coord
}
