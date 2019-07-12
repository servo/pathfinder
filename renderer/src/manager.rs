// pathfinder/renderer/src/manager.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Directs the rendering of a scene and manages tile caching policies.

use crate::builder::{SceneBuilder};
use crate::command::{BlockKey, RenderCommand};
use crate::concurrent::executor::Executor;
use crate::scene::Scene;
use crate::tiles::{TILE_HEIGHT, TILE_WIDTH};
use hashbrown::HashSet;
use pathfinder_content::clip::PolygonClipper3D;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::transform3d::{Perspective, Transform4F};
use pathfinder_geometry::vector::{Vector2F, Vector2I, Vector4F};
use std::time::Duration;

#[derive(Clone)]
pub struct SceneManager {
    // FIXME(pcwalton): Should this be public? Changes to it might invalidate
    // cached data…
    pub scene: Scene,

    // Cache
    cached_blocks: HashSet<BlockKey>,
    paints_cached: bool,

    // Options
    cache_policy: CachePolicy,
    render_transform: RenderTransform,
    options: BuildOptions,
}

impl SceneManager {
    #[inline]
    pub fn new() -> SceneManager {
        SceneManager::from_scene(Scene::new())
    }

    pub fn from_scene(scene: Scene) -> SceneManager {
        SceneManager {
            scene,

            cached_blocks: HashSet::new(),
            paints_cached: false,

            cache_policy: CachePolicy::Never,
            render_transform: RenderTransform::Transform2D(Transform2F::default()),
            options: BuildOptions::default(),
        }
    }

    #[inline]
    pub fn set_cache_policy(&mut self, new_cache_policy: CachePolicy) {
        self.cache_policy = new_cache_policy
    }

    #[inline]
    pub fn set_2d_transform(&mut self, new_transform: &Transform2F) {
        self.render_transform = RenderTransform::Transform2D(*new_transform)
    }

    #[inline]
    pub fn set_perspective_transform(&mut self, new_perspective: &Perspective) {
        self.render_transform = RenderTransform::Perspective(*new_perspective)
    }

    #[inline]
    pub fn set_dilation(&mut self, new_dilation: Vector2F) {
        self.options.dilation = new_dilation
    }

    #[inline]
    pub fn set_subpixel_aa_enabled(&mut self, enabled: bool) {
        self.options.subpixel_aa_enabled = enabled
    }

    pub fn build<E>(&mut self, listener: Box<dyn RenderCommandListener>, executor: &E)
                    where E: Executor {
        let prepared_render_transform = self.render_transform.prepare(self.scene.bounds());
        let bounding_quad = prepared_render_transform.bounding_quad();
        let path_count = self.scene.paths.len();
        listener.send(RenderCommand::Start { bounding_quad, path_count });

        // Send paint data.
        if !self.paints_cached {
            listener.send(RenderCommand::AddPaintData(self.scene.build_paint_data()));
            self.paints_cached = true;
        }

        // TODO(pcwalton): Perspective.
        let render_transform = match self.render_transform {
            RenderTransform::Transform2D(render_transform) => render_transform,
            RenderTransform::Perspective(_) => panic!("TODO"),
        };
        println!("render transform={:?}", render_transform);

        // Determine needed blocks.
        let block_keys = self.determine_needed_blocks(&render_transform);

        // Build tiles if applicable.
        let mut total_build_time = Duration::new(0, 0);
        for &block_key in &block_keys {
            let block_transforms = self.compute_block_transforms(block_key, &render_transform);
            if self.cached_blocks.contains(&block_key) {
                continue;
            }

            let bounds = self.scene.bounds();
            let prepared_render_transform =
                RenderTransform::Transform2D(block_transforms.render).prepare(bounds);
            let new_build_time = SceneBuilder::new(&self.scene,
                                                   block_key,
                                                   prepared_render_transform,
                                                   self.cache_policy,
                                                   &self.options,
                                                   &*listener).build(executor);

            total_build_time += new_build_time;

            // TODO(pcwalton): Check cache policy.
            self.cached_blocks.insert(block_key);
        }

        // Composite.
        for &block_key in &block_keys {
            let block_transforms = self.compute_block_transforms(block_key, &render_transform);
            listener.send(RenderCommand::CompositeBlock {
                block: block_key,
                transform: block_transforms.composite,
            });
        }

        // Finish up.
        listener.send(RenderCommand::Finish { build_time: total_build_time });
    }

    fn determine_needed_blocks(&self, current_transform: &Transform2F) -> Vec<BlockKey> {
        let matrix = current_transform.matrix;
        let scale = (f32::max(matrix.m11().abs(), matrix.m12().abs()) +
                     f32::max(matrix.m21().abs(), matrix.m22().abs())) * 0.5;
        let level = 32 - u32::leading_zeros(scale as u32);

        let block_render_transform =
            self.compute_block_render_transform(BlockKey::new(level, Vector2I::default()));

        // Transform view box to scene space.
        let scene_view_box = current_transform.inverse() * self.scene.view_box();

        // Transform scene-space view box to block space.
        let block_view_box = block_render_transform * scene_view_box;

        let block_tile_size = Vector2I::new(256, 256);
        let inv_tile_size = Vector2F::new(1.0 / TILE_WIDTH as f32, 1.0 / TILE_HEIGHT as f32);
        let inv_block_size = inv_tile_size.scale(1.0 / block_tile_size.x() as f32);
        let block_rect = block_view_box.scale_xy(inv_block_size).round_out().to_i32();

        let mut results = vec![];
        for y in i32::max(0, block_rect.min_y())..block_rect.max_y() {
            for x in i32::max(0, block_rect.min_x())..block_rect.max_x() {
                results.push(BlockKey::new(level, Vector2I::new(x, y).scale_xy(block_tile_size)))
            }
        }

        results
    }

