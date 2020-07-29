// pathfinder/content/src/segment.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Single line or Bézier curve segments, optimized with SIMD.

use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::util::EPSILON;
use pathfinder_geometry::vector::{Vector2F, vec2f};
use pathfinder_simd::default::F32x4;
use std::f32::consts::SQRT_2;

/// A single line or Bézier curve segment, with explicit start and end points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    /// The start and end points of the curve.
    pub baseline: LineSegment2F,
    /// The control point or points.
    ///
    /// If this is a line (which can be determined by examining the segment kind), this field is
    /// ignored. If this is a quadratic Bézier curve, the start point of this line represents the
    /// control point, and the endpoint of this line is ignored. Otherwise, if this is a cubic
    /// Bézier curve, both the start and endpoints are used.
    pub ctrl: LineSegment2F,
    /// The type of segment this is: invalid, line, quadratic, or cubic Bézier curve.
    pub kind: SegmentKind,
    /// Various flags that describe information about this segment in a path.
    pub flags: SegmentFlags,
}

impl Segment {
    /// Returns an invalid segment.
    #[inline]
    pub fn none() -> Segment {
        Segment {
            baseline: LineSegment2F::default(),
            ctrl: LineSegment2F::default(),
            kind: SegmentKind::None,
            flags: SegmentFlags::empty(),
        }
    }

    /// Returns a segment representing a straight line.
    #[inline]
    pub fn line(line: LineSegment2F) -> Segment {
        Segment {
            baseline: line,
            ctrl: LineSegment2F::default(),
            kind: SegmentKind::Line,
            flags: SegmentFlags::empty(),
        }
    }

    /// Returns a segment representing a quadratic Bézier curve.
    #[inline]
    pub fn quadratic(baseline: LineSegment2F, ctrl: Vector2F) -> Segment {
        Segment {
            baseline,
            ctrl: LineSegment2F::new(ctrl, Vector2F::zero()),
            kind: SegmentKind::Quadratic,
            flags: SegmentFlags::empty(),
        }
    }

    /// Returns a segment representing a cubic Bézier curve.
    #[inline]
    pub fn cubic(baseline: LineSegment2F, ctrl: LineSegment2F) -> Segment {
        Segment {
            baseline,
            ctrl,
            kind: SegmentKind::Cubic,
            flags: SegmentFlags::empty(),
        }
    }

    /// Approximates an unit-length arc with a cubic Bézier curve.
    ///
    /// The maximum supported sweep angle is π/2 (i.e. 90°).
    pub fn arc(sweep_angle: f32) -> Segment {
        Segment::arc_from_cos(f32::cos(sweep_angle))
    }

    /// Approximates an unit-length arc with a cubic Bézier curve, given the cosine of the sweep
    /// angle.
    ///
    /// The maximum supported sweep angle is π/2 (i.e. 90°).
    pub fn arc_from_cos(cos_sweep_angle: f32) -> Segment {
        // Richard A. DeVeneza, "How to determine the control points of a Bézier curve that
        // approximates a small arc", 2004.
        //
        // https://www.tinaja.com/glib/bezcirc2.pdf
        if cos_sweep_angle >= 1.0 - EPSILON {
            return Segment::line(LineSegment2F::new(vec2f(1.0, 0.0), vec2f(1.0, 0.0)));
        }

        let term = F32x4::new(cos_sweep_angle, -cos_sweep_angle,
                              cos_sweep_angle, -cos_sweep_angle);
        let signs = F32x4::new(1.0, -1.0, 1.0, 1.0);
        let p3p0 = ((F32x4::splat(1.0) + term) * F32x4::splat(0.5)).sqrt() * signs;
        let (p0x, p0y) = (p3p0.z(), p3p0.w());
        let (p1x, p1y) = (4.0 - p0x, (1.0 - p0x) * (3.0 - p0x) / p0y);
        let p2p1 = F32x4::new(p1x, -p1y, p1x, p1y) * F32x4::splat(1.0 / 3.0);
        return Segment::cubic(LineSegment2F(p3p0), LineSegment2F(p2p1));
    }

    /// Returns a cubic Bézier segment that approximates a quarter of an arc, centered on the +x
    /// axis.
    #[inline]
    pub fn quarter_circle_arc() -> Segment {
        let p0 = Vector2F::splat(SQRT_2 * 0.5);
        let p1 = vec2f(-SQRT_2 / 6.0 + 4.0 / 3.0, 7.0 * SQRT_2 / 6.0 - 4.0 / 3.0);
        let flip = vec2f(1.0, -1.0);
        let (p2, p3) = (p1 * flip, p0 * flip);
        Segment::cubic(LineSegment2F::new(p3, p0), LineSegment2F::new(p2, p1))
    }

    /// If this segment is a line, returns it. In debug builds, panics otherwise.
    #[inline]
    pub fn as_line_segment(&self) -> LineSegment2F {
        debug_assert!(self.is_line());
        self.baseline
    }

