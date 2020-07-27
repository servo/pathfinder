// pathfinder/renderer/src/paint.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Defines how a path is to be filled.

use crate::allocator::{AllocationMode, TextureAllocator};
use crate::gpu_data::{ColorCombineMode, RenderCommand, TextureLocation, TextureMetadataEntry};
use crate::gpu_data::{TexturePageDescriptor, TexturePageId, TileBatchTexture};
use crate::scene::{RenderTarget, SceneId};
use hashbrown::{HashMap, HashSet};
use pathfinder_color::ColorU;
use pathfinder_content::effects::{BlendMode, Filter, PatternFilter};
use pathfinder_content::gradient::{Gradient, GradientGeometry, GradientWrap};
use pathfinder_content::pattern::{ImageHash, Pattern, PatternSource};
use pathfinder_content::render_target::RenderTargetId;
use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_geometry::rect::{RectF, RectI};
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::{Vector2F, Vector2I, vec2f, vec2i};
use pathfinder_gpu::TextureSamplingFlags;
use pathfinder_simd::default::{F32x2, F32x4};
use std::f32;
use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;

// The size of a gradient tile.
//
// TODO(pcwalton): Choose this size dynamically!
const GRADIENT_TILE_LENGTH: u32 = 256;

// Stores all paints in a scene.
#[derive(Clone)]
pub(crate) struct Palette {
    pub(crate) paints: Vec<Paint>,
    render_targets: Vec<RenderTarget>,
    cache: HashMap<Paint, PaintId>,
    scene_id: SceneId,
}

// Caches texture images from scene to scene.
pub(crate) struct PaintTextureManager {
    allocator: TextureAllocator,
    cached_images: HashMap<ImageHash, TextureLocation>,
}

/// Defines how a path is to be filled: with a solid color, gradient, or pattern.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Paint {
    base_color: ColorU,
    overlay: Option<PaintOverlay>,
}

/// What is to be overlaid on top of a base color.
///
/// An overlay is a gradient or a pattern, plus a composite operation which determines how the
/// gradient or pattern is to be combined with the base color.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct PaintOverlay {
    composite_op: PaintCompositeOp,
    contents: PaintContents,
}

/// The contents of an overlay: either a gradient or a pattern.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum PaintContents {
    /// A gradient, either linear or radial.
    Gradient(Gradient),
    /// A raster image pattern.
    Pattern(Pattern),
}

/// The ID of a paint, unique to a scene.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PaintId(pub u16);

/// The ID of a gradient, unique to a scene.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GradientId(pub u32);

/// How an overlay is to be composited over a base color.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PaintCompositeOp {
    /// The source that overlaps the destination, replaces the destination.
    SrcIn,
    /// Destination which overlaps the source, replaces the source.
    DestIn,
}

impl Debug for PaintContents {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match *self {
            PaintContents::Gradient(ref gradient) => gradient.fmt(formatter),
            PaintContents::Pattern(ref pattern) => pattern.fmt(formatter),
        }
    }
}

impl Palette {
    #[inline]
    pub(crate) fn new(scene_id: SceneId) -> Palette {
        Palette {
            paints: vec![],
            render_targets: vec![],
            cache: HashMap::new(),
            scene_id,
        }
    }
}

impl Paint {
    /// Creates a simple paint from a single base color.
    #[inline]
    pub fn from_color(color: ColorU) -> Paint {
        Paint { base_color: color, overlay: None }
    }

    /// Creates a paint from a gradient.
    #[inline]
    pub fn from_gradient(gradient: Gradient) -> Paint {
        Paint {
            base_color: ColorU::white(),
            overlay: Some(PaintOverlay {
                composite_op: PaintCompositeOp::SrcIn,
                contents: PaintContents::Gradient(gradient),
            }),
        }
    }

    /// Creates a paint from a raster pattern.
    #[inline]
    pub fn from_pattern(pattern: Pattern) -> Paint {
        Paint {
            base_color: ColorU::white(),
            overlay: Some(PaintOverlay {
                composite_op: PaintCompositeOp::SrcIn,
                contents: PaintContents::Pattern(pattern),
            }),
        }
    }

    /// A convenience function to create a solid black paint.
    #[inline]
    pub fn black() -> Paint {
        Paint::from_color(ColorU::black())
    }