    fn compute_block_render_transform(&self, block_key: BlockKey) -> Transform2F {
        let transformed_bounds = self.scene.bounds().scale(block_key.scale() as f32);
        let tile_size = Vector2I::new(-(TILE_WIDTH as i32), -(TILE_HEIGHT as i32));
        let block_render_offset = block_key.tile_origin().scale_xy(tile_size).to_f32() -
            transformed_bounds.origin();
        Transform2F::from_uniform_scale(block_key.scale() as f32).translate(block_render_offset)
    }

    fn compute_block_transforms(&self, block_key: BlockKey, current_transform: &Transform2F)
                                -> BlockTransforms {
        let block_render_transform = self.compute_block_render_transform(block_key);
        println!("block_render_transform={:?}", block_render_transform);
        println!("composite 2D transform={:?}",
                 block_render_transform.inverse() * *current_transform);

        let view_box = self.scene.view_box();
        let scale = Vector4F::new(2.0 / view_box.size().x(), -2.0 / view_box.size().y(), 1.0, 1.0);
        let offset = Vector4F::new(-1.0, 1.0, 0.0, 1.0);
        let to_ndc_transform = Transform4F::from_scale(scale).translate(offset);
        let composite_transform = to_ndc_transform *
            (block_render_transform.inverse() * *current_transform).to_3d();

        let other_composite_transform = to_ndc_transform *
            block_render_transform.inverse().to_3d() * current_transform.to_3d();

        println!("expected composite transform: {:?}", composite_transform);
        println!("... got: {:?}", other_composite_transform);

        BlockTransforms {
            render: block_render_transform,
            composite: composite_transform,
        }
    }
}

/// How tiles are cached from frame to frame.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CachePolicy {
    /// No caching is performed.
    Never,
    /// The full scene is prerendered to tiles without regard for view box.
    /// Tiles are cached from frame to frame when the translation changes.
    /// If scale, skew, or rotation change, then we tile again.
    OnTranslation,
    /// The full scene is prerendered to tiles without regard for view box.
    /// Tiles are cached from frame to frame as long as the scale does not
    /// change by more than 2x. If the change falls out of tolerance, then we
    /// tile again.
    Mipmap,
}

#[derive(Clone)]
enum RenderTransform {
    Transform2D(Transform2F),
    Perspective(Perspective),
}

impl Default for RenderTransform {
    #[inline]
    fn default() -> RenderTransform {
        RenderTransform::Transform2D(Transform2F::default())
    }
}

impl RenderTransform {
    fn prepare(&self, bounds: RectF) -> PreparedRenderTransform {
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
        debug!("-----");
        debug!("bounds={:?} ORIGINAL quad={:?}", bounds, points);
        for point in &mut points {
            *point = perspective.transform * *point;
        }
        debug!("... PERSPECTIVE quad={:?}", points);

        // Compute depth.
        let quad = [
            points[0].perspective_divide(),
            points[1].perspective_divide(),
            points[2].perspective_divide(),
            points[3].perspective_divide(),
        ];
        debug!("... PERSPECTIVE-DIVIDED points = {:?}", quad);

        points = PolygonClipper3D::new(points).clip();
        debug!("... CLIPPED quad={:?}", points);
        for point in &mut points {
            *point = point.perspective_divide()
        }

        let inverse_transform = perspective.transform.inverse();
        let clip_polygon = points
            .into_iter()
            .map(|point| (inverse_transform * point).perspective_divide().to_2d())
            .collect();
        return PreparedRenderTransform::Perspective {
            perspective,
            clip_polygon,
            quad,
        };
    }
}

pub trait RenderCommandListener: Send + Sync {
    fn send(&self, command: RenderCommand);
}

impl<F> RenderCommandListener for F
where
    F: Fn(RenderCommand) + Send + Sync,
{
    #[inline]
    fn send(&self, command: RenderCommand) {
        (*self)(command)
    }
}

#[derive(Copy, Clone, Default)]
pub(crate) struct BuildOptions {
    pub(crate) dilation: Vector2F,
    pub(crate) subpixel_aa_enabled: bool,
}

pub(crate) type BoundingQuad = [Vector4F; 4];

pub(crate) enum PreparedRenderTransform {
    None,
    Transform2D(Transform2F),
    Perspective {
        perspective: Perspective,
        clip_polygon: Vec<Vector2F>,
        quad: [Vector4F; 4],
    },
}

impl PreparedRenderTransform {
    #[inline]
    pub(crate) fn bounding_quad(&self) -> BoundingQuad {
        match *self {
            PreparedRenderTransform::Perspective { quad, .. } => quad,
            _ => [Vector4F::default(); 4],
        }
    }

    #[inline]
    pub(crate) fn is_2d(&self) -> bool {
        match *self {
            PreparedRenderTransform::Transform2D(_) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
struct BlockTransforms {
    render: Transform2F,
    composite: Transform4F,
}
