// pathfinder/renderer/src/paint.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::gpu_data::PaintData;
use crate::scene::Scene;
use crate::sorted_vector::SortedVector;
use indexmap::IndexSet;
use pathfinder_geometry::basic::line_segment::LineSegmentF32;
use pathfinder_geometry::basic::point::Point2DI32;
use pathfinder_geometry::basic::rect::RectI32;
use pathfinder_geometry::color::ColorU;

const PAINT_TEXTURE_WIDTH: i32 = 256;
const PAINT_TEXTURE_HEIGHT: i32 = 256;

const PAINT_TEXTURE_U_PER_TEXEL: i32 = 65536 / PAINT_TEXTURE_WIDTH;
const PAINT_TEXTURE_V_PER_TEXEL: i32 = 65536 / PAINT_TEXTURE_HEIGHT;

#[derive(Clone)]
pub(crate) struct Palette {
    pub(crate) colors: IndexSet<ColorU>,
    pub(crate) linear_gradients: IndexSet<LinearGradient>,
}

pub(crate) struct PaletteTexCoords {
    pub(crate) colors: Vec<Point2DI32>,
    pub(crate) linear_gradients: Vec<i32>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ColorId(u32);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LinearGradientId(u32);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Paint {
    Color(ColorId),
    LinearGradient {
        id: LinearGradientId,
        line: LineSegmentF32,
    },
}

// In 0.16-bit fixed point.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PaintTexCoords {
    pub origin: Point2DI32,
    pub gradient: Point2DI32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LinearGradient {
    pub stops: SortedVector<GradientStop>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Hash)]
pub struct GradientStop {
    // 16-bit normalized fixed point between [0, 1].
    pub distance: u16,
    // A monotonically increasing ID.
    pub id: u16,
    // The color.
    pub color: ColorU,
}

impl Palette {
    #[inline]
    pub(crate) fn new() -> Palette {
        Palette { colors: IndexSet::new(), linear_gradients: IndexSet::new() }
    }

    #[inline]
    pub(crate) fn add_color(&mut self, color: ColorU) -> ColorId {
        ColorId(self.colors.insert_full(color).0 as u32)
    }

    #[inline]
    pub(crate) fn add_linear_gradient(&mut self, gradient: &LinearGradient) -> LinearGradientId {
        if let Some((gradient_id, _)) = self.linear_gradients.get_full(gradient) {
            return LinearGradientId(gradient_id as u32);
        }

        LinearGradientId(self.linear_gradients.insert_full((*gradient).clone()).0 as u32)
    }

    #[inline]
    pub(crate) fn get_color(&self, color_id: ColorId) -> Option<ColorU> {
        self.colors.get_index(color_id.0 as usize).map(|color| *color)
    }

    #[inline]
    pub(crate) fn get_linear_gradient(&self, gradient_id: LinearGradientId)
                                      -> Option<&LinearGradient> {
        self.linear_gradients.get_index(gradient_id.0 as usize)
    }

    #[inline]
    pub(crate) fn paint_is_opaque(&self, paint: &Paint) -> bool {
        match *paint {
            Paint::Color(color_id) => self.get_color(color_id).unwrap().is_opaque(),
            Paint::LinearGradient { id: gradient_id, .. } => {
                let gradient = self.get_linear_gradient(gradient_id).unwrap();
                gradient.stops.array.iter().all(|stop| stop.color.is_opaque())
            }
        }
    }
}

impl LinearGradient {
    #[inline]
    pub fn new() -> LinearGradient {
        LinearGradient { stops: SortedVector::new() }
    }

    #[inline]
    pub fn add_color_stop(&mut self, offset: f32, color: ColorU) {
        debug_assert!(offset >= 0.0 && offset <= 1.0);
        let distance = f32::round(offset * 65535.0) as u16;
        let id = self.stops.len() as u16;
        self.stops.push(GradientStop { distance, id, color });
    }
}

