// pathfinder/renderer/src/paint.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::allocator::{AllocationMode, TextureAllocator};
use crate::gpu_data::{RenderCommand, TextureLocation, TextureMetadataEntry, TexturePageDescriptor};
use crate::gpu_data::{TexturePageId, TileBatchTexture};
use crate::scene::RenderTarget;
use hashbrown::HashMap;
use pathfinder_color::ColorU;
use pathfinder_content::effects::{Filter, PatternFilter};
use pathfinder_content::gradient::Gradient;
use pathfinder_content::pattern::{Pattern, PatternSource};
use pathfinder_content::render_target::RenderTargetId;
use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_geometry::rect::{RectF, RectI};
use pathfinder_geometry::transform2d::{Matrix2x2F, Transform2F};
use pathfinder_geometry::vector::{Vector2F, Vector2I, vec2f, vec2i};
use pathfinder_gpu::TextureSamplingFlags;
use pathfinder_simd::default::F32x2;
use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;

// The size of a gradient tile.
//
// TODO(pcwalton): Choose this size dynamically!
const GRADIENT_TILE_LENGTH: u32 = 256;

#[derive(Clone)]
pub struct Palette {
    pub(crate) paints: Vec<Paint>,
    pub(crate) render_targets: Vec<RenderTarget>,
    cache: HashMap<Paint, PaintId>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Paint {
    base_color: ColorU,
    overlay: Option<PaintOverlay>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct PaintOverlay {
    composite_op: PaintCompositeOp,
    contents: PaintContents,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum PaintContents {
    Gradient(Gradient),
    Pattern(Pattern),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PaintId(pub u16);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GradientId(pub u32);

/// How a paint is to be composited over a base color, or vice versa.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PaintCompositeOp {
    SrcIn,
    DestIn,
}

impl Debug for PaintContents {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match *self {
            PaintContents::Gradient(_) => {
                // TODO(pcwalton)
                write!(formatter, "(gradient)")
            }
            PaintContents::Pattern(ref pattern) => pattern.fmt(formatter),
        }
    }
}

impl Palette {
    #[inline]
    pub fn new() -> Palette {
        Palette { paints: vec![], render_targets: vec![], cache: HashMap::new() }
    }
}

impl Paint {
    #[inline]
    pub fn from_color(color: ColorU) -> Paint {
        Paint { base_color: color, overlay: None }
    }

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

    #[inline]
    pub fn black() -> Paint {
        Paint::from_color(ColorU::black())
    }

    #[inline]
    pub fn transparent_black() -> Paint {
        Paint::from_color(ColorU::transparent_black())
    }

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

    #[inline]
    pub fn is_color(&self) -> bool {
        self.overlay.is_none()
    }

    pub fn apply_transform(&mut self, transform: &Transform2F) {
        if transform.is_identity() {
            return;
        }

        if let Some(ref mut overlay) = self.overlay {
            match overlay.contents {
                PaintContents::Gradient(ref mut gradient) => {
                    gradient.set_line(*transform * gradient.line());
                    if let Some(radii) = gradient.radii() {
                        gradient.set_radii(Some(radii * transform.extract_scale().0));
                    }
                }
                PaintContents::Pattern(ref mut pattern) => pattern.apply_transform(*transform),
            }
        }
    }

    #[inline]
    pub fn base_color(&self) -> ColorU {
        self.base_color
    }

    #[inline]
    pub fn set_base_color(&mut self, new_base_color: ColorU) {
        self.base_color = new_base_color;
    }

    #[inline]
    pub fn overlay(&self) -> &Option<PaintOverlay> {
        &self.overlay
    }

    #[inline]
    pub fn overlay_mut(&mut self) -> &mut Option<PaintOverlay> {
        &mut self.overlay
    }

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
    pub fn contents(&self) -> &PaintContents {
        &self.contents
    }

    #[inline]
    pub fn composite_op(&self) -> PaintCompositeOp {
        self.composite_op
    }

    #[inline]
    pub fn set_composite_op(&mut self, new_composite_op: PaintCompositeOp) {
        self.composite_op = new_composite_op
    }
}

pub struct PaintInfo {
    /// The render commands needed to prepare the textures.
    pub render_commands: Vec<RenderCommand>,
    /// The metadata for each paint.
    ///
    /// The indices of this vector are paint IDs.
    pub paint_metadata: Vec<PaintMetadata>,
    /// The metadata for each render target.
    ///
    /// The indices of this vector are render target IDs.
    pub render_target_metadata: Vec<RenderTargetMetadata>,
}

#[derive(Debug)]
pub struct PaintMetadata {
    /// Metadata associated with the color texture, if applicable.
    pub color_texture_metadata: Option<PaintColorTextureMetadata>,
    /// The base color that the color texture gets mixed into.
    pub base_color: ColorU,
    /// True if this paint is fully opaque.
    pub is_opaque: bool,
}

#[derive(Debug)]
pub struct PaintColorTextureMetadata {
    /// The location of the paint.
    pub location: TextureLocation,
    /// The transform to apply to screen coordinates to translate them into UVs.
    pub transform: Transform2F,
    /// The sampling mode for the texture.
    pub sampling_flags: TextureSamplingFlags,
    /// The filter to be applied to this paint.
    pub filter: PaintFilter,
    /// How the color texture is to be composited over the base color.
    pub composite_op: PaintCompositeOp,
}

#[derive(Clone, Copy, Debug)]
pub struct RadialGradientMetadata {
    /// The line segment that connects the two circles.
    pub line: LineSegment2F,
    /// The radii of the two circles.
    pub radii: F32x2,
}

#[derive(Debug)]
pub struct RenderTargetMetadata {
    /// The location of the render target.
    pub location: TextureLocation,
}

#[derive(Debug)]
pub enum PaintFilter {
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
    pub fn push_paint(&mut self, paint: &Paint) -> PaintId {
        if let Some(paint_id) = self.cache.get(paint) {
            return *paint_id;
        }

        let paint_id = PaintId(self.paints.len() as u16);
        self.cache.insert((*paint).clone(), paint_id);
        self.paints.push((*paint).clone());
        paint_id
    }

    pub fn push_render_target(&mut self, render_target: RenderTarget) -> RenderTargetId {
        let id = RenderTargetId(self.render_targets.len() as u32);
        self.render_targets.push(render_target);
        id
    }

    pub fn build_paint_info(&self, render_transform: Transform2F) -> PaintInfo {
        let mut allocator = TextureAllocator::new();
        let (mut paint_metadata, mut render_target_metadata) = (vec![], vec![]);

        // Assign render target locations.
        for render_target in &self.render_targets {
            render_target_metadata.push(RenderTargetMetadata {
                location: allocator.allocate_image(render_target.size()),
            });
        }

        // Assign paint locations.
        let mut gradient_tile_builder = GradientTileBuilder::new();
        let mut image_texel_info = vec![];
        for paint in &self.paints {
            let color_texture_metadata = paint.overlay.as_ref().map(|overlay| {
                match overlay.contents {
                    PaintContents::Gradient(ref gradient) => {
                        // FIXME(pcwalton): The gradient size might not be big enough. Detect this.
                        PaintColorTextureMetadata {
                            location: gradient_tile_builder.allocate(&mut allocator, gradient),
                            sampling_flags: TextureSamplingFlags::empty(),
                            filter: match gradient.radii() {
                                None => PaintFilter::None,
                                Some(radii) => {
                                    PaintFilter::RadialGradient { line: gradient.line(), radii }
                                }
                            },
                            transform: Transform2F::default(),
                            composite_op: overlay.composite_op(),
                        }
                    }
                    PaintContents::Pattern(ref pattern) => {
                        let location;
                        match *pattern.source() {
                            PatternSource::RenderTarget { id: render_target_id, .. } => {
                                location =
                                    render_target_metadata[render_target_id.0 as usize].location;
                            }
                            PatternSource::Image(ref image) => {
                                // TODO(pcwalton): We should be able to use tile cleverness to
                                // repeat inside the atlas in some cases.
                                let allocation_mode = AllocationMode::OwnPage;
                                location = allocator.allocate(image.size(), allocation_mode);
                                image_texel_info.push(ImageTexelInfo {
                                    location,
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

                        PaintColorTextureMetadata {
                            location,
                            sampling_flags,
                            filter,
                            transform: Transform2F::default(),
                            composite_op: overlay.composite_op(),
                        }
                    }
                }
            });

            paint_metadata.push(PaintMetadata {
                color_texture_metadata,
                is_opaque: paint.is_opaque(),
                base_color: paint.base_color(),
            });
        }

        // Calculate texture transforms.
        for (paint, metadata) in self.paints.iter().zip(paint_metadata.iter_mut()) {
            let mut color_texture_metadata = match metadata.color_texture_metadata {
                None => continue,
                Some(ref mut color_texture_metadata) => color_texture_metadata,
            };

            let texture_scale = allocator.page_scale(color_texture_metadata.location.page);
            let texture_rect = color_texture_metadata.location.rect;
            color_texture_metadata.transform = match paint.overlay    
                                                          .as_ref()
                                                          .expect("Why do we have color texture \
                                                                   metadata but no overlay?")
                                                          .contents {
                PaintContents::Gradient(Gradient {
                    line: gradient_line,
                    radii: None,
                    ..
                }) => {
                    let v0 = texture_rect.to_f32().center().y() * texture_scale.y();
                    let length_inv = 1.0 / gradient_line.square_length();
                    let (p0, d) = (gradient_line.from(), gradient_line.vector());
                    Transform2F {
                        matrix: Matrix2x2F::row_major(
                            d.x(), d.y(), 0.0, 0.0).scale(length_inv),
                        vector: Vector2F::new(-p0.dot(d) * length_inv, v0),
                    } * render_transform
                }
                PaintContents::Gradient(Gradient { radii: Some(_), .. }) => {
                    let texture_origin_uv = rect_to_inset_uv(texture_rect, texture_scale).origin();
                    let gradient_tile_scale = texture_scale * (GRADIENT_TILE_LENGTH - 1) as
                        f32;
                    Transform2F {
                        matrix: Matrix2x2F::from_scale(gradient_tile_scale),
                        vector: texture_origin_uv,
                    } * render_transform
                }
                PaintContents::Pattern(ref pattern) => {
                    match pattern.source() {
                        PatternSource::Image(_) => {
                            let texture_origin_uv =
                                rect_to_uv(texture_rect, texture_scale).origin();
                            Transform2F::from_translation(texture_origin_uv) *
                                Transform2F::from_scale(texture_scale) *
                                pattern.transform().inverse() * render_transform
                        }
                        PatternSource::RenderTarget { .. } => {
                            // FIXME(pcwalton): Only do this in GL, not Metal!
                            let texture_origin_uv =
                                rect_to_uv(texture_rect, texture_scale).lower_left();
                            Transform2F::from_translation(texture_origin_uv) *
                                Transform2F::from_scale(texture_scale * vec2f(1.0, -1.0)) *
                                pattern.transform().inverse() * render_transform
                        }
                    }
                }
            }
        }

        // Allocate textures.
        let mut texture_page_descriptors = vec![];
        for page_index in 0..allocator.page_count() {
            let page_size = allocator.page_size(TexturePageId(page_index));
            texture_page_descriptors.push(TexturePageDescriptor { size: page_size });
        }

        // Create texture metadata.
        let texture_metadata = paint_metadata.iter().map(|paint_metadata| {
            TextureMetadataEntry {
                color_0_transform: match paint_metadata.color_texture_metadata {
                    None => Transform2F::default(),
                    Some(ref color_texture_metadata) => color_texture_metadata.transform,
                },
                base_color: paint_metadata.base_color,
            }
        }).collect();

        // Create render commands.
        let mut render_commands = vec![
            RenderCommand::UploadTextureMetadata(texture_metadata),
            RenderCommand::AllocateTexturePages(texture_page_descriptors),
        ];
        for (index, metadata) in render_target_metadata.iter().enumerate() {
            let id = RenderTargetId(index as u32);
            render_commands.push(RenderCommand::DeclareRenderTarget {
                id,
                location: metadata.location,
            });
        }
        gradient_tile_builder.create_render_commands(&mut render_commands);
        for image_texel_info in image_texel_info {
            render_commands.push(RenderCommand::UploadTexelData {
                texels: image_texel_info.texels,
                location: image_texel_info.location,
            });
        }

        PaintInfo { render_commands, paint_metadata, render_target_metadata }
    }
}

impl PaintMetadata {
    pub(crate) fn filter(&self) -> Filter {
        match self.color_texture_metadata {
            None => Filter::None,
            Some(ref color_metadata) => {
                match color_metadata.filter {
                    PaintFilter::None => Filter::None,
                    PaintFilter::RadialGradient { line, radii } => {
                        let uv_origin = color_metadata.transform.vector;
                        Filter::RadialGradient { line, radii, uv_origin }
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

fn rect_to_inset_uv(rect: RectI, texture_scale: Vector2F) -> RectF {
    rect_to_uv(rect, texture_scale).contract(texture_scale * 0.5)
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

    fn allocate(&mut self, allocator: &mut TextureAllocator, gradient: &Gradient)
                -> TextureLocation {
        if self.tiles.is_empty() ||
                self.tiles.last().unwrap().next_index == GRADIENT_TILE_LENGTH {
            let size = Vector2I::splat(GRADIENT_TILE_LENGTH as i32);
            let area = size.x() as usize * size.y() as usize;
            self.tiles.push(GradientTile {
                texels: vec![ColorU::black(); area],
                page: allocator.allocate(size, AllocationMode::OwnPage).page,
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