    /// A convenience function to create a transparent paint with all channels set to zero.
    #[inline]
    pub fn transparent_black() -> Paint {
        Paint::from_color(ColorU::transparent_black())
    }

    /// Returns true if this paint is obviously opaque, via a quick check.
    ///
    /// Even if the paint is opaque, this function might return false.
    pub fn is_opaque(&self) -> bool {
        if !self.base_color.is_opaque() {
            return false;
        }

        match self.overlay {
            None => true,
            Some(ref overlay) => {
                match overlay.contents {
                    PaintContents::Gradient(ref gradient) => gradient.is_opaque(),
                    PaintContents::Pattern(ref pattern) => pattern.is_opaque(),
                }
            }
        }
    }

    /// Returns true if this paint is fully transparent, via a quick check.
    ///
    /// Even if the paint is fully transparent, this function might return false.
    pub fn is_fully_transparent(&self) -> bool {
        if !self.base_color.is_fully_transparent() {
            return false;
        }

        match self.overlay {
            None => true,
            Some(ref overlay) => {
                match overlay.contents {
                    PaintContents::Gradient(ref gradient) => gradient.is_fully_transparent(),
                    PaintContents::Pattern(_) => false,
                }
            }
        }
    }

    /// Returns true if this paint represents a solid color.
    #[inline]
    pub fn is_color(&self) -> bool {
        self.overlay.is_none()
    }

    /// Applies an affine transform to this paint.
    ///
    /// This has no effect if this paint is a solid color.
    pub fn apply_transform(&mut self, transform: &Transform2F) {
        if transform.is_identity() {
            return;
        }

        if let Some(ref mut overlay) = self.overlay {
            match overlay.contents {
                PaintContents::Gradient(ref mut gradient) => gradient.apply_transform(*transform),
                PaintContents::Pattern(ref mut pattern) => pattern.apply_transform(*transform),
            }
        }
    }

    /// Returns the *base color* of this paint.
    ///
    /// The base color is the color that goes underneath the gradient or pattern, if there is one.
    #[inline]
    pub fn base_color(&self) -> ColorU {
        self.base_color
    }

    /// Changes the *base color* of this paint.
    ///
    /// The base color is the color that goes underneath the gradient or pattern, if there is one.
    #[inline]
    pub fn set_base_color(&mut self, new_base_color: ColorU) {
        self.base_color = new_base_color;
    }

    /// Returns the paint overlay, which is the portion of the paint on top of the base color.
    #[inline]
    pub fn overlay(&self) -> &Option<PaintOverlay> {
        &self.overlay
    }

    /// Returns a mutable reference to the paint overlay, which is the portion of the paint on top
    /// of the base color.
    #[inline]
    pub fn overlay_mut(&mut self) -> &mut Option<PaintOverlay> {
        &mut self.overlay
    }

    /// Returns the pattern, if this paint represents one.
    #[inline]
    pub fn pattern(&self) -> Option<&Pattern> {
        match self.overlay {
            None => None,
            Some(ref overlay) => {
                match overlay.contents {
                    PaintContents::Pattern(ref pattern) => Some(pattern),
                    _ => None,
                }
            }
        }
    }

    /// Returns a mutable reference to the pattern, if this paint represents one.
    #[inline]
    pub fn pattern_mut(&mut self) -> Option<&mut Pattern> {
        match self.overlay {
            None => None,
            Some(ref mut overlay) => {
                match overlay.contents {
                    PaintContents::Pattern(ref mut pattern) => Some(pattern),
                    _ => None,
                }
            }
        }
    }

    /// Returns the gradient, if this paint represents one.
    #[inline]
    pub fn gradient(&self) -> Option<&Gradient> {
        match self.overlay {
            None => None,
            Some(ref overlay) => {
                match overlay.contents {
                    PaintContents::Gradient(ref gradient) => Some(gradient),
                    _ => None,
                }
            }
        }
    }
}

impl PaintOverlay {
    #[inline]
    pub(crate) fn contents(&self) -> &PaintContents {
        &self.contents
    }

    /// Returns the composite operation, which defines how the overlay is to be composited on top
    /// of the base color.
    #[inline]
    pub fn composite_op(&self) -> PaintCompositeOp {
        self.composite_op
    }