impl Palette {
    pub(crate) fn build_tex_coords(&self) -> PaletteTexCoords {
        // Allocate linear gradients.
        let mut linear_gradient_tex_coords = vec![];
        let mut next_y = 0;
        for linear_gradient in &self.linear_gradients {
            linear_gradient_tex_coords.push(next_y);
            next_y += 1;
        }

        // Allocate colors.
        let mut color_tex_coords = vec![];
        let mut next_tex_coord = Point2DI32::new(0, next_y);
        for &color in &self.colors {
            color_tex_coords.push(next_tex_coord);
            next_tex_coord.set_x(next_tex_coord.x() + 1);
            if next_tex_coord.x() >= PAINT_TEXTURE_WIDTH {
                next_tex_coord.set_x(0);
                next_tex_coord.set_y(next_tex_coord.y() + 1);
            }
        }

        PaletteTexCoords { colors: color_tex_coords, linear_gradients: linear_gradient_tex_coords }
    }
}

impl PaletteTexCoords {
    #[inline]
    pub(crate) fn new() -> PaletteTexCoords {
        PaletteTexCoords { colors: vec![], linear_gradients: vec![] }
    }

    pub(crate) fn build_paint_data(&self, palette: &Palette) -> PaintData {
        let size = Point2DI32::new(PAINT_TEXTURE_WIDTH, PAINT_TEXTURE_HEIGHT);
        let mut paint_data = PaintData {
            size,
            texels: vec![0; size.x() as usize * size.y() as usize * 4],
        };
        for (color_index, color) in palette.colors.iter().enumerate() {
            let tex_coords = &self.colors[color_index];
            paint_data.put_pixel(*tex_coords, *color);
        }
        for (gradient_index, gradient) in palette.linear_gradients.iter().enumerate() {
            // FIXME(pcwalton)
            let y = self.linear_gradients[gradient_index];
            let stop_count = gradient.stops.len();
            for x in 0..PAINT_TEXTURE_WIDTH {
                let color = gradient.stops.array[x as usize % stop_count].color;
                paint_data.put_pixel(Point2DI32::new(x, y), color);
            }
        }
        paint_data
    }

    #[inline]
    pub(crate) fn tex_coords(&self, paint: &Paint) -> PaintTexCoords {
        let scale = Point2DI32::new(PAINT_TEXTURE_U_PER_TEXEL, PAINT_TEXTURE_V_PER_TEXEL);
        let half_texel = Point2DI32::new(PAINT_TEXTURE_U_PER_TEXEL / 2,
                                         PAINT_TEXTURE_V_PER_TEXEL / 2);
        match *paint {
            Paint::Color(color_id) => {
                PaintTexCoords {
                    origin: self.colors[color_id.0 as usize].scale_xy(scale) + half_texel,
                    gradient: Point2DI32::default(),
                }
            }
            Paint::LinearGradient { id: gradient_id, .. } => {
                // FIXME(pcwalton): Take the line into account!
                let y = self.linear_gradients[gradient_id.0 as usize] * scale.y();
                PaintTexCoords {
                    origin: Point2DI32::new(0, y),
                    gradient: Point2DI32::new(PAINT_TEXTURE_U_PER_TEXEL, 0),
                }
            }
        }
    }

    #[inline]
    pub(crate) fn half_texel() -> Point2DI32 {
        Point2DI32::new(PAINT_TEXTURE_U_PER_TEXEL / 2, PAINT_TEXTURE_V_PER_TEXEL / 2)
    }
}

impl PaintData {
    fn put_pixel(&mut self, coords: Point2DI32, color: ColorU) {
        // FIXME(pcwalton): I'm sure this is slow.
        let width = PAINT_TEXTURE_WIDTH as usize;
        let offset = (coords.y() as usize * width + coords.x() as usize) * 4;
        self.texels[offset + 0] = color.r;
        self.texels[offset + 1] = color.g;
        self.texels[offset + 2] = color.b;
        self.texels[offset + 3] = color.a;
    }
}
