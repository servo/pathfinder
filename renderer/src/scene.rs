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
use crate::options::{BuildOptions, PreparedBuildOptions};
use crate::options::{PreparedRenderTransform, RenderCommandListener};
use crate::paint::{Paint, PaintId, PaintInfo, Palette};
use pathfinder_content::effects::{BlendMode, Effects};
use pathfinder_content::fill::FillRule;
use pathfinder_content::outline::Outline;
use pathfinder_content::render_target::RenderTargetId;
use pathfinder_geometry::vector::{Vector2F, Vector2I};
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::transform2d::Transform2F;

#[derive(Clone)]
pub struct Scene {
    pub(crate) display_list: Vec<DisplayItem>,
    pub(crate) paths: Vec<DrawPath>,
    pub(crate) clip_paths: Vec<ClipPath>,
    palette: Palette,
    bounds: RectF,
    view_box: RectF,
}

impl Scene {
    #[inline]
    pub fn new() -> Scene {
        Scene {
            display_list: vec![],
            paths: vec![],
            clip_paths: vec![],
            palette: Palette::new(),
            bounds: RectF::default(),
            view_box: RectF::default(),
        }
    }

    pub fn push_path(&mut self, path: DrawPath) {
        self.bounds = self.bounds.union_rect(path.outline.bounds());
        self.paths.push(path);

        let new_path_count = self.paths.len() as u32;
        if let Some(DisplayItem::DrawPaths {
            start_index: _,
            ref mut end_index
        }) = self.display_list.last_mut() {
            *end_index = new_path_count;
        } else {
            self.display_list.push(DisplayItem::DrawPaths {
                start_index: new_path_count - 1,
                end_index: new_path_count,
            });
        }
    }

    pub fn push_clip_path(&mut self, clip_path: ClipPath) -> ClipPathId {
        self.bounds = self.bounds.union_rect(clip_path.outline.bounds());
        let clip_path_id = ClipPathId(self.clip_paths.len() as u32);
        self.clip_paths.push(clip_path);
        clip_path_id
    }

    pub fn push_render_target(&mut self, render_target: RenderTarget) -> RenderTargetId {
        let render_target_id = self.palette.push_render_target(render_target);
        self.display_list.push(DisplayItem::PushRenderTarget(render_target_id));
        render_target_id
    }

    pub fn pop_render_target(&mut self) {
        self.display_list.push(DisplayItem::PopRenderTarget);
    }

    pub fn draw_render_target(&mut self, render_target: RenderTargetId, effects: Effects) {
        self.display_list.push(DisplayItem::DrawRenderTarget { render_target, effects });
    }

    #[inline]
    pub fn build_paint_info(&self) -> PaintInfo {
        self.palette.build_paint_info(self.view_box.size().to_i32())
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn push_paint(&mut self, paint: &Paint) -> PaintId {
        self.palette.push_paint(paint)
    }

    #[inline]
    pub fn path_count(&self) -> usize {
        self.paths.len()
    }

    #[inline]
    pub fn bounds(&self) -> RectF {
        self.bounds
    }

    #[inline]
    pub fn set_bounds(&mut self, new_bounds: RectF) {
        self.bounds = new_bounds;
    }

    #[inline]
    pub fn view_box(&self) -> RectF {
        self.view_box
    }

    #[inline]
    pub fn set_view_box(&mut self, new_view_box: RectF) {
        self.view_box = new_view_box;
    }

    pub(crate) fn apply_render_options(
        &self,
        original_outline: &Outline,
        options: &PreparedBuildOptions,
    ) -> Outline {
        let effective_view_box = self.effective_view_box(options);

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
                    outline.clip_against_polygon(clip_polygon);
                    outline.apply_perspective(perspective);

                    // TODO(pcwalton): Support subpixel AA in 3D.
                }
            }
            _ => {
                // TODO(pcwalton): Short circuit.
                outline = (*original_outline).clone();
                if options.transform.is_2d() || options.subpixel_aa_enabled {
                    let mut transform = match options.transform {
                        PreparedRenderTransform::Transform2D(transform) => transform,
                        PreparedRenderTransform::None => Transform2F::default(),
                        PreparedRenderTransform::Perspective { .. } => unreachable!(),
                    };
                    if options.subpixel_aa_enabled {
                        transform *= Transform2F::from_scale(Vector2F::new(3.0, 1.0))
                    }
                    outline.transform(&transform);
                }
                outline.clip_against_rect(effective_view_box);
            }
        }

        if !options.dilation.is_zero() {
            outline.dilate(options.dilation);
        }

        // TODO(pcwalton): Fold this into previous passes to avoid unnecessary clones during
        // monotonic conversion.
        outline.prepare_for_tiling(self.effective_view_box(options));
        outline
    }

    #[inline]
    pub(crate) fn effective_view_box(&self, render_options: &PreparedBuildOptions) -> RectF {
        if render_options.subpixel_aa_enabled {
            self.view_box.scale_xy(Vector2F::new(3.0, 1.0))
        } else {
            self.view_box
        }
    }

    #[inline]
    pub fn build<E, L: RenderCommandListener>(&self,
                    options: BuildOptions,
                    listener: L,
                    executor: &E)
                    where E: Executor {
        let prepared_options = options.prepare(self.bounds);
        SceneBuilder::new(self, &prepared_options, listener).build(executor)
    }

    pub fn paths<'a>(&'a self) -> PathIter {
        PathIter {
            scene: self,
            pos: 0
        }
    }
}

pub struct PathIter<'a> {
    scene: &'a Scene,
    pos: usize
}

impl<'a> Iterator for PathIter<'a> {
    type Item = (&'a Paint, &'a Outline, &'a str);
    fn next(&mut self) -> Option<Self::Item> {
        let item = self.scene.paths.get(self.pos).map(|path_object| {
            (
                self.scene.palette.paints.get(path_object.paint.0 as usize).unwrap(),
                &path_object.outline,
                &*path_object.name
            )
        });
        self.pos += 1;
        item
    }
}

#[derive(Clone, Debug)]
pub struct DrawPath {
    outline: Outline,
    paint: PaintId,
    clip_path: Option<ClipPathId>,
    fill_rule: FillRule,
    blend_mode: BlendMode,
    opacity: u8,
    name: String,
}

#[derive(Clone, Debug)]
pub struct ClipPath {
    outline: Outline,
    fill_rule: FillRule,
    name: String,
}

#[derive(Clone, Copy, Debug)]
pub struct ClipPathId(pub u32);

#[derive(Clone, Debug)]
pub struct RenderTarget {
    size: Vector2I,
    name: String,
}

/// Drawing commands.
#[derive(Clone, Debug)]
pub enum DisplayItem {
    /// Draws paths to the render target on top of the stack.
    DrawPaths { start_index: u32, end_index: u32 },

    /// Draws an entire render target to the render target on top of the stack.
    ///
    /// FIXME(pcwalton): This draws the entire render target, so it's inefficient. We should get
    /// rid of this command and transition all uses to `DrawPaths`. The reason it exists is that we
    /// don't have logic to create tiles for blur bounding regions yet.
    DrawRenderTarget { render_target: RenderTargetId, effects: Effects },

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
            opacity: !0,
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
    pub(crate) fn opacity(&self) -> u8 {
        self.opacity
    }

    #[inline]
    pub fn set_opacity(&mut self, new_opacity: u8) {
        self.opacity = new_opacity
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