    /// Returns true if this segment is invalid.
    #[inline]
    pub fn is_none(&self) -> bool {
        self.kind == SegmentKind::None
    }

    /// Returns true if this segment represents a straight line.
    #[inline]
    pub fn is_line(&self) -> bool {
        self.kind == SegmentKind::Line
    }

    /// Returns true if this segment represents a quadratic Bézier curve.
    #[inline]
    pub fn is_quadratic(&self) -> bool {
        self.kind == SegmentKind::Quadratic
    }

    /// Returns true if this segment represents a cubic Bézier curve.
    #[inline]
    pub fn is_cubic(&self) -> bool {
        self.kind == SegmentKind::Cubic
    }

    /// If this segment is a cubic Bézier curve, returns it. In debug builds, panics otherwise.
    #[inline]
    pub fn as_cubic_segment(&self) -> CubicSegment {
        debug_assert!(self.is_cubic());
        CubicSegment(self)
    }

    /// If this segment is a quadratic Bézier curve, elevates it to a cubic Bézier curve and
    /// returns it. If this segment is a cubic Bézier curve, this method simply returns it.
    ///
    /// If this segment is neither a quadratic Bézier curve nor a cubic Bézier curve, this method
    /// returns an unspecified result.
    ///
    /// FIXME(pcwalton): Handle lines!
    // FIXME(pcwalton): We should basically never use this function.
    #[inline]
    pub fn to_cubic(&self) -> Segment {
        if self.is_cubic() {
            return *self;
        }

        let mut new_segment = *self;
        let p1_2 = self.ctrl.from() + self.ctrl.from();
        new_segment.ctrl = LineSegment2F::new(self.baseline.from() + p1_2,
                                              p1_2 + self.baseline.to()) * (1.0 / 3.0);
        new_segment.kind = SegmentKind::Cubic;
        new_segment
    }

    /// Returns this segment with endpoints and control points reversed.
    #[inline]
    pub fn reversed(&self) -> Segment {
        Segment {
            baseline: self.baseline.reversed(),
            ctrl: if self.is_quadratic() {
                self.ctrl
            } else {
                self.ctrl.reversed()
            },
            kind: self.kind,
            flags: self.flags,
        }
    }

    /// Returns true if this segment is smaller than an implementation-defined epsilon value.
    #[inline]
    pub fn is_tiny(&self) -> bool {
        const EPSILON: f32 = 0.0001;
        self.baseline.square_length() < EPSILON
    }

    /// Divides this segment into two at the given parametric t value, which must range from 0.0 to
    /// 1.0.
    ///
    /// This uses de Casteljau subdivision.
    #[inline]
    pub fn split(&self, t: f32) -> (Segment, Segment) {
        // FIXME(pcwalton): Don't degree elevate!
        if self.is_line() {
            let (before, after) = self.as_line_segment().split(t);
            (Segment::line(before), Segment::line(after))
        } else {
            self.to_cubic().as_cubic_segment().split(t)
        }
    }

    /// Returns the position of the point on this line or curve with the given parametric t value,
    /// which must range from 0.0 to 1.0.
    ///
    /// If called on an invalid segment (`None` type), the result is unspecified.
    #[inline]
    pub fn sample(self, t: f32) -> Vector2F {
        // FIXME(pcwalton): Don't degree elevate!
        if self.is_line() {
            self.as_line_segment().sample(t)
        } else {
            self.to_cubic().as_cubic_segment().sample(t)
        }
    }

    /// Applies the given affine transform to this segment and returns it.
    #[inline]
    pub fn transform(self, transform: &Transform2F) -> Segment {
        Segment {
            baseline: *transform * self.baseline,
            ctrl: *transform * self.ctrl,
            kind: self.kind,
            flags: self.flags,
        }
    }

    pub(crate) fn arc_length(&self) -> f32 {
        // FIXME(pcwalton)
        self.baseline.vector().length()
    }

    pub(crate) fn time_for_distance(&self, distance: f32) -> f32 {
        // FIXME(pcwalton)
        distance / self.arc_length()
    }
}

/// The type of segment this is.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum SegmentKind {
    /// An invalid segment.
    None,
    /// A line segment.
    Line,
    /// A quadratic Bézier curve.
    Quadratic,
    /// A cubic Bézier curve.
    Cubic,
}

bitflags! {
    /// Various flags that specify the relation of this segment to other segments in a contour.
    pub struct SegmentFlags: u8 {
        /// This segment is the first one in the contour.
        const FIRST_IN_SUBPATH = 0x01;
        /// This segment is the closing segment of the contour (i.e. it returns back to the
        /// starting point).
        const CLOSES_SUBPATH = 0x02;
    }
}

/// A wrapper for a `Segment` that contains method specific to cubic Bézier curves.
#[derive(Clone, Copy, Debug)]
pub struct CubicSegment<'s>(pub &'s Segment);