    /// Changes the composite operation, which defines how the overlay is to be composited on top
    /// of the base color.
    #[inline]
    pub fn set_composite_op(&mut self, new_composite_op: PaintCompositeOp) {
        self.composite_op = new_composite_op;
    }
}

pub(crate) struct PaintInfo {
    /// The render commands needed to prepare the textures.
    pub(crate) render_commands: Vec<RenderCommand>,
    /// The metadata for each paint.
    ///
    /// The indices of this vector are paint IDs.
    pub(crate) paint_metadata: Vec<PaintMetadata>,
}

#[derive(Debug)]
pub(crate) struct PaintMetadata {
    /// Metadata associated with the color texture, if applicable.
    pub(crate) color_texture_metadata: Option<PaintColorTextureMetadata>,
    /// The base color that the color texture gets mixed into.
    pub(crate) base_color: ColorU,
    pub(crate) blend_mode: BlendMode,
    /// True if this paint is fully opaque.
    pub(crate) is_opaque: bool,
}

#[derive(Debug)]
pub(crate) struct PaintColorTextureMetadata {
    /// The location of the paint.
    pub(crate) location: TextureLocation,
    /// The scale for the page this paint is on.
    pub(crate) page_scale: Vector2F,
    /// The transform to apply to screen coordinates to translate them into UVs.
    pub(crate) transform: Transform2F,
    /// The sampling mode for the texture.
    pub(crate) sampling_flags: TextureSamplingFlags,
    /// The filter to be applied to this paint.
    pub(crate) filter: PaintFilter,
    /// How the color texture is to be composited over the base color.
    pub(crate) composite_op: PaintCompositeOp,
    /// How much of a border there needs to be around the image.
    ///
    /// The border ensures clamp-to-edge yields the right result.
    pub(crate) border: Vector2I,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RadialGradientMetadata {
    /// The line segment that connects the two circles.
    pub(crate) line: LineSegment2F,
    /// The radii of the two circles.
    pub(crate) radii: F32x2,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RenderTargetMetadata {
    /// The location of the render target.
    pub(crate) location: TextureLocation,
}

#[derive(Debug)]
pub(crate) enum PaintFilter {
    None,
    RadialGradient {
        /// The line segment that connects the two circles.
        line: LineSegment2F,
        /// The radii of the two circles.
        radii: F32x2,
    },
    PatternFilter(PatternFilter),
}

impl Palette {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub(crate) fn push_paint(&mut self, paint: &Paint) -> PaintId {
        if let Some(paint_id) = self.cache.get(paint) {
            return *paint_id;
        }

        let paint_id = PaintId(self.paints.len() as u16);
        self.cache.insert((*paint).clone(), paint_id);
        self.paints.push((*paint).clone());
        paint_id
    }

    pub(crate) fn push_render_target(&mut self, render_target: RenderTarget) -> RenderTargetId {
        let id = self.render_targets.len() as u32;
        self.render_targets.push(render_target);
        RenderTargetId { scene: self.scene_id.0, render_target: id }
    }

    pub(crate) fn build_paint_info(&mut self,
                                   texture_manager: &mut PaintTextureManager,
                                   render_transform: Transform2F)
                                   -> PaintInfo {
        // Assign render target locations.
        let mut transient_paint_locations = vec![];
        let render_target_metadata =
            self.assign_render_target_locations(texture_manager, &mut transient_paint_locations);

        // Assign paint locations.
        let PaintLocationsInfo {
            mut paint_metadata,
            gradient_tile_builder,
            image_texel_info,
            used_image_hashes,
        } = self.assign_paint_locations(&render_target_metadata,
                                        texture_manager,
                                        &mut transient_paint_locations);

        // Calculate texture transforms.
        self.calculate_texture_transforms(&mut paint_metadata, texture_manager, render_transform);

        // Create texture metadata.
        let texture_metadata = self.create_texture_metadata(&paint_metadata);
        let mut render_commands = vec![RenderCommand::UploadTextureMetadata(texture_metadata)];

        // Allocate textures.
        self.allocate_textures(&mut render_commands, texture_manager);

        // Create render commands.
        self.create_render_commands(&mut render_commands,
                                    render_target_metadata,
                                    gradient_tile_builder,
                                    image_texel_info);

        // Free transient locations and unused images, now that they're no longer needed.
        self.free_transient_locations(texture_manager, transient_paint_locations);
        self.free_unused_images(texture_manager, used_image_hashes);

        PaintInfo { render_commands, paint_metadata }
    }

    fn assign_render_target_locations(&self,
                                      texture_manager: &mut PaintTextureManager,
                                      transient_paint_locations: &mut Vec<TextureLocation>)
                                      -> Vec<RenderTargetMetadata> {
        let mut render_target_metadata = vec![];
        for render_target in &self.render_targets {
            let location = texture_manager.allocator.allocate_image(render_target.size());
            render_target_metadata.push(RenderTargetMetadata { location });
            transient_paint_locations.push(location);
        }
        render_target_metadata
    }

    fn assign_paint_locations(&self,
                              render_target_metadata: &[RenderTargetMetadata],
                              texture_manager: &mut PaintTextureManager,
                              transient_paint_locations: &mut Vec<TextureLocation>)
                              -> PaintLocationsInfo {
        let mut paint_metadata = vec![];
        let mut gradient_tile_builder = GradientTileBuilder::new();
        let mut image_texel_info = vec![];
        let mut used_image_hashes = HashSet::new();
        for paint in &self.paints {
            let allocator = &mut texture_manager.allocator;
            let color_texture_metadata = match paint.overlay {
                None => None,
                Some(ref overlay) => {
                    match overlay.contents {
                        PaintContents::Gradient(ref gradient) => {
                            let mut sampling_flags = TextureSamplingFlags::empty();
                            match gradient.wrap {
                                GradientWrap::Repeat => {
                                    sampling_flags.insert(TextureSamplingFlags::REPEAT_U);
                                }
                                GradientWrap::Clamp => {}
                            }

                            // FIXME(pcwalton): The gradient size might not be big enough. Detect
                            // this.
                            let location =
                                gradient_tile_builder.allocate(allocator,
                                                               transient_paint_locations,
                                                               gradient);
                            Some(PaintColorTextureMetadata {
                                location,
                                page_scale: allocator.page_scale(location.page),
                                sampling_flags,
                                filter: match gradient.geometry {
                                    GradientGeometry::Linear(_) => PaintFilter::None,
                                    GradientGeometry::Radial { line, radii, .. } => {
                                        PaintFilter::RadialGradient { line, radii }
                                    }
                                },
                                transform: Transform2F::default(),
                                composite_op: overlay.composite_op(),
                                border: Vector2I::zero(),
                            })
                        }
                        PaintContents::Pattern(ref pattern) => {
                            let border = vec2i(if pattern.repeat_x() { 0 } else { 1 },
                                               if pattern.repeat_y() { 0 } else { 1 });

                            let location;
                            match *pattern.source() {
                                PatternSource::RenderTarget { id: render_target_id, .. } => {
                                    let index = render_target_id.render_target as usize;
                                    location = render_target_metadata[index].location;
                                }
                                PatternSource::Image(ref image) => {
                                    // TODO(pcwalton): We should be able to use tile cleverness to
                                    // repeat inside the atlas in some cases.
                                    let image_hash = image.get_hash();
                                    match texture_manager.cached_images.get(&image_hash) {
                                        Some(cached_location) => {
                                            location = *cached_location;
                                            used_image_hashes.insert(image_hash);
                                        }
                                        None => {
                                            // Leave a pixel of border on the side.
                                            let allocation_mode = AllocationMode::OwnPage;
                                            location = allocator.allocate(
                                                image.size() + border * 2,
                                                allocation_mode);
                                            texture_manager.cached_images.insert(image_hash,
                                                                                 location);
                                        }
                                    }
                                    image_texel_info.push(ImageTexelInfo {
                                        location: TextureLocation {
                                            page: location.page,
                                            rect: location.rect.contract(border),
                                        },
                                        texels: (*image.pixels()).clone(),
                                    });
                                }
                            }

                            let mut sampling_flags = TextureSamplingFlags::empty();
                            if pattern.repeat_x() {
                                sampling_flags.insert(TextureSamplingFlags::REPEAT_U);
                            }
                            if pattern.repeat_y() {
                                sampling_flags.insert(TextureSamplingFlags::REPEAT_V);
                            }
                            if !pattern.smoothing_enabled() {
                                sampling_flags.insert(TextureSamplingFlags::NEAREST_MIN |
                                                    TextureSamplingFlags::NEAREST_MAG);
                            }

                            let filter = match pattern.filter() {
                                None => PaintFilter::None,
                                Some(pattern_filter) => PaintFilter::PatternFilter(pattern_filter),
                            };

                            Some(PaintColorTextureMetadata {
                                location,
                                page_scale: allocator.page_scale(location.page),
                                sampling_flags,
                                filter,
                                transform: Transform2F::from_translation(border.to_f32()),
                                composite_op: overlay.composite_op(),
                                border,
                            })
                        }
                    }
                }
            };

            paint_metadata.push(PaintMetadata {
                color_texture_metadata,
                is_opaque: paint.is_opaque(),
                base_color: paint.base_color(),
                // FIXME(pcwalton)
                blend_mode: BlendMode::SrcOver,
            });
        }

        PaintLocationsInfo {
            paint_metadata,
            gradient_tile_builder,
            image_texel_info,
            used_image_hashes,
        }
    }

    fn calculate_texture_transforms(&self,
                                    paint_metadata: &mut [PaintMetadata],
                                    texture_manager: &mut PaintTextureManager,
                                    render_transform: Transform2F) {
        for (paint, metadata) in self.paints.iter().zip(paint_metadata.iter_mut()) {
            let mut color_texture_metadata = match metadata.color_texture_metadata {
                None => continue,
                Some(ref mut color_texture_metadata) => color_texture_metadata,
            };

            let texture_scale = texture_manager.allocator
                                               .page_scale(color_texture_metadata.location.page);
            let texture_rect = color_texture_metadata.location.rect;
            color_texture_metadata.transform = match paint.overlay    
                                                          .as_ref()
                                                          .expect("Why do we have color texture \
                                                                   metadata but no overlay?")
                                                          .contents {
                PaintContents::Gradient(Gradient {
                    geometry: GradientGeometry::Linear(gradient_line),
                    ..
                }) => {
                    // Project gradient line onto (0.0-1.0, v0).
                    let v0 = texture_rect.to_f32().center().y() * texture_scale.y();
                    let dp = gradient_line.vector();
                    let m0 = dp.0.concat_xy_xy(dp.0) / F32x4::splat(gradient_line.square_length());
                    let m13 = m0.zw() * -gradient_line.from().0;
                    Transform2F::row_major(m0.x(), m0.y(), m13.x() + m13.y(), 0.0, 0.0, v0)
                }
                PaintContents::Gradient(Gradient {
                    geometry: GradientGeometry::Radial { ref transform, .. },
                    ..
                }) => transform.inverse(),
                PaintContents::Pattern(ref pattern) => {
                    match pattern.source() {
                        PatternSource::Image(_) => {
                            let texture_origin_uv =
                                rect_to_uv(texture_rect, texture_scale).origin();
                            Transform2F::from_scale(texture_scale).translate(texture_origin_uv) *
                                pattern.transform().inverse()
                        }
                        PatternSource::RenderTarget { .. } => {
                            // FIXME(pcwalton): Only do this in GL, not Metal!
                            let texture_origin_uv =
                                rect_to_uv(texture_rect, texture_scale).lower_left();
                            Transform2F::from_translation(texture_origin_uv) *
                                Transform2F::from_scale(texture_scale * vec2f(1.0, -1.0)) *
                                pattern.transform().inverse()
                        }
                    }
                }
            };
            color_texture_metadata.transform *= render_transform;
        }
    }

    fn create_texture_metadata(&self, paint_metadata: &[PaintMetadata])
                               -> Vec<TextureMetadataEntry> {
        paint_metadata.iter().map(|paint_metadata| {
            TextureMetadataEntry {
                color_0_transform: match paint_metadata.color_texture_metadata {
                    None => Transform2F::default(),
                    Some(ref color_texture_metadata) => color_texture_metadata.transform,
                },
                color_0_combine_mode: if paint_metadata.color_texture_metadata.is_some() {
                    ColorCombineMode::SrcIn
                } else {
                    ColorCombineMode::None
                },
                base_color: paint_metadata.base_color,
                filter: paint_metadata.filter(),
                blend_mode: paint_metadata.blend_mode,
            }
        }).collect()
    }

    fn allocate_textures(&self,
                         render_commands: &mut Vec<RenderCommand>,
                         texture_manager: &mut PaintTextureManager) {
        for page_id in texture_manager.allocator.page_ids() {
            let page_size = texture_manager.allocator.page_size(page_id);
            let descriptor = TexturePageDescriptor { size: page_size };

            if texture_manager.allocator.page_is_new(page_id) {
                render_commands.push(RenderCommand::AllocateTexturePage { page_id, descriptor });
            }
        }
        texture_manager.allocator.mark_all_pages_as_allocated();
    }

    fn create_render_commands(&self,
                              render_commands: &mut Vec<RenderCommand>,
                              render_target_metadata: Vec<RenderTargetMetadata>,
                              gradient_tile_builder: GradientTileBuilder,
                              image_texel_info: Vec<ImageTexelInfo>) {
        for (index, metadata) in render_target_metadata.iter().enumerate() {
            let id = RenderTargetId { scene: self.scene_id.0, render_target: index as u32 };
            render_commands.push(RenderCommand::DeclareRenderTarget {
                id,
                location: metadata.location,
            });
        }
        gradient_tile_builder.create_render_commands(render_commands);
        for image_texel_info in image_texel_info {
            render_commands.push(RenderCommand::UploadTexelData {
                texels: image_texel_info.texels,
                location: image_texel_info.location,
            });
        }
    }

    fn free_transient_locations(&self,
                                texture_manager: &mut PaintTextureManager,
                                transient_paint_locations: Vec<TextureLocation>) {
        for location in transient_paint_locations {
            texture_manager.allocator.free(location);
        }
    }

    // Frees images that are cached but not used this frame.
    fn free_unused_images(&self,
                          texture_manager: &mut PaintTextureManager,
                          used_image_hashes: HashSet<ImageHash>) {
        let cached_images = &mut texture_manager.cached_images;
        let allocator = &mut texture_manager.allocator;
        cached_images.retain(|image_hash, location| {
            let keep = used_image_hashes.contains(image_hash);
            if !keep {
                allocator.free(*location);
            }
            keep
        });
    }

    pub(crate) fn append_palette(&mut self, palette: Palette) -> MergedPaletteInfo {
        // Merge render targets.
        let mut render_target_mapping = HashMap::new();
        for (old_render_target_index, render_target) in palette.render_targets
                                                               .into_iter()
                                                               .enumerate() {
            let old_render_target_id = RenderTargetId {
                scene: palette.scene_id.0,
                render_target: old_render_target_index as u32,
            };
            let new_render_target_id = self.push_render_target(render_target);
            render_target_mapping.insert(old_render_target_id, new_render_target_id);
        }

        // Merge paints.
        let mut paint_mapping = HashMap::new();
        for (old_paint_index, old_paint) in palette.paints.iter().enumerate() {
            let old_paint_id = PaintId(old_paint_index as u16);
            let new_paint_id = match *old_paint.overlay() {
                None => self.push_paint(old_paint),
                Some(ref overlay) => {
                    match *overlay.contents() {
                        PaintContents::Pattern(ref pattern) => {
                            match pattern.source() {
                                PatternSource::RenderTarget { id: old_render_target_id, size } => {
                                    let mut new_pattern =
                                        Pattern::from_render_target(*old_render_target_id, *size);
                                    new_pattern.set_filter(pattern.filter());
                                    new_pattern.apply_transform(pattern.transform());
                                    new_pattern.set_repeat_x(pattern.repeat_x());
                                    new_pattern.set_repeat_y(pattern.repeat_y());
                                    new_pattern.set_smoothing_enabled(pattern.smoothing_enabled());
                                    self.push_paint(&Paint::from_pattern(new_pattern))
                                }
                                _ => self.push_paint(old_paint),
                            }
                        }
                        _ => self.push_paint(old_paint),
                    }
                }
            };
            paint_mapping.insert(old_paint_id, new_paint_id);
        }

        MergedPaletteInfo { render_target_mapping, paint_mapping }
    }
}

impl PaintTextureManager {
    pub(crate) fn new() -> PaintTextureManager {
        PaintTextureManager {
            allocator: TextureAllocator::new(),
            cached_images: HashMap::new(),
        }
    }
}

pub(crate) struct MergedPaletteInfo {
    pub(crate) render_target_mapping: HashMap<RenderTargetId, RenderTargetId>,
    pub(crate) paint_mapping: HashMap<PaintId, PaintId>,
}

impl PaintMetadata {
    pub(crate) fn filter(&self) -> Filter {
        match self.color_texture_metadata {
            None => Filter::None,
            Some(ref color_metadata) => {
                match color_metadata.filter {
                    PaintFilter::None => Filter::None,
                    PaintFilter::RadialGradient { line, radii } => {
                        let uv_rect = rect_to_uv(color_metadata.location.rect,
                                                 color_metadata.page_scale).contract(
                            vec2f(0.0, color_metadata.page_scale.y() * 0.5));
                        Filter::RadialGradient { line, radii, uv_origin: uv_rect.origin() }
                    }
                    PaintFilter::PatternFilter(pattern_filter) => {
                        Filter::PatternFilter(pattern_filter)
                    }
                }
            }
        }
    }

    pub(crate) fn tile_batch_texture(&self) -> Option<TileBatchTexture> {
        self.color_texture_metadata.as_ref().map(PaintColorTextureMetadata::as_tile_batch_texture)
    }
}

fn rect_to_uv(rect: RectI, texture_scale: Vector2F) -> RectF {
    rect.to_f32() * texture_scale
}

// Gradient allocation

struct GradientTileBuilder {
    tiles: Vec<GradientTile>,
}

struct GradientTile {
    texels: Vec<ColorU>,
    page: TexturePageId,
    next_index: u32,
}

impl GradientTileBuilder {
    fn new() -> GradientTileBuilder {
        GradientTileBuilder { tiles: vec![] }
    }

    fn allocate(&mut self,
                allocator: &mut TextureAllocator,
                transient_paint_locations: &mut Vec<TextureLocation>,
                gradient: &Gradient)
                -> TextureLocation {
        if self.tiles.is_empty() ||
                self.tiles.last().unwrap().next_index == GRADIENT_TILE_LENGTH {
            let size = Vector2I::splat(GRADIENT_TILE_LENGTH as i32);
            let area = size.x() as usize * size.y() as usize;
            let page_location = allocator.allocate(size, AllocationMode::OwnPage);
            transient_paint_locations.push(page_location);
            self.tiles.push(GradientTile {
                texels: vec![ColorU::black(); area],
                page: page_location.page,
                next_index: 0,
            })
        }

        let mut data = self.tiles.last_mut().unwrap();
        let location = TextureLocation {
            page: data.page,
            rect: RectI::new(vec2i(0, data.next_index as i32),
                             vec2i(GRADIENT_TILE_LENGTH as i32, 1)),
        };
        data.next_index += 1;

        // FIXME(pcwalton): Paint transparent if gradient line has zero size, per spec.
        // TODO(pcwalton): Optimize this:
        // 1. Calculate ∇t up front and use differencing in the inner loop.
        // 2. Go four pixels at a time with SIMD.
        let first_address = location.rect.origin_y() as usize * GRADIENT_TILE_LENGTH as usize;
        for x in 0..(GRADIENT_TILE_LENGTH as i32) {
            let t = (x as f32 + 0.5) / GRADIENT_TILE_LENGTH as f32;
            data.texels[first_address + x as usize] = gradient.sample(t);
        }

        location
    }

    fn create_render_commands(self, render_commands: &mut Vec<RenderCommand>) {
        for tile in self.tiles {
            render_commands.push(RenderCommand::UploadTexelData {
                texels: Arc::new(tile.texels),
                location: TextureLocation {
                    rect: RectI::new(vec2i(0, 0), Vector2I::splat(GRADIENT_TILE_LENGTH as i32)),
                    page: tile.page,
                },
            });
        }
    }
}

struct PaintLocationsInfo {
    paint_metadata: Vec<PaintMetadata>,
    gradient_tile_builder: GradientTileBuilder,
    image_texel_info: Vec<ImageTexelInfo>,
    used_image_hashes: HashSet<ImageHash>,
}

struct ImageTexelInfo {
    location: TextureLocation,
    texels: Arc<Vec<ColorU>>,
}

impl PaintColorTextureMetadata {
    pub(crate) fn as_tile_batch_texture(&self) -> TileBatchTexture {
        TileBatchTexture {
            page: self.location.page,
            sampling_flags: self.sampling_flags,
            composite_op: self.composite_op,
        }
    }
}
