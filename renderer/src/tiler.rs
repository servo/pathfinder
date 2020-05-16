// pathfinder/renderer/src/tiler.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implements the fast lattice-clipping algorithm from Nehab and Hoppe, "Random-Access Rendering
//! of General Vector Graphics" 2006.

use crate::builder::{ObjectBuilder, Occluder, SceneBuilder, SolidTiles};
use crate::tiles::{PackedTile, TILE_HEIGHT, TILE_WIDTH, TileType, TilingPathInfo};
use pathfinder_content::fill::FillRule;
use pathfinder_content::outline::{ContourIterFlags, Outline};
use pathfinder_content::segment::Segment;
use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{Vector2F, Vector2I, vec2f, vec2i};
use pathfinder_simd::default::{F32x2, U32x2};

const FLATTENING_TOLERANCE: f32 = 0.25;

pub(crate) struct Tiler<'a, 'b> {
    scene_builder: &'a SceneBuilder<'b, 'a>,
    pub(crate) object_builder: ObjectBuilder,
    outline: &'a Outline,
    path_info: TilingPathInfo<'a>,
}

impl<'a, 'b> Tiler<'a, 'b> {
    pub(crate) fn new(scene_builder: &'a SceneBuilder<'b, 'a>,
                      outline: &'a Outline,
                      fill_rule: FillRule,
                      view_box: RectF,
                      path_info: TilingPathInfo<'a>)
                      -> Tiler<'a, 'b> {
        let bounds = outline.bounds().intersection(view_box).unwrap_or(RectF::default());
        let object_builder = ObjectBuilder::new(bounds, view_box, fill_rule, &path_info);
        Tiler { scene_builder, object_builder, outline, path_info }
    }

    pub(crate) fn generate_tiles(&mut self) {
        for contour in self.outline.contours() {
            for segment in contour.iter(ContourIterFlags::empty()) {
                process_segment(&segment, self.scene_builder, &mut self.object_builder);
            }
        }

        self.propagate_backdrops();
        self.pack_and_cull();
    }

    fn propagate_backdrops(&mut self) {
        let tiles_across = self.object_builder.built_path.tiles.rect.width() as usize;
        for (draw_tile_index, draw_tile) in self.object_builder
                                                .built_path
                                                .tiles
                                                .data
                                                .iter_mut()
                                                .enumerate() {
            let column = draw_tile_index % tiles_across;
            let delta = draw_tile.backdrop;
            draw_tile.backdrop = self.object_builder.current_backdrops[column];
            self.object_builder.current_backdrops[column] += delta;
        }
    }

    fn pack_and_cull(&mut self) {
        let draw_tiling_path_info = match self.path_info {
            TilingPathInfo::Clip => return,
            TilingPathInfo::Draw(draw_tiling_path_info) => draw_tiling_path_info,
        };

        let blend_mode_is_destructive = draw_tiling_path_info.blend_mode.is_destructive();

        for (draw_tile_index, draw_tile) in self.object_builder
                                                .built_path
                                                .tiles
                                                .data
                                                .iter()
                                                .enumerate() {
            let packed_tile = PackedTile::new(draw_tile_index as u32,
                                              draw_tile,
                                              &draw_tiling_path_info,
                                              &self.object_builder);

            match packed_tile.tile_type {
                TileType::Solid => {
                    match self.object_builder.built_path.solid_tiles {
                        SolidTiles::Occluders(ref mut occluders) => {
                            occluders.push(Occluder::new(packed_tile.tile_coords));
                        }
                        SolidTiles::Regular(ref mut solid_tiles) => {
                            packed_tile.add_to(solid_tiles,
                                               &mut self.object_builder.built_path.clip_tiles,
                                               &draw_tiling_path_info,
                                               &self.scene_builder);
                        }
                    }
                }
                TileType::SingleMask => {
                    debug_assert_ne!(packed_tile.draw_tile.alpha_tile_id.page(), !0);
                    packed_tile.add_to(&mut self.object_builder.built_path.single_mask_tiles,
                                       &mut self.object_builder.built_path.clip_tiles,
                                       &draw_tiling_path_info,
                                       &self.scene_builder);
                }
                TileType::Empty if blend_mode_is_destructive => {
                    packed_tile.add_to(&mut self.object_builder.built_path.empty_tiles,
                                       &mut self.object_builder.built_path.clip_tiles,
                                       &draw_tiling_path_info,
                                       &self.scene_builder);
                }
                TileType::Empty => {
                    // Just cull.
                }
            }
        }
    }
}

fn process_segment(segment: &Segment,
                   scene_builder: &SceneBuilder,
                   object_builder: &mut ObjectBuilder) {
    // TODO(pcwalton): Stop degree elevating.
    if segment.is_quadratic() {
        let cubic = segment.to_cubic();
        return process_segment(&cubic, scene_builder, object_builder);
    }

    if segment.is_line() ||
            (segment.is_cubic() && segment.as_cubic_segment().is_flat(FLATTENING_TOLERANCE)) {
        return process_line_segment(segment.baseline, scene_builder, object_builder);
    }

    // TODO(pcwalton): Use a smarter flattening algorithm.
    let (prev, next) = segment.split(0.5);
    process_segment(&prev, scene_builder, object_builder);
    process_segment(&next, scene_builder, object_builder);
}

// This is the meat of the technique. It implements the fast lattice-clipping algorithm from
// Nehab and Hoppe, "Random-Access Rendering of General Vector Graphics" 2006.
//
// The algorithm to step through tiles is Amanatides and Woo, "A Fast Voxel Traversal Algorithm for
// Ray Tracing" 1987: http://www.cse.yorku.ca/~amana/research/grid.pdf
fn process_line_segment(line_segment: LineSegment2F,
                        scene_builder: &SceneBuilder,
                        object_builder: &mut ObjectBuilder) {
    let tile_size = vec2f(TILE_WIDTH as f32, TILE_HEIGHT as f32);
    let tile_size_recip = Vector2F::splat(1.0) / tile_size;

    let tile_line_segment =
        (line_segment.0 * tile_size_recip.0.concat_xy_xy(tile_size_recip.0)).floor().to_i32x4();
    let from_tile_coords = Vector2I(tile_line_segment.xy());
    let to_tile_coords = Vector2I(tile_line_segment.zw());

    // Compute `vector_is_negative = vec2i(vector.x < 0 ? -1 : 0, vector.y < 0 ? -1 : 0)`.
    let vector = line_segment.vector();
    let vector_is_negative = vector.0.packed_lt(F32x2::default());

    // Compute `step = vec2f(vector.x < 0 ? -1 : 1, vector.y < 0 ? -1 : 1)`.
    let step = Vector2I((vector_is_negative | U32x2::splat(1)).to_i32x2());

    // Compute `first_tile_crossing = (from_tile_coords + vec2i(vector.x > 0 ? 1 : 0,
    // vector.y > 0 ? 1 : 0)) * tile_size`.
    let first_tile_crossing = (from_tile_coords +
        Vector2I((!vector_is_negative & U32x2::splat(1)).to_i32x2())).to_f32() * tile_size;

    let mut t_max = (first_tile_crossing - line_segment.from()) / vector;
    let t_delta = (tile_size / vector).abs();

    let (mut current_position, mut tile_coords) = (line_segment.from(), from_tile_coords);
    let mut last_step_direction = None;
    let mut iteration = 0;

    loop {
        // Quick check to catch missing the end tile.
        debug_assert!(iteration < MAX_ITERATIONS);

        let next_step_direction = if t_max.x() < t_max.y() {
            StepDirection::X
        } else if t_max.x() > t_max.y() {
            StepDirection::Y
        } else {
            // This should only happen if the line's destination is precisely on a corner point
            // between tiles:
            //
            //     +-----+--O--+
            //     |     | /   |
            //     |     |/    |
            //     +-----O-----+
            //     |     | end |
            //     |     | tile|
            //     +-----+-----+
            //
            // In that case we just need to step in the positive direction to move to the lower
            // right tile.
            if step.x() > 0 { StepDirection::X } else { StepDirection::Y }
        };

        let next_t =
            (if next_step_direction == StepDirection::X { t_max.x() } else { t_max.y() }).min(1.0);

        // If we've reached the end tile, don't step at all.
        let next_step_direction = if tile_coords == to_tile_coords {
            None
        } else {
            Some(next_step_direction)
        };

        let next_position = line_segment.sample(next_t);
        let clipped_line_segment = LineSegment2F::new(current_position, next_position);
        object_builder.add_fill(scene_builder, clipped_line_segment, tile_coords);

        // Add extra fills if necessary.
        if step.y() < 0 && next_step_direction == Some(StepDirection::Y) {
            // Leaves through top boundary.
            let auxiliary_segment = LineSegment2F::new(clipped_line_segment.to(),
                                                       tile_coords.to_f32() * tile_size);
            object_builder.add_fill(scene_builder, auxiliary_segment, tile_coords);
        } else if step.y() > 0 && last_step_direction == Some(StepDirection::Y) {
            // Enters through top boundary.
            let auxiliary_segment = LineSegment2F::new(tile_coords.to_f32() * tile_size,
                                                       clipped_line_segment.from());
            object_builder.add_fill(scene_builder, auxiliary_segment, tile_coords);
        }

        // Adjust backdrop if necessary.
        if step.x() < 0 && last_step_direction == Some(StepDirection::X) {
            // Entered through right boundary.
            object_builder.adjust_alpha_tile_backdrop(tile_coords, 1);
        } else if step.x() > 0 && next_step_direction == Some(StepDirection::X) {
            // Leaving through right boundary.
            object_builder.adjust_alpha_tile_backdrop(tile_coords, -1);
        }

        // Take a step.
        match next_step_direction {
            None => break,
            Some(StepDirection::X) => {
                t_max += vec2f(t_delta.x(), 0.0);
                tile_coords += vec2i(step.x(), 0);
            }
            Some(StepDirection::Y) => {
                t_max += vec2f(0.0, t_delta.y());
                tile_coords += vec2i(0, step.y());
            }
        }

        current_position = next_position;
        last_step_direction = next_step_direction;

        iteration += 1;
    }

    const MAX_ITERATIONS: u32 = 1024;
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum StepDirection {
    X,
    Y,
}
