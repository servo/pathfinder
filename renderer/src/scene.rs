// pathfinder/renderer/src/scene.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A set of paths to be rendered.

use crate::builder::SceneBuilder;
use crate::concurrent::executor::Executor;
use crate::gpu::options::RendererLevel;
use crate::options::{BuildOptions, PreparedBuildOptions};
use crate::options::{PreparedRenderTransform, RenderCommandListener};
use crate::paint::{MergedPaletteInfo, Paint, PaintId, PaintInfo, Palette};
use pathfinder_content::effects::BlendMode;
use pathfinder_content::fill::FillRule;
use pathfinder_content::outline::Outline;
use pathfinder_content::render_target::RenderTargetId;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::{Vector2I, vec2f};
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::u64;

static NEXT_SCENE_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
pub struct Scene {
    display_list: Vec<DisplayItem>,
    draw_paths: Vec<DrawPath>,
    clip_paths: Vec<ClipPath>,
    palette: Palette,
    bounds: RectF,
    view_box: RectF,
    id: SceneId,
    epoch: SceneEpoch,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneId(pub u32);

impl Scene {
    #[inline]
    pub fn new() -> Scene {
        let scene_id = SceneId(NEXT_SCENE_ID.fetch_add(1, Ordering::Relaxed) as u32);
        Scene {
            display_list: vec![],
            draw_paths: vec![],
            clip_paths: vec![],
            palette: Palette::new(scene_id),
            bounds: RectF::default(),
            view_box: RectF::default(),
            id: scene_id,
            epoch: SceneEpoch::new(0, 1),
        }
    }

    pub fn push_draw_path(&mut self, draw_path: DrawPath) {
        let draw_path_index = DrawPathId(self.draw_paths.len() as u32);
        self.draw_paths.push(draw_path);
        self.push_draw_path_with_index(draw_path_index);
    }

    fn push_draw_path_with_index(&mut self, draw_path_id: DrawPathId) {
        let new_path_bounds = self.draw_paths[draw_path_id.0 as usize].outline.bounds();
        self.bounds = self.bounds.union_rect(new_path_bounds);

        let end_path_id = DrawPathId(draw_path_id.0 + 1);
        match self.display_list.last_mut() {
            Some(DisplayItem::DrawPaths(ref mut range)) => range.end = end_path_id,
            _ => self.display_list.push(DisplayItem::DrawPaths(draw_path_id..end_path_id)),
        }

        self.epoch.next();
    }

    pub fn push_clip_path(&mut self, clip_path: ClipPath) -> ClipPathId {
        self.bounds = self.bounds.union_rect(clip_path.outline.bounds());
        let clip_path_id = ClipPathId(self.clip_paths.len() as u32);
        self.clip_paths.push(clip_path);
        self.epoch.next();
        clip_path_id
    }

    pub fn push_render_target(&mut self, render_target: RenderTarget) -> RenderTargetId {
        let render_target_id = self.palette.push_render_target(render_target);
        self.display_list.push(DisplayItem::PushRenderTarget(render_target_id));
        self.epoch.next();
        render_target_id
    }

    pub fn pop_render_target(&mut self) {
        self.display_list.push(DisplayItem::PopRenderTarget);
    }

    pub fn append_scene(&mut self, scene: Scene) {
        let MergedPaletteInfo {
            render_target_mapping,
            paint_mapping,
        } = self.palette.append_palette(scene.palette);

        // Merge clip paths.
        let mut clip_path_mapping = Vec::with_capacity(scene.clip_paths.len());
        for clip_path in scene.clip_paths {
            clip_path_mapping.push(self.clip_paths.len());
            self.clip_paths.push(clip_path);
        }

        // Merge draw paths.
        let mut draw_path_mapping = Vec::with_capacity(scene.draw_paths.len());
        for draw_path in scene.draw_paths {
            draw_path_mapping.push(self.draw_paths.len() as u32);
            self.draw_paths.push(DrawPath {
                outline: draw_path.outline,
                paint: paint_mapping[&draw_path.paint],
                clip_path: draw_path.clip_path.map(|clip_path_id| {
                    ClipPathId(clip_path_mapping[clip_path_id.0 as usize] as u32)
                }),
                fill_rule: draw_path.fill_rule,
                blend_mode: draw_path.blend_mode,
                name: draw_path.name,
            });
        }

        // Merge display items.
        for display_item in scene.display_list {
            match display_item {
                DisplayItem::PushRenderTarget(old_render_target_id) => {
                    let new_render_target_id = render_target_mapping[&old_render_target_id];
                    self.display_list.push(DisplayItem::PushRenderTarget(new_render_target_id));
                }
                DisplayItem::PopRenderTarget => {
                    self.display_list.push(DisplayItem::PopRenderTarget);
                }
                DisplayItem::DrawPaths(range) => {
                    for old_path_index in (range.start.0 as usize)..(range.end.0 as usize) {
                        let old_draw_path_id = DrawPathId(draw_path_mapping[old_path_index]);
                        self.push_draw_path_with_index(old_draw_path_id);
                    }
                }
            }
        }

        // Bump epoch.
        self.epoch.next();
    }

