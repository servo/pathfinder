// pathfinder/content/src/gradient.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Gradient effects that paths can be filled with.

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

/// A gradient, either linear or radial.
#[derive(Clone, PartialEq, Debug)]
pub struct Gradient {
    /// Information specific to the type of gradient (linear or radial).
    pub geometry: GradientGeometry,
    stops: Vec<ColorStop>,
    /// What should be rendered upon reaching the end of the color stops.
    pub wrap: GradientWrap,
}

/// A color in a gradient. Points in a gradient between two stops interpolate linearly between the
/// stops.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ColorStop {
    /// The offset of the color stop, between 0.0 and 1.0 inclusive. The value 0.0 represents the
    /// start of the gradient, and 1.0 represents the end.
    pub offset: f32,
    /// The color of the gradient stop.
    pub color: ColorU,
}

/// The type of gradient: linear or radial.
#[derive(Clone, PartialEq, Debug)]
pub enum GradientGeometry {
    /// A linear gradient that follows a line.
    ///
    /// The line is in scene coordinates, not relative to the bounding box of the path.
    Linear(LineSegment2F),
    /// A radial gradient that radiates outward from a line connecting two circles (or from one
    /// circle).
    Radial {
        /// The line that connects the centers of the two circles. For single-circle radial
        /// gradients (the common case), this line has zero length, with start point and endpoint
        /// both at the circle's center point.
        ///
        /// This is in scene coordinates, not relative to the bounding box of the path.
        line: LineSegment2F,
        /// The radii of the two circles. The first value may be zero to start the gradient at the
        /// center of the circle.
        radii: F32x2,
        /// Transform from radial gradient space into screen space.
        ///
        /// Like `gradientTransform` in SVG. Note that this is the inverse of Cairo's gradient
        /// transform.
        transform: Transform2F,
    }
}

/// What should be rendered outside the color stops.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GradientWrap {
    /// The area before the gradient is filled with the color of the first stop, and the area after
    /// the gradient is filled with the color of the last stop.
    Clamp,
    /// The gradient repeats indefinitely.
    Repeat,
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
    /// Creates a new linear gradient with the given line.
    ///
    /// The line is in scene coordinates, not relative to the bounding box of the current path.
    #[inline]
    pub fn linear(line: LineSegment2F) -> Gradient {
        Gradient {
            geometry: GradientGeometry::Linear(line),
            stops: Vec::new(),
            wrap: GradientWrap::Clamp,
        }
    }

    /// A convenience method equivalent to `Gradient::linear(LineSegment2F::new(from, to))`.
    #[inline]
    pub fn linear_from_points(from: Vector2F, to: Vector2F) -> Gradient {
        Gradient::linear(LineSegment2F::new(from, to))
    }

    /// Creates a new radial gradient from a line connecting the centers of two circles with the
    /// given radii, or a point at the center of one circle.
    ///
    /// To create a radial gradient with a single circle (the common case), pass a `Vector2F`
    /// representing the center of the circle for `line`; otherwise, to create a radial gradient
    /// with two circles, pass a `LineSegment2F`. To start the gradient at the center of the
    /// circle, pass zero for the first radius.
    #[inline]
    pub fn radial<L>(line: L, radii: F32x2) -> Gradient where L: RadialGradientLine {
        let transform = Transform2F::default();
        Gradient {
            geometry: GradientGeometry::Radial { line: line.to_line(), radii, transform },
            stops: Vec::new(),
            wrap: GradientWrap::Clamp,
        }
    }

    /// Adds a new color stop to the radial gradient.
    #[inline]
    pub fn add(&mut self, stop: ColorStop) {
        let index = self.stops.binary_search_by(|other| {
            if other.offset <= stop.offset { Ordering::Less } else { Ordering::Greater }
        }).unwrap_or_else(convert::identity);
        self.stops.insert(index, stop);
    }

    /// A convenience method equivalent to
    /// `gradient.add_color_stop(ColorStop::new(color, offset))`.
    #[inline]
    pub fn add_color_stop(&mut self, color: ColorU, offset: f32) {
        self.add(ColorStop::new(color, offset))
    }

    /// Returns the list of color stops in this gradient.
    #[inline]
    pub fn stops(&self) -> &[ColorStop] {
        &self.stops
    }

    /// Returns a mutable version of the list of color stops in this gradient.
    #[inline]
    pub fn stops_mut(&mut self) -> &mut [ColorStop] {
        &mut self.stops
    }

    /// Returns the value of the gradient at offset `t`, which will be clamped between 0.0 and 1.0.
    ///
    /// FIXME(pcwalton): This should probably take `wrap` into account…
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

        let ratio = ((t - lower_stop.offset) / denom).min(1.0);
        lower_stop.color.to_f32().lerp(upper_stop.color.to_f32(), ratio).to_u8()
    }

    /// Returns true if all colors of all stops in this gradient are opaque (alpha is 1.0).
    #[inline]
    pub fn is_opaque(&self) -> bool {
        self.stops.iter().all(|stop| stop.color.is_opaque())
    }

    /// Returns true if all colors of all stops in this gradient are fully transparent (alpha is
    /// 0.0).
    #[inline]
    pub fn is_fully_transparent(&self) -> bool {
        self.stops.iter().all(|stop| stop.color.is_fully_transparent())
    }

    /// Applies the given affine transform to this gradient.
    ///
    /// FIXME(pcwalton): This isn't correct for radial gradients, as transforms can transform the
    /// circles into ellipses…
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
    /// Creates a new color stop from a color and offset between 0.0 and 1.0 inclusive.
    #[inline]
    pub fn new(color: ColorU, offset: f32) -> ColorStop {
        ColorStop { color, offset }
    }
}

/// Allows `Gradient::radial` to be called with either a `LineSegment2F` or a `Vector2F`.
pub trait RadialGradientLine {
    /// Represents this value as a line.
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
