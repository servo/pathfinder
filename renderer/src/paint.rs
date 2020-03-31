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
use crate::gpu_data::{RenderCommand, TextureLocation, TexturePageDescriptor, TexturePageId};
use crate::scene::RenderTarget;
use crate::tiles::{TILE_HEIGHT, TILE_WIDTH};
use hashbrown::HashMap;
use pathfinder_color::ColorU;
use pathfinder_content::effects::{Effects, Filter};
use pathfinder_content::gradient::Gradient;
use pathfinder_content::pattern::{Image, Pattern, PatternFlags, PatternSource};
use pathfinder_content::render_target::RenderTargetId;
use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_geometry::rect::{RectF, RectI};
use pathfinder_geometry::transform2d::{Matrix2x2F, Transform2F};
use pathfinder_geometry::util;
use pathfinder_geometry::vector::{Vector2F, Vector2I, vec2f, vec2i};
use pathfinder_gpu::TextureSamplingFlags;
use pathfinder_simd::default::{F32x2, F32x4};
use std::fmt::{self, Debug, Formatter};

// The size of a gradient tile.
//
// TODO(pcwalton): Choose this size dynamically!
const GRADIENT_TILE_LENGTH: u32 = 256;

const SOLID_COLOR_TILE_LENGTH: u32 = 16;
const MAX_SOLID_COLORS_PER_TILE: u32 = SOLID_COLOR_TILE_LENGTH * SOLID_COLOR_TILE_LENGTH;

