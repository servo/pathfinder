// pathfinder/content/src/gradient.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::sorted_vector::SortedVector;
use crate::util;
use pathfinder_color::ColorU;
use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_geometry::vector::Vector2F;
use pathfinder_geometry::util as geometry_util;
use pathfinder_simd::default::F32x2;
use std::cmp::{Ordering, PartialOrd};
use std::convert;
use std::hash::{Hash, Hasher};
use std::mem;

#[derive(Clone, PartialEq, Debug)]
pub struct Gradient {
    /// The line this gradient runs along.
    /// 
    /// If this is a radial gradient, this is the line that connects the two circles. It may have
    /// zero-length in the case of simple radial gradients.
    pub line: LineSegment2F,
    /// For radial gradients, the radii of the start and endpoints respectively. If this is a
    /// linear gradient, this is `None`.
    pub radii: Option<F32x2>,
    stops: SortedVector<ColorStop>,
}

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct ColorStop {
    pub offset: f32,
    pub color: ColorU,
}

impl Eq for Gradient {}

impl Hash for Gradient {
    fn hash<H>(&self, state: &mut H) where H: Hasher {
        util::hash_line_segment(self.line, state);
        match self.radii {
            None => (0).hash(state),
            Some(radii) => {
                (1).hash(state);
                util::hash_f32(radii.x(), state);
                util::hash_f32(radii.y(), state);
            }
        }
        self.stops.hash(state);
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
    pub fn linear(line: LineSegment2F) -> Gradient {
        Gradient { line, radii: None, stops: SortedVector::new() }
    }

    #[inline]
    pub fn linear_from_points(from: Vector2F, to: Vector2F) -> Gradient {
        Gradient::linear(LineSegment2F::new(from, to))
    }

    #[inline]
    pub fn radial<L>(line: L, radii: F32x2) -> Gradient where L: RadialGradientLine {
        Gradient { line: line.to_line(), radii: Some(radii), stops: SortedVector::new() }
    }

    #[inline]
    pub fn add(&mut self, stop: ColorStop) {
        self.stops.push(stop);
    }

    /// A convenience method to add a color stop.
    #[inline]
    pub fn add_color_stop(&mut self, color: ColorU, offset: f32) {
        self.add(ColorStop::new(color, offset))
    }

    #[inline]
    pub fn line(&self) -> LineSegment2F {
        self.line
    }

    #[inline]
    pub fn set_line(&mut self, line: LineSegment2F) {
        self.line = line
    }

    #[inline]
    pub fn radii(&self) -> Option<F32x2> {
        self.radii
    }

    #[inline]
    pub fn set_radii(&mut self, radii: Option<F32x2>) {
        self.radii = radii
    }

    #[inline]
    pub fn stops(&self) -> &[ColorStop] {
        &self.stops.array
    }

    #[inline]
    pub fn stops_mut(&mut self) -> &mut [ColorStop] {
        &mut self.stops.array
    }

    pub fn sample(&self, mut t: f32) -> ColorU {
        if self.stops.is_empty() {
            return ColorU::transparent_black();
        }

        t = geometry_util::clamp(t, 0.0, 1.0);
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

    #[inline]
    pub fn is_opaque(&self) -> bool {
        self.stops.array.iter().all(|stop| stop.color.is_opaque())
    }

    #[inline]
    pub fn is_fully_transparent(&self) -> bool {
        self.stops.array.iter().all(|stop| stop.color.is_fully_transparent())
    }
}

impl ColorStop {
    #[inline]
    pub fn new(color: ColorU, offset: f32) -> ColorStop {
        ColorStop { color, offset }
    }
}

pub trait RadialGradientLine {
    fn to_line(self) -> LineSegment2F;
}

impl RadialGradientLine for LineSegment2F {
    #[inline]
    fn to_line(self) -> LineSegment2F {
        self
    }
}

impl RadialGradientLine for Vector2F {
    #[inline]
    fn to_line(self) -> LineSegment2F {
        LineSegment2F::new(self, self)
    }
}
