// pathfinder/renderer/src/paint.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::allocator::{AllocationMode, TextureAllocator, TextureLocation};
use crate::gpu_data::{TextureData, TexturePageContents, TexturePageData, TexturePageId};
use crate::scene::RenderTarget;
use crate::tiles::{TILE_HEIGHT, TILE_WIDTH};
use hashbrown::HashMap;
use pathfinder_color::ColorU;
use pathfinder_content::gradient::{Gradient, GradientGeometry};
use pathfinder_content::pattern::{Image, Pattern, PatternFlags, PatternSource};
use pathfinder_content::render_target::RenderTargetId;
use pathfinder_geometry::rect::{RectF, RectI};
use pathfinder_geometry::transform2d::{Matrix2x2F, Transform2F};
use pathfinder_geometry::util;
use pathfinder_geometry::vector::{Vector2F, Vector2I};
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
                match *gradient.geometry_mut() {
                    GradientGeometry::Linear(ref mut line) => {
                        *line = *transform * *line;
                    }
                    GradientGeometry::Radial {
                        ref mut line,
                        ref mut start_radius,
                        ref mut end_radius,
                    } => {
                        *line = *transform * *line;

                        // FIXME(pcwalton): This is wrong; I think the transform can make the
                        // radial gradient into an ellipse.
                        *start_radius *= util::lerp(transform.matrix.m11(),
                                                    transform.matrix.m22(),
                                                    0.5);
                        *end_radius *= util::lerp(transform.matrix.m11(),
                                                  transform.matrix.m22(),
                                                  0.5);
                    }
                }
            }
            Paint::Pattern(_) => {
                // TODO(pcwalton): Implement this.
            }
        }
    }
}

pub struct PaintInfo {
    /// The data that is sent to the renderer.
    pub data: TextureData,
    /// The metadata for each paint.
    ///
    /// The indices of this vector are paint IDs.
    pub metadata: Vec<PaintMetadata>,
}

#[derive(Debug)]
pub struct PaintMetadata {
    /// The location of the texture.
    pub texture_page: TexturePageId,
    /// The rectangle within the texture atlas.
    pub texture_rect: RectI,
    /// The transform to apply to screen coordinates to translate them into UVs.
    pub texture_transform: Transform2F,
    /// The sampling mode for the texture.
    pub sampling_flags: TextureSamplingFlags,
    /// True if this paint is fully opaque.
    pub is_opaque: bool,
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

