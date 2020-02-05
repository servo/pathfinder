// pathfinder/renderer/src/paint.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::allocator::{TextureAllocator, TextureLocation};
use crate::gpu_data::PaintData;
use crate::scene::Scene;
use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::transform2d::{Matrix2x2I, Transform2I};
use pathfinder_geometry::vector::Vector2I;
use pathfinder_simd::default::I32x4;

const PAINT_TEXTURE_LENGTH: u32 = 1024;
const PAINT_TEXTURE_SCALE: u32 = 65536 / PAINT_TEXTURE_LENGTH;

const SOLID_COLOR_TILE_LENGTH: u32 = 16;
const MAX_SOLID_COLORS_PER_TILE: u32 = SOLID_COLOR_TILE_LENGTH * SOLID_COLOR_TILE_LENGTH;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Paint {
    pub color: ColorU,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PaintId(pub u16);

impl Paint {
    #[inline]
    pub fn is_opaque(&self) -> bool {
        self.color.a == 255
    }

    #[inline]
    pub fn is_fully_transparent(&self) -> bool {
        self.color.is_fully_transparent()
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

#[derive(Debug)]
pub struct PaintMetadata {
    /// The transform to apply to the texture coordinates, in 0.16 fixed point.
    pub tex_transform: Transform2I,
    /// True if this paint is fully opaque.
    pub is_opaque: bool,
}

impl Scene {
    pub fn build_paint_info(&self) -> PaintInfo {
        let mut allocator = TextureAllocator::new(PAINT_TEXTURE_LENGTH);
        let area = PAINT_TEXTURE_LENGTH as usize * PAINT_TEXTURE_LENGTH as usize;
        let (mut texels, mut metadata) = (vec![0; area * 4], vec![]);
        let mut solid_color_tile_builder = SolidColorTileBuilder::new();

        for paint in &self.paints {
            // TODO(pcwalton): Handle other paint types.
            let texture_location = solid_color_tile_builder.allocate(&mut allocator);
            put_pixel(&mut texels, texture_location.rect.origin(), paint.color);
            let tex_transform = Transform2I {
                matrix: Matrix2x2I(I32x4::default()),
                vector: texture_location.rect.origin().scale(PAINT_TEXTURE_SCALE as i32) +
                    Vector2I::splat(PAINT_TEXTURE_SCALE as i32 / 2),
            };
            metadata.push(PaintMetadata { tex_transform, is_opaque: paint.is_opaque() });
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
    }
}

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
