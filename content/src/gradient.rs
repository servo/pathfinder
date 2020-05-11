// pathfinder/content/src/gradient.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::util;
use pathfinder_color::ColorU;
use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::Vector2F;
use pathfinder_geometry::util as geometry_util;
use pathfinder_simd::default::F32x2;
use std::cmp::Ordering;
use std::convert;
use std::hash::{Hash, Hasher};
use std::mem;

#[derive(Clone, PartialEq, Debug)]
pub struct Gradient {
    pub geometry: GradientGeometry,
    stops: Vec<ColorStop>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ColorStop {
    pub offset: f32,
    pub color: ColorU,
}

#[derive(Clone, PartialEq, Debug)]
pub enum GradientGeometry {
    Linear(LineSegment2F),
    Radial {
        /// The line that connects the two circles. It may have zero length for simple radial
        /// gradients.
        line: LineSegment2F,
        /// The radii of the two circles. The first value may be zero.
        radii: F32x2,
        /// Transform from radial gradient space into screen space.
        ///
        /// Like `gradientTransform` in SVG. Note that this is the inverse of Cairo's gradient
        /// transform.
        transform: Transform2F,
    }
}

impl Eq for Gradient {}

impl Hash for Gradient {
    fn hash<H>(&self, state: &mut H) where H: Hasher {
        match self.geometry {
            GradientGeometry::Linear(line) => {
                (0).hash(state);
                util::hash_line_segment(line, state);
            }
            GradientGeometry::Radial { line, radii, transform } => {
                (1).hash(state);
                util::hash_line_segment(line, state);
                util::hash_f32(radii.x(), state);
                util::hash_f32(radii.y(), state);
                util::hash_f32(transform.m11(), state);
                util::hash_f32(transform.m12(), state);
                util::hash_f32(transform.m13(), state);
                util::hash_f32(transform.m21(), state);
                util::hash_f32(transform.m22(), state);
                util::hash_f32(transform.m23(), state);
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
        Gradient { geometry: GradientGeometry::Linear(line), stops: Vec::new() }
    }

    #[inline]
    pub fn linear_from_points(from: Vector2F, to: Vector2F) -> Gradient {
        Gradient::linear(LineSegment2F::new(from, to))
    }

    #[inline]
    pub fn radial<L>(line: L, radii: F32x2) -> Gradient where L: RadialGradientLine {
        let transform = Transform2F::default();
        Gradient {
            geometry: GradientGeometry::Radial { line: line.to_line(), radii, transform },
            stops: Vec::new(),
        }
    }

    #[inline]
    pub fn add(&mut self, stop: ColorStop) {
        let index = self.stops.binary_search_by(|other| {
            if other.offset <= stop.offset { Ordering::Less } else { Ordering::Greater }
        }).unwrap_or_else(convert::identity);
        self.stops.insert(index, stop);
    }

    /// A convenience method to add a color stop.
    #[inline]
    pub fn add_color_stop(&mut self, color: ColorU, offset: f32) {
        self.add(ColorStop::new(color, offset))
    }

    #[inline]
    pub fn stops(&self) -> &[ColorStop] {
        &self.stops
    }

    #[inline]
    pub fn stops_mut(&mut self) -> &mut [ColorStop] {
        &mut self.stops
    }

    pub fn sample(&self, mut t: f32) -> ColorU {
        if self.stops.is_empty() {
            return ColorU::transparent_black();
        }

        t = geometry_util::clamp(t, 0.0, 1.0);
        let last_index = self.stops.len() - 1;

        let upper_index = self.stops.binary_search_by(|stop| {
            if stop.offset < t || stop.offset == 0.0 { Ordering::Less } else { Ordering::Greater }
        }).unwrap_or_else(convert::identity).min(last_index);
        let lower_index = if upper_index > 0 { upper_index - 1 } else { upper_index };

        let lower_stop = &self.stops[lower_index];
        let upper_stop = &self.stops[upper_index];

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
        self.stops.iter().all(|stop| stop.color.is_opaque())
    }

    #[inline]
    pub fn is_fully_transparent(&self) -> bool {
        self.stops.iter().all(|stop| stop.color.is_fully_transparent())
    }

    pub fn apply_transform(&mut self, new_transform: Transform2F) {
        if new_transform.is_identity() {
            return;
        }

        match self.geometry {
            GradientGeometry::Linear(ref mut line) => *line = new_transform * *line,
            GradientGeometry::Radial { ref mut transform, .. } => {
                *transform = new_transform * *transform
            }
        }
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

#[cfg(test)]
mod test {
    use crate::gradient::Gradient;
    use pathfinder_color::ColorU;
    use pathfinder_geometry::vector::Vector2F;

    #[test]
    fn stable_order() {
        let mut grad = Gradient::linear_from_points(Vector2F::default(), Vector2F::default());
        for i in 0..110 {
            grad.add_color_stop(ColorU::new(i, 0, 0, 1), (i % 11) as f32 / 10.0);
        }

        // Check that it sorted stably
        assert!(grad.stops.windows(2).all(|w| {
            w[0].offset < w[1].offset || w[0].color.r < w[1].color.r
        }));
    }

    #[test]
    fn never_sample_zero_width() {
        let mut grad = Gradient::linear_from_points(Vector2F::default(), Vector2F::default());
        for i in 0..110 {
            let zero_width = (i == 0) || (11 <= i && i < 99) || (i == 109);
            grad.add_color_stop(ColorU::new(if zero_width { 255 } else { 0 }, 0, 0, 1), (i % 11) as f32 / 10.0);
        }

        for i in 0..11 {
            let sample = grad.sample(i as f32 / 10.0);
            assert!(sample.r == 0, "{} {}", i, sample.r);
        }
    }
}
