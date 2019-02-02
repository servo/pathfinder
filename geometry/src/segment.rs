// pathfinder/geometry/src/segment.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Line or curve segments, optimized with SIMD.

use crate::line_segment::LineSegmentF32;
use crate::point::Point2DF32;
use lyon_path::PathEvent;
use pathfinder_simd::default::F32x4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub baseline: LineSegmentF32,
    pub ctrl: LineSegmentF32,
    pub kind: SegmentKind,
    pub flags: SegmentFlags,
}

impl Segment {
    #[inline]
    pub fn none() -> Segment {
        Segment {
            baseline: LineSegmentF32::default(),
            ctrl: LineSegmentF32::default(),
            kind: SegmentKind::None,
            flags: SegmentFlags::empty(),
        }
    }

    #[inline]
    pub fn line(line: &LineSegmentF32) -> Segment {
        Segment {
            baseline: *line,
            ctrl: LineSegmentF32::default(),
            kind: SegmentKind::Line,
            flags: SegmentFlags::empty(),
        }
    }

    #[inline]
    pub fn quadratic(baseline: &LineSegmentF32, ctrl: &Point2DF32) -> Segment {
        Segment {
            baseline: *baseline,
            ctrl: LineSegmentF32::new(ctrl, &Point2DF32::default()),
            kind: SegmentKind::Cubic,
            flags: SegmentFlags::empty(),
        }
    }

    #[inline]
    pub fn cubic(baseline: &LineSegmentF32, ctrl: &LineSegmentF32) -> Segment {
        Segment {
            baseline: *baseline,
            ctrl: *ctrl,
            kind: SegmentKind::Cubic,
            flags: SegmentFlags::empty(),
        }
    }

    #[inline]
    pub fn as_line_segment(&self) -> LineSegmentF32 {
        debug_assert!(self.is_line());
        self.baseline
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        self.kind == SegmentKind::None
    }

    #[inline]
    pub fn is_line(&self) -> bool {
        self.kind == SegmentKind::Line
    }

    #[inline]
    pub fn is_quadratic(&self) -> bool {
        self.kind == SegmentKind::Quadratic
    }

    #[inline]
    pub fn is_cubic(&self) -> bool {
        self.kind == SegmentKind::Cubic
    }

    #[inline]
    pub fn as_cubic_segment(&self) -> CubicSegment {
        debug_assert!(self.is_cubic());
        CubicSegment(self)
    }

    // FIXME(pcwalton): We should basically never use this function.
    // FIXME(pcwalton): Handle lines!
    #[inline]
    pub fn to_cubic(&self) -> Segment {
        if self.is_cubic() {
            return *self;
        }

        let mut new_segment = *self;
        let p1_2 = self.ctrl.from() + self.ctrl.from();
        new_segment.ctrl =
            LineSegmentF32::new(&(self.baseline.from() + p1_2), &(p1_2 + self.baseline.to()))
                .scale(1.0 / 3.0);
        new_segment
    }

    #[inline]
    pub fn is_monotonic(&self) -> bool {
        // FIXME(pcwalton): Don't degree elevate!
        match self.kind {
            SegmentKind::None | SegmentKind::Line => true,
            SegmentKind::Quadratic => self.to_cubic().as_cubic_segment().is_monotonic(),
            SegmentKind::Cubic => self.as_cubic_segment().is_monotonic(),
        }
    }

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

    // Reverses if necessary so that the from point is above the to point. Calling this method
    // again will undo the transformation.
    #[inline]
    pub fn orient(&self, y_winding: i32) -> Segment {
        if y_winding >= 0 {
            *self
        } else {
            self.reversed()
        }
    }

    #[inline]
    pub fn is_tiny(&self) -> bool {
        const EPSILON: f32 = 0.0001;
        self.baseline.square_length() < EPSILON
    }