    pub fn build_paint_info(&self, view_box_size: Vector2I) -> PaintInfo {
        let mut allocator = TextureAllocator::new();
        let mut metadata = vec![];

        // Assign render target locations.
        let mut render_target_locations = vec![];
        for (render_target_index, render_target) in self.render_targets.iter().enumerate() {
            let render_target_id = RenderTargetId(render_target_index as u32);
            render_target_locations.push(allocator.allocate_render_target(render_target.size(),
                                                                          render_target_id));
        }

        // Assign paint locations.
        let mut solid_color_tile_builder = SolidColorTileBuilder::new();
        for paint in &self.paints {
            let (texture_location, mut sampling_flags);
            match paint {
                Paint::Color(_) => {
                    texture_location = solid_color_tile_builder.allocate(&mut allocator);
                    sampling_flags = TextureSamplingFlags::empty();
                }
                Paint::Gradient(_) => {
                    // TODO(pcwalton): Optimize this:
                    // 1. Use repeating/clamp on the sides.
                    // 2. Choose an optimal size for the gradient that minimizes memory usage while
                    //    retaining quality.
                    texture_location = allocator.allocate(Vector2I::splat(GRADIENT_TILE_LENGTH as i32),
                                                      AllocationMode::Atlas);
                    sampling_flags = TextureSamplingFlags::empty();
                }
                Paint::Pattern(ref pattern) => {
                    match pattern.source {
                        PatternSource::RenderTarget(render_target_id) => {
                            texture_location =
                                render_target_locations[render_target_id.0 as usize];
                        }
                        PatternSource::Image(ref image) => {
                            // TODO(pcwalton): We should be able to use tile cleverness to repeat
                            // inside the atlas in some cases.
                            let allocation_mode = if pattern.flags == PatternFlags::empty() {
                                AllocationMode::Atlas
                            } else {
                                AllocationMode::OwnPage
                            };

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
                }
            };

            metadata.push(PaintMetadata {
                texture_page: texture_location.page,
                texture_rect: texture_location.rect,
                texture_transform: Transform2F::default(),
                sampling_flags,
                is_opaque: paint.is_opaque(),
            });
        }

        // Calculate texture transforms.
        for (paint, metadata) in self.paints.iter().zip(metadata.iter_mut()) {
            let texture_scale = allocator.page_scale(metadata.texture_page);
            metadata.texture_transform = match paint {
                Paint::Color(_) => {
                    let vector = rect_to_inset_uv(metadata.texture_rect, texture_scale).origin();
                    Transform2F { matrix: Matrix2x2F(F32x4::default()), vector }
                }
                Paint::Gradient(_) => {
                    let texture_origin_uv =
                        rect_to_uv(metadata.texture_rect, texture_scale).origin();
                    let gradient_tile_scale = texture_scale.scale(GRADIENT_TILE_LENGTH as f32);
                    Transform2F::from_translation(texture_origin_uv) *
                        Transform2F::from_scale(gradient_tile_scale / view_box_size.to_f32())
                }
                Paint::Pattern(Pattern { source: PatternSource::Image(_), .. }) => {
                    let texture_origin_uv =
                        rect_to_uv(metadata.texture_rect, texture_scale).origin();
                    Transform2F::from_translation(texture_origin_uv) *
                        Transform2F::from_scale(texture_scale)
                }
                Paint::Pattern(Pattern { source: PatternSource::RenderTarget(_), .. }) => {
                    // FIXME(pcwalton): Only do this in GL, not Metal!
                    let texture_origin_uv = rect_to_uv(metadata.texture_rect,
                                                       texture_scale).lower_left();
                    Transform2F::from_translation(texture_origin_uv) *
                        Transform2F::from_scale(texture_scale.scale_xy(Vector2F::new(1.0, -1.0)))
                }
            }
        }

        // Render the actual texels.
        //
        // TODO(pcwalton): This is slow. Do more on GPU.
        let mut texture_data = TextureData { pages: vec![] };
        for page_index in 0..allocator.page_count() {
            let page_index = TexturePageId(page_index);
            let page_size = allocator.page_size(page_index);
            if let Some(render_target_id) = allocator.page_render_target_id(page_index) {
                texture_data.pages.push(TexturePageData {
                    size: page_size,
                    contents: TexturePageContents::RenderTarget(render_target_id),
                });
                continue;
            }

            let page_area = page_size.x() as usize * page_size.y() as usize;
            let texels = vec![ColorU::default(); page_area];
            texture_data.pages.push(TexturePageData {
                size: page_size,
                contents: TexturePageContents::Texels(texels),
            });
        }

        for (paint, metadata) in self.paints.iter().zip(metadata.iter()) {
            let texture_page = metadata.texture_page;
            let paint_page_data = &mut texture_data.pages[texture_page.0 as usize];
            let page_size = paint_page_data.size;
            let page_scale = allocator.page_scale(texture_page);

            match paint_page_data.contents {
                TexturePageContents::Texels(ref mut texels) => {
                    match paint {
                        Paint::Color(color) => {
                            put_pixel(metadata.texture_rect.origin(), *color, texels, page_size);
                        }
                        Paint::Gradient(ref gradient) => {
                            self.render_gradient(gradient,
                                                 metadata.texture_rect,
                                                 &metadata.texture_transform,
                                                 texels,
                                                 page_size,
                                                 page_scale);
                        }
                        Paint::Pattern(ref pattern) => {
                            match pattern.source {
                                PatternSource::RenderTarget(_) => {}
                                PatternSource::Image(ref image) => {
                                    self.render_image(image,
                                                      metadata.texture_rect,
                                                      texels,
                                                      page_size);
                                }
                            }
                        }
                    }
                }
                TexturePageContents::RenderTarget(_) => {}
            }
        }

        return PaintInfo { data: texture_data, metadata };
    }

    // TODO(pcwalton): This is slow. Do on GPU instead.
    fn render_gradient(&self,
                       gradient: &Gradient,
                       tex_rect: RectI,
                       tex_transform: &Transform2F,
                       texels: &mut [ColorU],
                       tex_size: Vector2I,
                       tex_scale: Vector2F) {
        match *gradient.geometry() {
            GradientGeometry::Linear(gradient_line) => {
                // FIXME(pcwalton): Paint transparent if gradient line has zero size, per spec.
                let gradient_line = *tex_transform * gradient_line;

                // TODO(pcwalton): Optimize this:
                // 1. Calculate ∇t up front and use differencing in the inner loop.
                // 2. Go four pixels at a time with SIMD.
                for y in 0..(GRADIENT_TILE_LENGTH as i32) {
                    for x in 0..(GRADIENT_TILE_LENGTH as i32) {
                        let point = tex_rect.origin() + Vector2I::new(x, y);
                        let vector = point.to_f32().scale_xy(tex_scale) - gradient_line.from();

                        let mut t = gradient_line.vector().projection_coefficient(vector);
                        t = util::clamp(t, 0.0, 1.0);

                        put_pixel(point, gradient.sample(t), texels, tex_size);
                    }
                }
            }

            GradientGeometry::Radial { line, start_radius: r0, end_radius: r1 } => {
                // FIXME(pcwalton): Paint transparent if line has zero size and radii are equal,
                // per spec.
                let line = *tex_transform * line;

                // This is based on Pixman (MIT license). Copy and pasting the excellent comment
                // from there:

                // Implementation of radial gradients following the PDF specification.
                // See section 8.7.4.5.4 Type 3 (Radial) Shadings of the PDF Reference
                // Manual (PDF 32000-1:2008 at the time of this writing).
                //
                // In the radial gradient problem we are given two circles (c₁,r₁) and
                // (c₂,r₂) that define the gradient itself.
                //
                // Mathematically the gradient can be defined as the family of circles
                //
                //     ((1-t)·c₁ + t·(c₂), (1-t)·r₁ + t·r₂)
                //
                // excluding those circles whose radius would be < 0. When a point
                // belongs to more than one circle, the one with a bigger t is the only
                // one that contributes to its color. When a point does not belong
                // to any of the circles, it is transparent black, i.e. RGBA (0, 0, 0, 0).
                // Further limitations on the range of values for t are imposed when
                // the gradient is not repeated, namely t must belong to [0,1].
                //
                // The graphical result is the same as drawing the valid (radius > 0)
                // circles with increasing t in [-inf, +inf] (or in [0,1] if the gradient
                // is not repeated) using SOURCE operator composition.
                //
                // It looks like a cone pointing towards the viewer if the ending circle
                // is smaller than the starting one, a cone pointing inside the page if
                // the starting circle is the smaller one and like a cylinder if they
                // have the same radius.
                //
                // What we actually do is, given the point whose color we are interested
                // in, compute the t values for that point, solving for t in:
                //
                //     length((1-t)·c₁ + t·(c₂) - p) = (1-t)·r₁ + t·r₂
                //
                // Let's rewrite it in a simpler way, by defining some auxiliary
                // variables:
                //
                //     cd = c₂ - c₁
                //     pd = p - c₁
                //     dr = r₂ - r₁
                //     length(t·cd - pd) = r₁ + t·dr
                //
                // which actually means
                //
                //     hypot(t·cdx - pdx, t·cdy - pdy) = r₁ + t·dr
                //
                // or
                //
                //     ⎷((t·cdx - pdx)² + (t·cdy - pdy)²) = r₁ + t·dr.
                //
                // If we impose (as stated earlier) that r₁ + t·dr >= 0, it becomes:
                //
                //     (t·cdx - pdx)² + (t·cdy - pdy)² = (r₁ + t·dr)²
                //
                // where we can actually expand the squares and solve for t:
                //
                //     t²cdx² - 2t·cdx·pdx + pdx² + t²cdy² - 2t·cdy·pdy + pdy² =
                //       = r₁² + 2·r₁·t·dr + t²·dr²
                //
                //     (cdx² + cdy² - dr²)t² - 2(cdx·pdx + cdy·pdy + r₁·dr)t +
                //         (pdx² + pdy² - r₁²) = 0
                //
                //     A = cdx² + cdy² - dr²
                //     B = pdx·cdx + pdy·cdy + r₁·dr
                //     C = pdx² + pdy² - r₁²
                //     At² - 2Bt + C = 0
                //
                // The solutions (unless the equation degenerates because of A = 0) are:
                //
                //     t = (B ± ⎷(B² - A·C)) / A
                //
                // The solution we are going to prefer is the bigger one, unless the
                // radius associated to it is negative (or it falls outside the valid t
                // range).
                //
                // Additional observations (useful for optimizations):
                // A does not depend on p
                //
                // A < 0 <=> one of the two circles completely contains the other one
                //   <=> for every p, the radiuses associated with the two t solutions
                //       have opposite sign

                let cd = line.vector();
                let dr = r1 - r0;
                let a = cd.square_length() - dr * dr;
                let a_inv = 1.0 / a;

                for y in 0..(GRADIENT_TILE_LENGTH as i32) {
                    for x in 0..(GRADIENT_TILE_LENGTH as i32) {
                        let point = tex_rect.origin() + Vector2I::new(x, y);
                        let point_f = point.to_f32();
                        let pd = point_f - line.from();

                        let b = pd.dot(cd) + r0 * dr;
                        let c = pd.square_length() - r0 * r0;
                        let discrim = b * b - a * c;

                        let mut color = ColorU::transparent_black();
                        if !util::approx_eq(discrim, 0.0) {
                            let discrim_sqrt = f32::sqrt(discrim);
                            let discrim_sqrts = F32x2::new(discrim_sqrt, -discrim_sqrt);
                            let ts = (discrim_sqrts + F32x2::splat(b)) * F32x2::splat(a_inv);
                            let t_min = f32::min(ts.x(), ts.y());
                            let t_max = f32::max(ts.x(), ts.y());
                            let t = if t_max <= 1.0 { t_max } else { t_min };
                            if t >= 0.0 {
                                color = gradient.sample(t);
                            }
                        };

                        put_pixel(point, color, texels, tex_size);
                    }
                }
            }
        }
    }

    fn render_image(&self,
                    image: &Image,
                    tex_rect: RectI,
                    texels: &mut [ColorU],
                    tex_size: Vector2I) {
        let image_size = image.size();
        for y in 0..image_size.y() {
            let dest_origin = tex_rect.origin() + Vector2I::new(0, y);
            let dest_start_index = paint_texel_index(dest_origin, tex_size);
            let src_start_index = y as usize * image_size.x() as usize;
            let dest_end_index = dest_start_index + image_size.x() as usize;
            let src_end_index = src_start_index + image_size.x() as usize;
            texels[dest_start_index..dest_end_index].copy_from_slice(
                &image.pixels()[src_start_index..src_end_index]);
        }
    }
}

impl PaintMetadata {
    // TODO(pcwalton): Apply clamp/repeat to tile rect.
    pub(crate) fn calculate_tex_coords(&self,
                                       tile_position: Vector2I,
                                       path_transform_inv: Transform2F)
                                       -> Vector2F {
        let tile_size = Vector2I::new(TILE_WIDTH as i32, TILE_HEIGHT as i32);
        let position = tile_position.scale_xy(tile_size).to_f32();
        let tex_coords = self.texture_transform * path_transform_inv * position;
        tex_coords
    }
}

fn paint_texel_index(position: Vector2I, tex_size: Vector2I) -> usize {
    position.y() as usize * tex_size.x() as usize + position.x() as usize
}

fn put_pixel(position: Vector2I, color: ColorU, texels: &mut [ColorU], tex_size: Vector2I) {
    texels[paint_texel_index(position, tex_size)] = color
}

fn rect_to_uv(rect: RectI, texture_scale: Vector2F) -> RectF {
    rect.to_f32().scale_xy(texture_scale)
}

fn rect_to_inset_uv(rect: RectI, texture_scale: Vector2F) -> RectF {
    rect_to_uv(rect, texture_scale).contract(texture_scale.scale(0.5))
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
            let subtile_origin = Vector2I::new((data.next_index % SOLID_COLOR_TILE_LENGTH) as i32,
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
