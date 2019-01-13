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
use crate::simd::F32x4;

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
pub struct CubicSegment<'s>(&'s Segment);

impl<'s> CubicSegment<'s> {
    #[inline]
    pub fn flatten_once(self, tolerance: f32) -> Option<Segment> {
        let (baseline, ctrl) = (self.0.baseline.0, self.0.ctrl.0);
        let from_from = baseline.xyxy();
        let v0102 = ctrl - from_from;

        //      v01.x   v01.y   v02.x v02.y
        //    * v01.x   v01.y   v01.y v01.x
        //    -------------------------
        //      v01.x^2 v01.y^2 ad    bc
        //         |       |     |     |
        //         +-------+     +-----+
        //             +            -
        //         v01 len^2   determinant
        let products = v0102 * v0102.xyyx();

        let det = products[2] - products[3];
        if det == 0.0 {
            return None;
        }

        let s2inv = (products[0] + products[1]).sqrt() / det;

        let t = 2.0 * ((tolerance / 3.0) * s2inv.abs()).sqrt();
        if t >= 1.0 - EPSILON || t == 0.0 {
            return None;
        }

        return Some(self.split_after(t));

        const EPSILON: f32 = 0.005;
    }

    #[inline]
    pub fn split(self, t: f32) -> (Segment, Segment) {
        let tttt = F32x4::splat(t);

        let (p0p3, p1p2) = (self.0.baseline.0, self.0.ctrl.0);
        let p0p1 = p0p3.as_f64x2().interleave(p1p2.as_f64x2()).0.as_f32x4();

        // p01 = lerp(p0, p1, t), p12 = lerp(p1, p2, t), p23 = lerp(p2, p3, t)
        let p01p12 = p0p1 + tttt * (p1p2 - p0p1);
        let pxxp23 = p1p2 + tttt * (p0p3 - p1p2);
        let p12p23 = p01p12.as_f64x2().interleave(pxxp23.as_f64x2()).1.as_f32x4();

        // p012 = lerp(p01, p12, t), p123 = lerp(p12, p23, t)
        let p012p123 = p01p12 + tttt * (p12p23 - p01p12);
        let p123 = p012p123.zwzw();

        // p0123 = lerp(p012, p123, t)
        let p0123 = p012p123 + tttt * (p123 - p012p123);

        let baseline0 = p0p3.as_f64x2().interleave(p0123.as_f64x2()).0.as_f32x4();
        let ctrl0 = p01p12.as_f64x2().interleave(p012p123.as_f64x2()).0.as_f32x4();
        let baseline1 = p0123.as_f64x2().combine_low_high(p0p3.as_f64x2()).as_f32x4();
        let ctrl1 = p012p123.as_f64x2().interleave(p12p23.as_f64x2()).1.as_f32x4();

        (Segment {
            baseline: LineSegmentF32(baseline0),
            ctrl: LineSegmentF32(ctrl0),
            kind: SegmentKind::Cubic,
            flags: self.0.flags & SegmentFlags::FIRST_IN_SUBPATH,
        }, Segment {
            baseline: LineSegmentF32(baseline1),
            ctrl: LineSegmentF32(ctrl1),
            kind: SegmentKind::Cubic,
            flags: self.0.flags & SegmentFlags::CLOSES_SUBPATH,
        })
    }

    #[inline]
    pub fn split_after(self, t: f32) -> Segment {
        self.split(t).1
    }

    #[inline]
    pub fn y_extrema(self) -> (Option<f32>, Option<f32>) {
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
}