impl<'s> CubicSegment<'s> {
    /// Returns true if the maximum deviation of this curve from the straight line connecting its
    /// endpoints is less than `tolerance`.
    ///
    /// See Kaspar Fischer, "Piecewise Linear Approximation of Bézier Curves", 2000.
    #[inline]
    pub fn is_flat(self, tolerance: f32) -> bool {
        let mut uv = F32x4::splat(3.0) * self.0.ctrl.0
            - self.0.baseline.0
            - self.0.baseline.0
            - self.0.baseline.reversed().0;
        uv = uv * uv;
        uv = uv.max(uv.zwxy());
        uv[0] + uv[1] <= 16.0 * tolerance * tolerance
    }

    /// Splits this cubic Bézier curve into two at the given parametric t value, which will be
    /// clamped to the range 0.0 to 1.0.
    ///
    /// This uses de Casteljau subdivision.
    #[inline]
    pub fn split(self, t: f32) -> (Segment, Segment) {
        let (baseline0, ctrl0, baseline1, ctrl1);
        if t <= 0.0 {
            let from = &self.0.baseline.from();
            baseline0 = LineSegment2F::new(*from, *from);
            ctrl0 = LineSegment2F::new(*from, *from);
            baseline1 = self.0.baseline;
            ctrl1 = self.0.ctrl;
        } else if t >= 1.0 {
            let to = &self.0.baseline.to();
            baseline0 = self.0.baseline;
            ctrl0 = self.0.ctrl;
            baseline1 = LineSegment2F::new(*to, *to);
            ctrl1 = LineSegment2F::new(*to, *to);
        } else {
            let tttt = F32x4::splat(t);

            let (p0p3, p1p2) = (self.0.baseline.0, self.0.ctrl.0);
            let p0p1 = p0p3.concat_xy_xy(p1p2);

            // p01 = lerp(p0, p1, t), p12 = lerp(p1, p2, t), p23 = lerp(p2, p3, t)
            let p01p12 = p0p1 + tttt * (p1p2 - p0p1);
            let pxxp23 = p1p2 + tttt * (p0p3 - p1p2);
            let p12p23 = p01p12.concat_zw_zw(pxxp23);

            // p012 = lerp(p01, p12, t), p123 = lerp(p12, p23, t)
            let p012p123 = p01p12 + tttt * (p12p23 - p01p12);
            let p123 = p012p123.zwzw();

            // p0123 = lerp(p012, p123, t)
            let p0123 = p012p123 + tttt * (p123 - p012p123);

            baseline0 = LineSegment2F(p0p3.concat_xy_xy(p0123));
            ctrl0 = LineSegment2F(p01p12.concat_xy_xy(p012p123));
            baseline1 = LineSegment2F(p0123.concat_xy_zw(p0p3));
            ctrl1 = LineSegment2F(p012p123.concat_zw_zw(p12p23));
        }

        (
            Segment {
                baseline: baseline0,
                ctrl: ctrl0,
                kind: SegmentKind::Cubic,
                flags: self.0.flags & SegmentFlags::FIRST_IN_SUBPATH,
            },
            Segment {
                baseline: baseline1,
                ctrl: ctrl1,
                kind: SegmentKind::Cubic,
                flags: self.0.flags & SegmentFlags::CLOSES_SUBPATH,
            },
        )
    }

    /// A convenience method equivalent to `segment.split(t).0`.
    #[inline]
    pub fn split_before(self, t: f32) -> Segment {
        self.split(t).0
    }

    /// A convenience method equivalent to `segment.split(t).1`.
    #[inline]
    pub fn split_after(self, t: f32) -> Segment {
        self.split(t).1
    }

    /// Returns the position of the point on this curve at parametric time `t`, which will be
    /// clamped between 0.0 and 1.0.
    ///
    /// FIXME(pcwalton): Use Horner's method!
    #[inline]
    pub fn sample(self, t: f32) -> Vector2F {
        self.split(t).0.baseline.to()
    }

    /// Returns the left extent of this curve's axis-aligned bounding box.
    #[inline]
    pub fn min_x(&self) -> f32 {
        f32::min(self.0.baseline.min_x(), self.0.ctrl.min_x())
    }
    /// Returns the top extent of this curve's axis-aligned bounding box.
    #[inline]
    pub fn min_y(&self) -> f32 {
        f32::min(self.0.baseline.min_y(), self.0.ctrl.min_y())
    }
    /// Returns the right extent of this curve's axis-aligned bounding box.
    #[inline]
    pub fn max_x(&self) -> f32 {
        f32::max(self.0.baseline.max_x(), self.0.ctrl.max_x())
    }
    /// Returns the bottom extent of this curve's axis-aligned bounding box.
    #[inline]
    pub fn max_y(&self) -> f32 {
        f32::max(self.0.baseline.max_y(), self.0.ctrl.max_y())
    }
}