    #[inline]
    pub fn split(&self, t: f32) -> (Segment, Segment) {
        // FIXME(pcwalton): Don't degree elevate!
        if self.is_line() {
            let (before, after) = self.as_line_segment().split(t);
            (Segment::line(&before), Segment::line(&after))
        } else {
            self.to_cubic().as_cubic_segment().split(t)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum SegmentKind {
    None,
    Line,
    Quadratic,
    Cubic,
}

bitflags! {
    pub struct SegmentFlags: u8 {
        const FIRST_IN_SUBPATH = 0x01;
        const CLOSES_SUBPATH = 0x02;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CubicSegment<'s>(pub &'s Segment);

impl<'s> CubicSegment<'s> {
    // See Kaspar Fischer, "Piecewise Linear Approximation of Bézier Curves", 2000.
    #[inline]
    pub fn is_flat(self, tolerance: f32) -> bool {
        let mut uv = F32x4::splat(3.0) * self.0.ctrl.0 -
            self.0.baseline.0 - self.0.baseline.0 -
            self.0.baseline.reversed().0;
        uv = uv * uv;
        uv = uv.max(uv.zwxy());
        uv[0] + uv[1] <= 16.0 * tolerance * tolerance
    }

    /*
    #[inline]
    pub fn flatten_once(self, tolerance: f32) -> Option<Segment> {
        if self.is_flat(tolerance) {
            None
        } else {
            Some(self.split_after(0.5))
        }
    }
    */

    #[inline]
    pub fn split(self, t: f32) -> (Segment, Segment) {
        let (baseline0, ctrl0, baseline1, ctrl1);
        if t <= 0.0 {
            let from = &self.0.baseline.from();
            baseline0 = LineSegmentF32::new(from, from);
            ctrl0 = LineSegmentF32::new(from, from);
            baseline1 = self.0.baseline;
            ctrl1 = self.0.ctrl;
        } else if t >= 1.0 {
            let to = &self.0.baseline.to();
            baseline0 = self.0.baseline;
            ctrl0 = self.0.ctrl;
            baseline1 = LineSegmentF32::new(to, to);
            ctrl1 = LineSegmentF32::new(to, to);
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

            baseline0 = LineSegmentF32(p0p3.concat_xy_xy(p0123));
            ctrl0 = LineSegmentF32(p01p12.concat_xy_xy(p012p123));
            baseline1 = LineSegmentF32(p0123.concat_xy_zw(p0p3));
            ctrl1 = LineSegmentF32(p012p123.concat_zw_zw(p12p23));
        }

        (Segment {
            baseline: baseline0,
            ctrl: ctrl0,
            kind: SegmentKind::Cubic,
            flags: self.0.flags & SegmentFlags::FIRST_IN_SUBPATH,
        }, Segment {
            baseline: baseline1,
            ctrl: ctrl1,
            kind: SegmentKind::Cubic,
            flags: self.0.flags & SegmentFlags::CLOSES_SUBPATH,
        })
    }

    #[inline]
    pub fn split_before(self, t: f32) -> Segment {
        self.split(t).0
    }

    #[inline]
    pub fn split_after(self, t: f32) -> Segment {
        self.split(t).1
    }

    // FIXME(pcwalton): Use Horner's method!
    #[inline]
    pub fn sample(self, t: f32) -> Point2DF32 {
        self.split(t).0.baseline.to()
    }

    #[inline]
    pub fn is_monotonic(self) -> bool {
        // TODO(pcwalton): Optimize this.
        let (p0, p3) = (self.0.baseline.from_y(), self.0.baseline.to_y());
        let (p1, p2) = (self.0.ctrl.from_y(), self.0.ctrl.to_y());
        (p0 <= p1 && p1 <= p2 && p2 <= p3) || (p0 >= p1 && p1 >= p2 && p2 >= p3)
    }

    #[inline]
    pub fn y_extrema(self) -> (Option<f32>, Option<f32>) {
        if self.is_monotonic() {
            return (None, None)
        }

        let p0p1p2p3 = F32x4::new(self.0.baseline.from_y(),
                                  self.0.ctrl.from_y(),
                                  self.0.ctrl.to_y(),
                                  self.0.baseline.to_y());

        let pxp0p1p2 = p0p1p2p3.wxyz();
        let pxv0v1v2 = p0p1p2p3 - pxp0p1p2;
        let (v0, v1, v2) = (pxv0v1v2[1], pxv0v1v2[2], pxv0v1v2[3]);

        let (v0_to_v1, v2_to_v1) = (v0 - v1, v2 - v1);
        let discrim = f32::sqrt(v1 * v1 - v0 * v2);
        let denom = 1.0 / (v0_to_v1 + v2_to_v1);

        let t0 = (v0_to_v1 + discrim) * denom;
        let t1 = (v0_to_v1 - discrim) * denom;

        return match (
            t0 > EPSILON && t0 < 1.0 - EPSILON,
            t1 > EPSILON && t1 < 1.0 - EPSILON,
        ) {
            (false, false) => (None, None),
            (true, false) => (Some(t0), None),
            (false, true) => (Some(t1), None),
            (true, true) => (Some(f32::min(t0, t1)), Some(f32::max(t0, t1))),
        };

        const EPSILON: f32 = 0.001;
    }

    #[inline]
    pub fn min_x(&self) -> f32 { f32::min(self.0.baseline.min_x(), self.0.ctrl.min_x()) }
    #[inline]
    pub fn min_y(&self) -> f32 { f32::min(self.0.baseline.min_y(), self.0.ctrl.min_y()) }
    #[inline]
    pub fn max_x(&self) -> f32 { f32::max(self.0.baseline.max_x(), self.0.ctrl.max_x()) }
    #[inline]
    pub fn max_y(&self) -> f32 { f32::max(self.0.baseline.max_y(), self.0.ctrl.max_y()) }
}

// Lyon interoperability

pub struct PathEventsToSegments<I>
where
    I: Iterator<Item = PathEvent>,
{
    iter: I,
    first_subpath_point: Point2DF32,
    last_subpath_point: Point2DF32,
    just_moved: bool,
}

impl<I> PathEventsToSegments<I>
where
    I: Iterator<Item = PathEvent>,
{
    #[inline]
    pub fn new(iter: I) -> PathEventsToSegments<I> {
        PathEventsToSegments {
            iter,
            first_subpath_point: Point2DF32::default(),
            last_subpath_point: Point2DF32::default(),
            just_moved: false,
        }
    }
}

impl<I> Iterator for PathEventsToSegments<I>
where
    I: Iterator<Item = PathEvent>,
{
    type Item = Segment;

    #[inline]
    fn next(&mut self) -> Option<Segment> {
        match self.iter.next()? {
            PathEvent::MoveTo(to) => {
                let to = Point2DF32::from_euclid(to);
                self.first_subpath_point = to;
                self.last_subpath_point = to;
                self.just_moved = true;
                self.next()
            }
            PathEvent::LineTo(to) => {
                let to = Point2DF32::from_euclid(to);
                let mut segment =
                    Segment::line(&LineSegmentF32::new(&self.last_subpath_point, &to));
                if self.just_moved {
                    segment.flags.insert(SegmentFlags::FIRST_IN_SUBPATH);
                }
                self.last_subpath_point = to;
                self.just_moved = false;
                Some(segment)
            }
            PathEvent::QuadraticTo(ctrl, to) => {
                let (ctrl, to) = (Point2DF32::from_euclid(ctrl), Point2DF32::from_euclid(to));
                let mut segment =
                    Segment::quadratic(&LineSegmentF32::new(&self.last_subpath_point, &to), &ctrl);
                if self.just_moved {
                    segment.flags.insert(SegmentFlags::FIRST_IN_SUBPATH);
                }
                self.last_subpath_point = to;
                self.just_moved = false;
                Some(segment)
            }
            PathEvent::CubicTo(ctrl0, ctrl1, to) => {
                let ctrl0 = Point2DF32::from_euclid(ctrl0);
                let ctrl1 = Point2DF32::from_euclid(ctrl1);
                let to = Point2DF32::from_euclid(to);
                let mut segment = Segment::cubic(
                    &LineSegmentF32::new(&self.last_subpath_point, &to),
                    &LineSegmentF32::new(&ctrl0, &ctrl1),
                );
                if self.just_moved {
                    segment.flags.insert(SegmentFlags::FIRST_IN_SUBPATH);
                }
                self.last_subpath_point = to;
                self.just_moved = false;
                Some(segment)
            }
            PathEvent::Close => {
                let mut segment = Segment::line(&LineSegmentF32::new(
                    &self.last_subpath_point,
                    &self.first_subpath_point,
                ));
                segment.flags.insert(SegmentFlags::CLOSES_SUBPATH);
                self.just_moved = false;
                self.last_subpath_point = self.first_subpath_point;
                Some(segment)
            }
            PathEvent::Arc(..) => panic!("TODO: arcs"),
        }
    }
}

pub struct SegmentsToPathEvents<I>
where
    I: Iterator<Item = Segment>,
{
    iter: I,
    buffer: Option<PathEvent>,
}

impl<I> SegmentsToPathEvents<I>
where
    I: Iterator<Item = Segment>,
{
    #[inline]
    pub fn new(iter: I) -> SegmentsToPathEvents<I> {
        SegmentsToPathEvents { iter, buffer: None }
    }
}

impl<I> Iterator for SegmentsToPathEvents<I>
where
    I: Iterator<Item = Segment>,
{
    type Item = PathEvent;

    #[inline]
    fn next(&mut self) -> Option<PathEvent> {
        if let Some(event) = self.buffer.take() {
            return Some(event);
        }

        let segment = self.iter.next()?;
        if segment.flags.contains(SegmentFlags::CLOSES_SUBPATH) {
            return Some(PathEvent::Close);
        }

        let event = match segment.kind {
            SegmentKind::None => return self.next(),
            SegmentKind::Line => PathEvent::LineTo(segment.baseline.to().as_euclid()),
            SegmentKind::Quadratic => PathEvent::QuadraticTo(
                segment.ctrl.from().as_euclid(),
                segment.baseline.to().as_euclid(),
            ),
            SegmentKind::Cubic => PathEvent::CubicTo(
                segment.ctrl.from().as_euclid(),
                segment.ctrl.to().as_euclid(),
                segment.baseline.to().as_euclid(),
            ),
        };

        if segment.flags.contains(SegmentFlags::FIRST_IN_SUBPATH) {
            self.buffer = Some(event);
            Some(PathEvent::MoveTo(segment.baseline.from().as_euclid()))
        } else {
            Some(event)
        }
    }
}