    #[inline]
    pub fn build_paint_info(&mut self, render_transform: Transform2F) -> PaintInfo {
        self.palette.build_paint_info(render_transform)
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn push_paint(&mut self, paint: &Paint) -> PaintId {
        let paint_id = self.palette.push_paint(paint);
        self.epoch.next();
        paint_id
    }

    #[inline]
    pub fn bounds(&self) -> RectF {
        self.bounds
    }

    #[inline]
    pub fn set_bounds(&mut self, new_bounds: RectF) {
        self.bounds = new_bounds;
        self.epoch.next();
    }

    #[inline]
    pub fn view_box(&self) -> RectF {
        self.view_box
    }

    #[inline]
    pub fn set_view_box(&mut self, new_view_box: RectF) {
        self.view_box = new_view_box;
        self.epoch.next();
    }

    pub(crate) fn apply_render_options(&self,
                                       original_outline: &Outline,
                                       options: &PreparedBuildOptions)
                                       -> Outline {
        let mut outline;
        match options.transform {
            PreparedRenderTransform::Perspective {
                ref perspective,
                ref clip_polygon,
                ..
            } => {
                if original_outline.is_outside_polygon(clip_polygon) {
                    outline = Outline::new();
                } else {
                    outline = (*original_outline).clone();
                    outline.close_all_contours();
                    outline.clip_against_polygon(clip_polygon);
                    outline.apply_perspective(perspective);

                    // TODO(pcwalton): Support subpixel AA in 3D.
                }
            }
            _ => {
                // TODO(pcwalton): Short circuit.
                outline = (*original_outline).clone();
                outline.close_all_contours();
                if options.transform.is_2d() || options.subpixel_aa_enabled {
                    let mut transform = match options.transform {
                        PreparedRenderTransform::Transform2D(transform) => transform,
                        PreparedRenderTransform::None => Transform2F::default(),
                        PreparedRenderTransform::Perspective { .. } => unreachable!(),
                    };
                    if options.subpixel_aa_enabled {
                        transform *= Transform2F::from_scale(vec2f(3.0, 1.0))
                    }
                    outline.transform(&transform);
                }
            }
        }

        if !options.dilation.is_zero() {
            outline.dilate(options.dilation);
        }

        outline
    }

    #[inline]
    pub(crate) fn effective_view_box(&self, render_options: &PreparedBuildOptions) -> RectF {
        if render_options.subpixel_aa_enabled {
            self.view_box * vec2f(3.0, 1.0)
        } else {
            self.view_box
        }
    }

    #[inline]
    pub fn build<'a, 'b, E>(&mut self,
                            options: BuildOptions,
                            sink: &'b mut SceneSink<'a>,
                            executor: &E)
                            where E: Executor {
        let prepared_options = options.prepare(self.bounds);
        SceneBuilder::new(self, &prepared_options, sink).build(executor)
    }

    #[inline]
    pub fn display_list(&self) -> &[DisplayItem] {
        &self.display_list
    }

    #[inline]
    pub fn draw_paths(&self) -> &[DrawPath] {
        &self.draw_paths
    }

    #[inline]
    pub fn clip_paths(&self) -> &[ClipPath] {
        &self.clip_paths
    }

    #[inline]
    pub fn get_draw_path(&self, draw_path_id: DrawPathId) -> &DrawPath {
        &self.draw_paths[draw_path_id.0 as usize]
    }

    #[inline]
    pub fn get_clip_path(&self, clip_path_id: ClipPathId) -> &ClipPath {
        &self.clip_paths[clip_path_id.0 as usize]
    }

    #[inline]
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    #[inline]
    pub fn id(&self) -> SceneId {
        self.id
    }

    #[inline]
    pub fn epoch(&self) -> SceneEpoch {
        self.epoch
    }
}

pub struct SceneSink<'a> {
    pub(crate) listener: RenderCommandListener<'a>,
    pub(crate) renderer_level: RendererLevel,
    pub(crate) last_scene: Option<LastSceneInfo>,
}

pub(crate) struct LastSceneInfo {
    pub(crate) scene_id: SceneId,
    pub(crate) scene_epoch: SceneEpoch,
    pub(crate) draw_segment_ranges: Vec<Range<u32>>,
    pub(crate) clip_segment_ranges: Vec<Range<u32>>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SceneEpoch {
    pub hi: u64,
    pub lo: u64,
}

impl SceneEpoch {
    #[inline]
    fn new(hi: u64, lo: u64) -> SceneEpoch {
        SceneEpoch { hi, lo }
    }

