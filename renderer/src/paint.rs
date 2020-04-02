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
use pathfinder_content::pattern::{Pattern, PatternFlags, PatternSource};
use pathfinder_content::render_target::RenderTargetId;
use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_geometry::rect::{RectF, RectI};
use pathfinder_geometry::transform2d::{Matrix2x2F, Transform2F};
use pathfinder_geometry::util;
use pathfinder_geometry::vector::{Vector2F, Vector2I, vec2f, vec2i};
use pathfinder_gpu::TextureSamplingFlags;
use pathfinder_simd::default::{F32x2, F32x4};
use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;

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
        let opacity_tile_builder = OpacityTileBuilder::new(&mut allocator);
        let mut solid_color_tile_builder = SolidColorTileBuilder::new();
        let mut gradient_tile_builder = GradientTileBuilder::new();
        let mut image_texel_info = vec![];
        for paint in &self.paints {
            let (texture_location, mut sampling_flags, radial_gradient);
            match paint {
                Paint::Color(color) => {
                    texture_location = solid_color_tile_builder.allocate(&mut allocator, *color);
                    sampling_flags = TextureSamplingFlags::empty();
                    radial_gradient = None;
                }
                Paint::Gradient(ref gradient) => {
                    // FIXME(pcwalton): The gradient size might not be big enough. Detect this.
                    texture_location = gradient_tile_builder.allocate(&mut allocator, gradient);
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
                            image_texel_info.push(ImageTexelInfo {
                                location: texture_location,
                                texels: (*image.pixels()).clone(),
                            });
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
                    Transform2F { matrix: Matrix2x2F(F32x4::default()), vector } * render_transform.inverse()
                }
                Paint::Gradient(Gradient { line: gradient_line, radii: None, .. }) => {
                    let v0 = metadata.location.rect.to_f32().center().y() * texture_scale.y();
                    let length_inv = 1.0 / gradient_line.square_length();
                    let (p0, d) = (gradient_line.from(), gradient_line.vector());
                    Transform2F {
                        matrix: Matrix2x2F::row_major(d.x(), d.y(), 0.0, 0.0).scale(length_inv),
                        vector: Vector2F::new(-p0.dot(d) * length_inv, v0),
                    } * render_transform
                }
                Paint::Gradient(Gradient { radii: Some(_), .. }) => {
                    let texture_origin_uv =
                        rect_to_inset_uv(metadata.location.rect, texture_scale).origin();
                    let gradient_tile_scale = texture_scale * (GRADIENT_TILE_LENGTH - 1) as f32;
                    Transform2F {
                        matrix: Matrix2x2F::from_scale(gradient_tile_scale),
                        vector: texture_origin_uv,
                    } * render_transform
                }
                Paint::Pattern(Pattern { source: PatternSource::Image(_), transform, .. }) => {
                    let texture_origin_uv =
                        rect_to_uv(metadata.location.rect, texture_scale).origin();
                    Transform2F::from_translation(texture_origin_uv) *
                        Transform2F::from_scale(texture_scale) *
                        transform.inverse() * render_transform
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
                        Transform2F::from_scale(texture_scale * vec2f(1.0, -1.0)) *
                        transform.inverse() * render_transform
                }
            }
        }

        // Allocate textures.
        let mut texture_page_descriptors = vec![];
        for page_index in 0..allocator.page_count() {
            let page_size = allocator.page_size(TexturePageId(page_index));
            texture_page_descriptors.push(TexturePageDescriptor { size: page_size });
        }

        // Gather opacity tile metadata.
        let opacity_tile_page = opacity_tile_builder.tile_location.page;
        let opacity_tile_transform = opacity_tile_builder.tile_transform(&allocator);

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
        solid_color_tile_builder.create_render_commands(&mut render_commands);
        gradient_tile_builder.create_render_commands(&mut render_commands);
        opacity_tile_builder.create_render_commands(&mut render_commands);
        for image_texel_info in image_texel_info {
            render_commands.push(RenderCommand::UploadTexelData {
                texels: image_texel_info.texels,
                location: image_texel_info.location,
            });
        }

        PaintInfo {
            render_commands,
            paint_metadata,
            render_target_metadata,
            opacity_tile_page,
            opacity_tile_transform,
        }
    }
}

impl PaintMetadata {
    // TODO(pcwalton): Apply clamp/repeat to tile rect.
    pub(crate) fn calculate_tex_coords(&self, tile_position: Vector2I) -> Vector2F {
        let tile_size = vec2i(TILE_WIDTH as i32, TILE_HEIGHT as i32);
        let position = (tile_position * tile_size).to_f32();
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

fn rect_to_uv(rect: RectI, texture_scale: Vector2F) -> RectF {
    rect.to_f32() * texture_scale
}

fn rect_to_inset_uv(rect: RectI, texture_scale: Vector2F) -> RectF {
    rect_to_uv(rect, texture_scale).contract(texture_scale * 0.5)
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

    fn create_texels(&self) -> Vec<ColorU> {
        let mut texels = Vec::with_capacity(256);
        for alpha in 0..=255 {
            texels.push(ColorU::new(255, 255, 255, alpha));
        }
        texels
    }

    fn tile_transform(&self, allocator: &TextureAllocator) -> Transform2F {
        let texture_scale = allocator.page_scale(self.tile_location.page);
        let matrix = Matrix2x2F::from_scale(texture_scale * 16.0);
        let vector = rect_to_uv(self.tile_location.rect, texture_scale).origin();
        Transform2F { matrix, vector }
    }

    fn create_render_commands(self, render_commands: &mut Vec<RenderCommand>) {
        render_commands.push(RenderCommand::UploadTexelData {
            texels: Arc::new(self.create_texels()),
            location: self.tile_location,
        });
    }
}

// Solid color allocation

struct SolidColorTileBuilder {
    tiles: Vec<SolidColorTile>,
}

struct SolidColorTile {
    texels: Vec<ColorU>,
    location: TextureLocation,
    next_index: u32,
}

impl SolidColorTileBuilder {
    fn new() -> SolidColorTileBuilder {
        SolidColorTileBuilder { tiles: vec![] }
    }

    fn allocate(&mut self, allocator: &mut TextureAllocator, color: ColorU) -> TextureLocation {
        if self.tiles.is_empty() ||
                self.tiles.last().unwrap().next_index == MAX_SOLID_COLORS_PER_TILE {
            let area = SOLID_COLOR_TILE_LENGTH as usize * SOLID_COLOR_TILE_LENGTH as usize;
            self.tiles.push(SolidColorTile {
                texels: vec![ColorU::black(); area],
                location: allocator.allocate(Vector2I::splat(SOLID_COLOR_TILE_LENGTH as i32),
                                             AllocationMode::Atlas),
                next_index: 0,
            });
        }

        let mut data = self.tiles.last_mut().unwrap();
        let subtile_origin = vec2i((data.next_index % SOLID_COLOR_TILE_LENGTH) as i32,
                                   (data.next_index / SOLID_COLOR_TILE_LENGTH) as i32);
        data.next_index += 1;

        let location = TextureLocation {
            page: data.location.page,
            rect: RectI::new(data.location.rect.origin() + subtile_origin, vec2i(1, 1)),
        };

        data.texels[subtile_origin.y() as usize * SOLID_COLOR_TILE_LENGTH as usize +
                    subtile_origin.x() as usize] = color;

        location
    }

    fn create_render_commands(self, render_commands: &mut Vec<RenderCommand>) {
        for tile in self.tiles {
            render_commands.push(RenderCommand::UploadTexelData {
                texels: Arc::new(tile.texels),
                location: tile.location,
            });
        }
    }
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
