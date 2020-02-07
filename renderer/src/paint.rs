// pathfinder/renderer/src/paint.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::allocator::{TextureAllocator, TextureLocation};
use crate::gpu_data::PaintData;
use crate::tiles::{TILE_HEIGHT, TILE_WIDTH};
use hashbrown::HashMap;
use pathfinder_color::ColorU;
use pathfinder_content::gradient::Gradient;
use pathfinder_geometry::rect::{RectF, RectI};
use pathfinder_geometry::transform2d::{Matrix2x2F, Transform2F};
use pathfinder_geometry::util;
use pathfinder_geometry::vector::{Vector2F, Vector2I};
use pathfinder_simd::default::F32x4;
use std::fmt::{self, Debug, Formatter};

const PAINT_TEXTURE_LENGTH: u32 = 1024;
const PAINT_TEXTURE_SCALE: f32 = 1.0 / PAINT_TEXTURE_LENGTH as f32;

// The size of a gradient tile.
//
// TODO(pcwalton): Choose this size dynamically!
const GRADIENT_TILE_LENGTH: u32 = 256;
const GRADIENT_TILE_SCALE: f32 = GRADIENT_TILE_LENGTH as f32 * PAINT_TEXTURE_SCALE;

const SOLID_COLOR_TILE_LENGTH: u32 = 16;
const MAX_SOLID_COLORS_PER_TILE: u32 = SOLID_COLOR_TILE_LENGTH * SOLID_COLOR_TILE_LENGTH;

#[derive(Clone)]
pub struct Palette {
    pub(crate) paints: Vec<Paint>,
    cache: HashMap<Paint, PaintId>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Paint {
    Color(ColorU),
    Gradient(Gradient),
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
        }
    }
}

impl Palette {
    #[inline]
    pub fn new() -> Palette {
        Palette { paints: vec![], cache: HashMap::new() }
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
        }
    }

    pub fn is_fully_transparent(&self) -> bool {
        match *self {
            Paint::Color(color) => color.is_opaque(),
            Paint::Gradient(ref gradient) => {
                gradient.stops().iter().all(|stop| stop.color.is_fully_transparent())
            }
        }
    }

    pub fn set_opacity(&mut self, alpha: f32) {
        if alpha == 1.0 {
            return;
        }

        match *self {
            Paint::Color(ref mut color) => color.a = (color.a as f32 * alpha).round() as u8,
            Paint::Gradient(ref mut gradient) => gradient.set_opacity(alpha),
        }
    }
}

pub struct PaintInfo {
    /// The data that is sent to the renderer.
    pub data: PaintData,
    /// The metadata for each paint.
    ///
    /// The indices of this vector are paint IDs.
    pub metadata: Vec<PaintMetadata>,
}

// TODO(pcwalton): Add clamp/repeat options.
#[derive(Debug)]
pub struct PaintMetadata {
    /// The rectangle within the texture atlas.
    pub tex_rect: RectI,
    /// The transform to apply to screen coordinates to translate them into UVs.
    pub tex_transform: Transform2F,
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

    pub fn build_paint_info(&self, view_box_size: Vector2I) -> PaintInfo {
        let mut allocator = TextureAllocator::new(PAINT_TEXTURE_LENGTH);
        let area = PAINT_TEXTURE_LENGTH as usize * PAINT_TEXTURE_LENGTH as usize;
        let (mut texels, mut metadata) = (vec![0; area * 4], vec![]);

        let mut solid_color_tile_builder = SolidColorTileBuilder::new();

        for paint in &self.paints {
            let (texture_location, tex_transform);
            match paint {
                Paint::Color(color) => {
                    texture_location = solid_color_tile_builder.allocate(&mut allocator);
                    let vector = rect_to_inset_uv(texture_location.rect).origin();
                    tex_transform = Transform2F { matrix: Matrix2x2F(F32x4::default()), vector };
                    put_pixel(&mut texels, texture_location.rect.origin(), *color);
                }
                Paint::Gradient(ref gradient) => {
                    // TODO(pcwalton): Optimize this:
                    // 1. Use repeating/clamp on the sides.
                    // 2. Choose an optimal size for the gradient that minimizes memory usage while
                    //    retaining quality.
                    texture_location =
                        allocator.allocate(Vector2I::splat(GRADIENT_TILE_LENGTH as i32))
                                 .expect("Failed to allocate space for the gradient!");

                    tex_transform =
                        Transform2F::from_translation(rect_to_uv(texture_location.rect).origin()) *
                        Transform2F::from_scale(Vector2F::splat(GRADIENT_TILE_SCALE) /
                                                view_box_size.to_f32());

                    let gradient_line = tex_transform * gradient.line();

                    // TODO(pcwalton): Optimize this:
                    // 1. Calculate ∇t up front and use differencing in the inner loop.
                    // 2. Go four pixels at a time with SIMD.
                    for y in 0..(GRADIENT_TILE_LENGTH as i32) {
                        for x in 0..(GRADIENT_TILE_LENGTH as i32) {
                            let point = texture_location.rect.origin() + Vector2I::new(x, y);
                            let vector = point.to_f32().scale(1.0 / PAINT_TEXTURE_LENGTH as f32) -
                                gradient_line.from();

                            let mut t = gradient_line.vector().projection_coefficient(vector);
                            t = util::clamp(t, 0.0, 1.0);

                            put_pixel(&mut texels, point, gradient.sample(t));
                        }
                    }
                }
            }

            metadata.push(PaintMetadata {
                tex_rect: texture_location.rect,
                tex_transform,
                is_opaque: paint.is_opaque(),
            });
        }

        let size = Vector2I::splat(PAINT_TEXTURE_LENGTH as i32);
        return PaintInfo { data: PaintData { size, texels }, metadata };

        fn put_pixel(texels: &mut [u8], position: Vector2I, color: ColorU) {
            let index = (position.y() as usize * PAINT_TEXTURE_LENGTH as usize +
                         position.x() as usize) * 4;
            texels[index + 0] = color.r;
            texels[index + 1] = color.g;
            texels[index + 2] = color.b;
            texels[index + 3] = color.a;
        }

        fn rect_to_uv(rect: RectI) -> RectF {
            rect.to_f32().scale(1.0 / PAINT_TEXTURE_LENGTH as f32)
        }

        fn rect_to_inset_uv(rect: RectI) -> RectF {
            rect_to_uv(rect).contract(Vector2F::splat(0.5 / PAINT_TEXTURE_LENGTH as f32))
        }
    }
}

impl PaintMetadata {
    // TODO(pcwalton): Apply clamp/repeat to tile rect.
    pub(crate) fn calculate_tex_coords(&self, tile_position: Vector2I) -> Vector2F {
        let tile_size = Vector2I::new(TILE_WIDTH as i32, TILE_HEIGHT as i32);
        let tex_coords = self.tex_transform * tile_position.scale_xy(tile_size).to_f32();
        tex_coords
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
                tile_location: allocator.allocate(Vector2I::splat(SOLID_COLOR_TILE_LENGTH as i32))
                                        .expect("Failed to allocate a solid color tile!"),
                next_index: 0,
            });
        }

        let (location, tile_full);
        {
            let mut data = self.0.as_mut().unwrap();
            let subtile_origin = Vector2I::new((data.next_index % SOLID_COLOR_TILE_LENGTH) as i32,
                                               (data.next_index / SOLID_COLOR_TILE_LENGTH) as i32);
            location = TextureLocation {
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