    #[inline]
    fn successor(&self) -> SceneEpoch {
        if self.lo == u64::MAX {
            SceneEpoch { hi: self.hi + 1, lo: 0 }
        } else {
            SceneEpoch { hi: self.hi, lo: self.lo + 1 }
        }
    }

    #[inline]
    fn next(&mut self) {
        *self = self.successor();
    }
}

impl<'a> SceneSink<'a> {
    #[inline]
    pub fn new(listener: RenderCommandListener<'a>, renderer_level: RendererLevel)
               -> SceneSink<'a> {
        SceneSink { listener, renderer_level, last_scene: None }
    }
}

#[derive(Clone, Debug)]
pub struct DrawPath {
    pub outline: Outline,
    pub paint: PaintId,
    pub clip_path: Option<ClipPathId>,
    pub fill_rule: FillRule,
    pub blend_mode: BlendMode,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct ClipPath {
    pub outline: Outline,
    pub fill_rule: FillRule,
    pub name: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DrawPathId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ClipPathId(pub u32);

/// Either a draw path ID or a clip path ID, depending on context.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PathId(pub u32);

#[derive(Clone, Debug)]
pub struct RenderTarget {
    size: Vector2I,
    name: String,
}

/// Drawing commands.
#[derive(Clone, Debug)]
pub enum DisplayItem {
    /// Draws paths to the render target on top of the stack.
    DrawPaths(Range<DrawPathId>),

    /// Pushes a render target onto the top of the stack.
    PushRenderTarget(RenderTargetId),

    /// Pops a render target from the stack.
    PopRenderTarget,
}

impl DrawPath {
    #[inline]
    pub fn new(outline: Outline, paint: PaintId) -> DrawPath {
        DrawPath {
            outline,
            paint,
            clip_path: None,
            fill_rule: FillRule::Winding,
            blend_mode: BlendMode::SrcOver,
            name: String::new(),
        }
    }

    #[inline]
    pub fn outline(&self) -> &Outline {
        &self.outline
    }

    #[inline]
    pub(crate) fn clip_path(&self) -> Option<ClipPathId> {
        self.clip_path
    }

    #[inline]
    pub fn set_clip_path(&mut self, new_clip_path: Option<ClipPathId>) {
        self.clip_path = new_clip_path
    }

    #[inline]
    pub(crate) fn paint(&self) -> PaintId {
        self.paint
    }

    #[inline]
    pub(crate) fn fill_rule(&self) -> FillRule {
        self.fill_rule
    }

    #[inline]
    pub fn set_fill_rule(&mut self, new_fill_rule: FillRule) {
        self.fill_rule = new_fill_rule
    }

    #[inline]
    pub(crate) fn blend_mode(&self) -> BlendMode {
        self.blend_mode
    }

    #[inline]
    pub fn set_blend_mode(&mut self, new_blend_mode: BlendMode) {
        self.blend_mode = new_blend_mode
    }

    #[inline]
    pub fn set_name(&mut self, new_name: String) {
        self.name = new_name
    }
}

impl ClipPath {
    #[inline]
    pub fn new(outline: Outline) -> ClipPath {
        ClipPath { outline, fill_rule: FillRule::Winding, name: String::new() }
    }

    #[inline]
    pub fn outline(&self) -> &Outline {
        &self.outline
    }

    #[inline]
    pub(crate) fn fill_rule(&self) -> FillRule {
        self.fill_rule
    }

    #[inline]
    pub fn set_fill_rule(&mut self, new_fill_rule: FillRule) {
        self.fill_rule = new_fill_rule
    }

    #[inline]
    pub fn set_name(&mut self, new_name: String) {
        self.name = new_name
    }
}

impl RenderTarget {
    #[inline]
    pub fn new(size: Vector2I, name: String) -> RenderTarget {
        RenderTarget { size, name }
    }

    #[inline]
    pub fn size(&self) -> Vector2I {
        self.size
    }
}

impl DrawPathId {
    #[inline]
    pub(crate) fn to_path_id(self) -> PathId {
        PathId(self.0)
    }
}

impl ClipPathId {
    #[inline]
    pub(crate) fn to_path_id(self) -> PathId {
        PathId(self.0)
    }
}

impl PathId {
    #[inline]
    pub(crate) fn to_clip_path_id(self) -> ClipPathId {
        ClipPathId(self.0)
    }

    #[inline]
    pub(crate) fn to_draw_path_id(self) -> DrawPathId {
        DrawPathId(self.0)
    }
}