#[derive(Clone)]
pub struct Palette {
    pub(crate) paints: Vec<Paint>,
    pub(crate) render_targets: Vec<RenderTarget>,
    cache: HashMap<Paint, PaintId>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Paint {
    Color(ColorU),
    Gradient(Gradient),
    Pattern(Pattern),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PaintId(pub u16);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GradientId(pub u32);

impl Debug for Paint {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match *self {
            Paint::Color(color) => color.fmt(formatter),
            Paint::Gradient(_) => {
                // TODO(pcwalton)
                write!(formatter, "(gradient)")
            }
            Paint::Pattern(ref pattern) => pattern.fmt(formatter),
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
    pub fn black() -> Paint {
        Paint::Color(ColorU::black())
    }

    #[inline]
    pub fn transparent_black() -> Paint {
        Paint::Color(ColorU::transparent_black())
    }

    pub fn is_opaque(&self) -> bool {
        match *self {
            Paint::Color(color) => color.is_opaque(),
            Paint::Gradient(ref gradient) => {
                gradient.stops().iter().all(|stop| stop.color.is_opaque())
            }
            Paint::Pattern(ref pattern) => pattern.source.is_opaque(),
        }
    }

    pub fn is_fully_transparent(&self) -> bool {
        match *self {
            Paint::Color(color) => color.is_fully_transparent(),
            Paint::Gradient(ref gradient) => {
                gradient.stops().iter().all(|stop| stop.color.is_fully_transparent())
            }
            Paint::Pattern(_) => {
                // TODO(pcwalton): Should we support this?
                false
            }
        }
    }

    #[inline]
    pub fn is_color(&self) -> bool {
        match *self {
            Paint::Color(_) => true,
            Paint::Gradient(_) | Paint::Pattern(_) => false,
        }
    }

    pub fn apply_transform(&mut self, transform: &Transform2F) {
        if transform.is_identity() {
            return;
        }

        match *self {
            Paint::Color(_) => {}
            Paint::Gradient(ref mut gradient) => {
                gradient.set_line(*transform * gradient.line());
                if let Some(radii) = gradient.radii() {
                    gradient.set_radii(Some(radii * F32x2::splat(util::lerp(transform.matrix.m11(),
                                                                            transform.matrix.m22(),
                                                                            0.5))));
                }
            }
            Paint::Pattern(ref mut pattern) => pattern.transform = *transform * pattern.transform,
        }
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
    /// The page containing the opacity tile.
    pub opacity_tile_page: TexturePageId,
    /// The transform for the opacity tile.
    pub opacity_tile_transform: Transform2F,
}

#[derive(Debug)]
pub struct PaintMetadata {
    /// The location of the paint.
    pub location: TextureLocation,
    /// The transform to apply to screen coordinates to translate them into UVs.
    pub texture_transform: Transform2F,
    /// The sampling mode for the texture.
    pub sampling_flags: TextureSamplingFlags,
    /// True if this paint is fully opaque.
    pub is_opaque: bool,
    /// The radial gradient for this paint, if applicable.
    pub radial_gradient: Option<RadialGradientMetadata>,
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

    pub fn build_paint_info(&self) -> PaintInfo {
        let mut allocator = TextureAllocator::new();
        let (mut paint_metadata, mut render_target_metadata) = (vec![], vec![]);

        // Assign render target locations.
        for render_target in &self.render_targets {
            render_target_metadata.push(RenderTargetMetadata {
                location: allocator.allocate_image(render_target.size()),
            });
        }

        // Assign paint locations.
        let opacity_tile_builder = OpacityTileBuilder::new(&mut allocator);
        let mut solid_color_tile_builder = SolidColorTileBuilder::new();
        let mut gradient_tile_builder = GradientTileBuilder::new();
        for paint in &self.paints {
            let (texture_location, mut sampling_flags, radial_gradient);
            match paint {
                Paint::Color(_) => {
                    texture_location = solid_color_tile_builder.allocate(&mut allocator);
                    sampling_flags = TextureSamplingFlags::empty();
                    radial_gradient = None;
                }
                Paint::Gradient(ref gradient) => {
                    // FIXME(pcwalton): The gradient size might not be big enough. Detect this.
                    texture_location = gradient_tile_builder.allocate(&mut allocator);
                    sampling_flags = TextureSamplingFlags::empty();
                    radial_gradient = gradient.radii().map(|radii| {
                        RadialGradientMetadata { line: gradient.line(), radii }
                    });
                }
                Paint::Pattern(ref pattern) => {
                    match pattern.source {
                        PatternSource::RenderTarget(render_target_id) => {
                            texture_location =
                                render_target_metadata[render_target_id.0 as usize].location;
                        }
                        PatternSource::Image(ref image) => {
                            // TODO(pcwalton): We should be able to use tile cleverness to repeat
                            // inside the atlas in some cases.
                            let allocation_mode = AllocationMode::OwnPage;
                            texture_location = allocator.allocate(image.size(), allocation_mode);
                        }
                    }

                    sampling_flags = TextureSamplingFlags::empty();
                    if pattern.flags.contains(PatternFlags::REPEAT_X) {
                        sampling_flags.insert(TextureSamplingFlags::REPEAT_U);
                    }
                    if pattern.flags.contains(PatternFlags::REPEAT_Y) {
                        sampling_flags.insert(TextureSamplingFlags::REPEAT_V);
                    }
                    if pattern.flags.contains(PatternFlags::NO_SMOOTHING) {
                        sampling_flags.insert(TextureSamplingFlags::NEAREST_MIN |
                                              TextureSamplingFlags::NEAREST_MAG);
                    }

                    radial_gradient = None;
                }
            };

            paint_metadata.push(PaintMetadata {
                location: texture_location,
                texture_transform: Transform2F::default(),
                sampling_flags,
                is_opaque: paint.is_opaque(),
                radial_gradient,
            });
        }

        // Calculate texture transforms.
        for (paint, metadata) in self.paints.iter().zip(paint_metadata.iter_mut()) {
            let texture_scale = allocator.page_scale(metadata.location.page);
            metadata.texture_transform = match paint {
                Paint::Color(_) => {
                    let vector = rect_to_inset_uv(metadata.location.rect, texture_scale).origin();
                    Transform2F { matrix: Matrix2x2F(F32x4::default()), vector }
                }
                Paint::Gradient(Gradient { line: gradient_line, radii: None, .. }) => {
                    let v0 = metadata.location.rect.to_f32().center().y() * texture_scale.y();
                    let length_inv = 1.0 / gradient_line.square_length();
                    let (p0, d) = (gradient_line.from(), gradient_line.vector());
                    Transform2F {
                        matrix: Matrix2x2F::row_major(d.x(), d.y(), 0.0, 0.0).scale(length_inv),
                        vector: Vector2F::new(-p0.dot(d) * length_inv, v0),
                    }
                }
                Paint::Gradient(Gradient { radii: Some(_), .. }) => {
                    let texture_origin_uv =
                        rect_to_inset_uv(metadata.location.rect, texture_scale).origin();
                    let gradient_tile_scale =
                        texture_scale.scale((GRADIENT_TILE_LENGTH - 1) as f32);
                    Transform2F {
                        matrix: Matrix2x2F::from_scale(gradient_tile_scale),
                        vector: texture_origin_uv,
                    }
                }
                Paint::Pattern(Pattern { source: PatternSource::Image(_), transform, .. }) => {
                    let texture_origin_uv =
                        rect_to_uv(metadata.location.rect, texture_scale).origin();
                    Transform2F::from_translation(texture_origin_uv) *
                        Transform2F::from_scale(texture_scale) *
                        transform.inverse()
                }
                Paint::Pattern(Pattern {
                    source: PatternSource::RenderTarget(_),
                    transform,
                    ..
                }) => {
                    // FIXME(pcwalton): Only do this in GL, not Metal!
                    let texture_origin_uv = rect_to_uv(metadata.location.rect,
                                                       texture_scale).lower_left();
                    Transform2F::from_translation(texture_origin_uv) *
                        Transform2F::from_scale(texture_scale.scale_xy(vec2f(1.0, -1.0))) *
                        transform.inverse()
                }
            }
        }

        // Allocate textures.
        let mut texture_page_descriptors = vec![];
        for page_index in 0..allocator.page_count() {
            let page_size = allocator.page_size(TexturePageId(page_index));
            texture_page_descriptors.push(TexturePageDescriptor { size: page_size });
        }

        // Allocate the texels.
        //
        // TODO(pcwalton): This is slow. Do more on GPU.
        let mut page_texels: Vec<_> =
            texture_page_descriptors.iter()
                                    .map(|descriptor| Texels::new(descriptor.size))
                                    .collect();

        // Draw to texels.
        //
        // TODO(pcwalton): Do more of this on GPU.
        opacity_tile_builder.render(&mut page_texels);
        for (paint, metadata) in self.paints.iter().zip(paint_metadata.iter()) {
            let texture_page = metadata.location.page;
            let texels = &mut page_texels[texture_page.0 as usize];

            match paint {
                Paint::Color(color) => {
                    texels.put_texel(metadata.location.rect.origin(), *color);
                }
                Paint::Gradient(ref gradient) => {
                    self.render_gradient(gradient, metadata.location.rect, texels);
                }
                Paint::Pattern(ref pattern) => {
                    match pattern.source {
                        PatternSource::RenderTarget(_) => {}
                        PatternSource::Image(ref image) => {
                            self.render_image(image, metadata.location.rect, texels);
                        }
                    }
                }
            }
        }

        // Create render commands.
        let mut render_commands = vec![
            RenderCommand::AllocateTexturePages(texture_page_descriptors)
        ];
        for (index, metadata) in render_target_metadata.iter().enumerate() {
            let id = RenderTargetId(index as u32);
            render_commands.push(RenderCommand::DeclareRenderTarget {
                id,
                location: metadata.location,
            });
        }
        for (page_index, texels) in page_texels.into_iter().enumerate() {
            if let Some(texel_data) = texels.data {
                let page_id = TexturePageId(page_index as u32);
                let page_size = allocator.page_size(page_id);
                let rect = RectI::new(Vector2I::default(), page_size);
                render_commands.push(RenderCommand::UploadTexelData {
                    texels: texel_data,
                    location: TextureLocation { page: page_id, rect },
                });
            }
        }

        PaintInfo {
            render_commands,
            paint_metadata,
            render_target_metadata,
            opacity_tile_page: opacity_tile_builder.tile_location.page,
            opacity_tile_transform: opacity_tile_builder.tile_transform(&allocator),
        }
    }

    // TODO(pcwalton): This is slow. Do on GPU instead.
    fn render_gradient(&self, gradient: &Gradient, tex_rect: RectI, texels: &mut Texels) {
        // FIXME(pcwalton): Paint transparent if gradient line has zero size, per spec.
        // TODO(pcwalton): Optimize this:
        // 1. Calculate ∇t up front and use differencing in the inner loop.
        // 2. Go four pixels at a time with SIMD.
        for x in 0..(GRADIENT_TILE_LENGTH as i32) {
            let point = tex_rect.origin() + vec2i(x, 0);
            let t = (x as f32 + 0.5) / GRADIENT_TILE_LENGTH as f32;
            texels.put_texel(point, gradient.sample(t));
        }
    }

    fn render_image(&self, image: &Image, tex_rect: RectI, texels: &mut Texels) {
        let image_size = image.size();
        for y in 0..image_size.y() {
            let dest_origin = tex_rect.origin() + vec2i(0, y);
            let src_start_index = y as usize * image_size.x() as usize;
            let src_end_index = src_start_index + image_size.x() as usize;
            texels.blit_scanline(dest_origin, &image.pixels()[src_start_index..src_end_index]);
        }
    }
}

impl PaintMetadata {
    // TODO(pcwalton): Apply clamp/repeat to tile rect.
    pub(crate) fn calculate_tex_coords(&self, tile_position: Vector2I) -> Vector2F {
        let tile_size = vec2i(TILE_WIDTH as i32, TILE_HEIGHT as i32);
        let position = tile_position.scale_xy(tile_size).to_f32();
        let tex_coords = self.texture_transform * position;
        tex_coords
    }

    pub(crate) fn effects(&self) -> Effects {
        Effects {
            filter: match self.radial_gradient {
                None => Filter::None,
                Some(gradient) => {
                    Filter::RadialGradient {
                        line: gradient.line,
                        radii: gradient.radii,
                        uv_origin: self.texture_transform.vector,
                    }
                }
            },
        }
    }
}

struct Texels {
    data: Option<Vec<ColorU>>,
    size: Vector2I,
}

impl Texels {
    fn new(size: Vector2I) -> Texels {
        Texels { data: None, size }
    }

    fn texel_index(&self, position: Vector2I) -> usize {
        position.y() as usize * self.size.x() as usize + position.x() as usize
    }

    fn allocate_texels_if_necessary(&mut self) {
        if self.data.is_none() {
            let area = self.size.x() as usize * self.size.y() as usize;
            self.data = Some(vec![ColorU::transparent_black(); area]);
        }
    }

    fn blit_scanline(&mut self, dest_origin: Vector2I, src: &[ColorU]) {
        self.allocate_texels_if_necessary();
        let start_index = self.texel_index(dest_origin);
        let end_index = start_index + src.len();
        self.data.as_mut().unwrap()[start_index..end_index].copy_from_slice(src)
    }

    fn put_texel(&mut self, position: Vector2I, color: ColorU) {
        self.blit_scanline(position, &[color])
    }
}

fn rect_to_uv(rect: RectI, texture_scale: Vector2F) -> RectF {
    rect.to_f32().scale_xy(texture_scale)
}

fn rect_to_inset_uv(rect: RectI, texture_scale: Vector2F) -> RectF {
    rect_to_uv(rect, texture_scale).contract(texture_scale.scale(0.5))
}

// Opacity allocation

struct OpacityTileBuilder {
    tile_location: TextureLocation,
}

impl OpacityTileBuilder {
    fn new(allocator: &mut TextureAllocator) -> OpacityTileBuilder {
        OpacityTileBuilder {
            tile_location: allocator.allocate(Vector2I::splat(16), AllocationMode::Atlas),
        }
    }

    fn render(&self, page_texels: &mut [Texels]) {
        let texels = &mut page_texels[self.tile_location.page.0 as usize];
        for y in 0..16 {
            for x in 0..16 {
                let color = ColorU::new(0xff, 0xff, 0xff, y * 16 + x);
                let coords = self.tile_location.rect.origin() + vec2i(x as i32, y as i32);
                texels.put_texel(coords, color);
            }
        }
    }

    fn tile_transform(&self, allocator: &TextureAllocator) -> Transform2F {
        let texture_scale = allocator.page_scale(self.tile_location.page);
        let matrix = Matrix2x2F::from_scale(texture_scale.scale(16.0));
        let vector = rect_to_uv(self.tile_location.rect, texture_scale).origin();
        Transform2F { matrix, vector }
    }
}

// Solid color allocation

struct SolidColorTileBuilder(Option<SolidColorTileBuilderData>);

struct SolidColorTileBuilderData {
    tile_location: TextureLocation,
    next_index: u32,
}

impl SolidColorTileBuilder {
    fn new() -> SolidColorTileBuilder {
        SolidColorTileBuilder(None)
    }

    fn allocate(&mut self, allocator: &mut TextureAllocator) -> TextureLocation {
        if self.0.is_none() {
            // TODO(pcwalton): Handle allocation failure gracefully!
            self.0 = Some(SolidColorTileBuilderData {
                tile_location: allocator.allocate(Vector2I::splat(SOLID_COLOR_TILE_LENGTH as i32),
                                                  AllocationMode::Atlas),
                next_index: 0,
            });
        }

        let (location, tile_full);
        {
            let mut data = self.0.as_mut().unwrap();
            let subtile_origin = vec2i((data.next_index % SOLID_COLOR_TILE_LENGTH) as i32,
                                       (data.next_index / SOLID_COLOR_TILE_LENGTH) as i32);
            location = TextureLocation {
                page: data.tile_location.page,
                rect: RectI::new(data.tile_location.rect.origin() + subtile_origin,
                                 Vector2I::splat(1)),
            };
            data.next_index += 1;
            tile_full = data.next_index == MAX_SOLID_COLORS_PER_TILE;
        }

        if tile_full {
            self.0 = None;
        }

        location
    }
}

// Gradient allocation

struct GradientTileBuilder(Option<GradientTileBuilderData>);

struct GradientTileBuilderData {
    page: TexturePageId,
    next_index: u32,
}

impl GradientTileBuilder {
    fn new() -> GradientTileBuilder {
        GradientTileBuilder(None)
    }

    fn allocate(&mut self, allocator: &mut TextureAllocator) -> TextureLocation {
        if self.0.is_none() {
            let size = Vector2I::splat(GRADIENT_TILE_LENGTH as i32);
            self.0 = Some(GradientTileBuilderData {
                page: allocator.allocate(size, AllocationMode::OwnPage).page,
                next_index: 0,
            })
        }

        let (location, tile_full);
        {
            let mut data = self.0.as_mut().unwrap();
            location = TextureLocation {
                page: data.page,
                rect: RectI::new(vec2i(0, data.next_index as i32),
                                 vec2i(GRADIENT_TILE_LENGTH as i32, 1)),
            };
            data.next_index += 1;
            tile_full = data.next_index == GRADIENT_TILE_LENGTH;
        }

        if tile_full {
            self.0 = None;
        }

        location
    }
}
