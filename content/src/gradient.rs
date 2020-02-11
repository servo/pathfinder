// pathfinder/geometry/src/gradient.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::sorted_vector::SortedVector;
use pathfinder_color::ColorU;
use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_simd::default::F32x4;
use std::cmp::{Ordering, PartialOrd};
use std::convert;
use std::hash::{Hash, Hasher};
use std::mem;

#[derive(Clone, PartialEq, Debug)]
pub struct Gradient {
    geometry: GradientGeometry,
    stops: SortedVector<ColorStop>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GradientGeometry {
    Linear(LineSegment2F),
    Radial {
        line: LineSegment2F,
        start_radius: f32,
        end_radius: f32,
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct ColorStop {
    pub offset: f32,
    pub color: ColorU,
}

impl Eq for Gradient {}

impl Hash for Gradient {
    fn hash<H>(&self, state: &mut H) where H: Hasher {
        match self.geometry {
            GradientGeometry::Linear(line) => {
                (0).hash(state);
                hash_line_segment(line, state);
            }
            GradientGeometry::Radial { line, start_radius, end_radius } => {
                (1).hash(state);
                hash_line_segment(line, state);
                hash_f32(start_radius, state);
                hash_f32(end_radius, state);
            }
        }
        self.stops.hash(state);

        fn hash_line_segment<H>(line_segment: LineSegment2F, state: &mut H) where H: Hasher {
            unsafe {
                let data: [u32; 4] = mem::transmute::<F32x4, [u32; 4]>(line_segment.0);
                data.hash(state);
            }
        }

        fn hash_f32<H>(value: f32, state: &mut H) where H: Hasher {
            unsafe {
                let data: u32 = mem::transmute::<f32, u32>(value);
                data.hash(state);
            }
        }
    }
}

impl Eq for ColorStop {}

impl Hash for ColorStop {
    fn hash<H>(&self, state: &mut H) where H: Hasher {
        unsafe {
            self.color.hash(state);
            let offset = mem::transmute::<f32, u32>(self.offset);
            offset.hash(state);
        }
    }
}

impl Gradient {
    #[inline]
    pub fn new(geometry: GradientGeometry) -> Gradient {
        Gradient { geometry, stops: SortedVector::new() }
    }

    #[inline]
    pub fn linear(line: LineSegment2F) -> Gradient {
        Gradient::new(GradientGeometry::Linear(line))
    }

    #[inline]
    pub fn radial(line: LineSegment2F, start_radius: f32, end_radius: f32) -> Gradient {
        Gradient::new(GradientGeometry::Radial { line, start_radius, end_radius })
    }

    #[inline]
    pub fn add_color_stop(&mut self, stop: ColorStop) {
        self.stops.push(stop);
    }

    #[inline]
    pub fn geometry(&self) -> &GradientGeometry {
        &self.geometry
    }

    #[inline]
    pub fn stops(&self) -> &[ColorStop] {
        &self.stops.array
    }

    pub fn sample(&self, t: f32) -> ColorU {
        if self.stops.is_empty() {
            return ColorU::transparent_black();
        }

        let last_index = self.stops.len() - 1;
        let upper_index = self.stops.binary_search_by(|stop| {
            stop.offset.partial_cmp(&t).unwrap_or(Ordering::Less)
        }).unwrap_or_else(convert::identity).min(last_index);
        let lower_index = if upper_index > 0 { upper_index - 1 } else { upper_index };

        let lower_stop = &self.stops.array[lower_index];
        let upper_stop = &self.stops.array[upper_index];

        let denom = upper_stop.offset - lower_stop.offset;
        if denom == 0.0 {
            return lower_stop.color;
        }

        lower_stop.color
                  .to_f32()
                  .lerp(upper_stop.color.to_f32(), (t - lower_stop.offset) / denom)
                  .to_u8()
    }

    pub fn set_opacity(&mut self, alpha: f32) {
        for stop in &mut self.stops.array {
            stop.color.a = (stop.color.a as f32 * alpha).round() as u8;
        }
    }
}

impl ColorStop {
    #[inline]
    pub fn new(color: ColorU, offset: f32) -> ColorStop {
        ColorStop { color, offset }
    }
}
