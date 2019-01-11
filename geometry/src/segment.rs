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

use crate::SimdImpl;
use crate::line_segment::LineSegmentF32;
use crate::point::Point2DF32;
use simdeez::Simd;

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
        let s2inv;
        unsafe {
            let (baseline, ctrl) = (self.0.baseline.0, self.0.ctrl.0);
            let from_from = SimdImpl::shuffle_ps(baseline, baseline, 0b0100_0100);

            let v0102 = SimdImpl::sub_ps(ctrl, from_from);

            //      v01.x   v01.y   v02.x v02.y
            //    * v01.x   v01.y   v01.y v01.x
            //    -------------------------
            //      v01.x^2 v01.y^2 ad    bc
            //         |       |     |     |
            //         +-------+     +-----+
            //             +            -
            //         v01 len^2   determinant
            let products = SimdImpl::mul_ps(v0102, SimdImpl::shuffle_ps(v0102, v0102, 0b0001_0100));

            let det = products[2] - products[3];
            if det == 0.0 {
                return None;
            }

            s2inv = (products[0] + products[1]).sqrt() / det;
        }

        let t = 2.0 * ((tolerance / 3.0) * s2inv.abs()).sqrt();
        if t >= 1.0 - EPSILON || t == 0.0 {
            return None;
        }

        return Some(self.split_after(t));

        const EPSILON: f32 = 0.005;
    }

    #[inline]
    pub fn split(self, t: f32) -> (Segment, Segment) {
        unsafe {
            let tttt = SimdImpl::set1_ps(t);

            let p0p3 = self.0.baseline.0;
            let p1p2 = self.0.ctrl.0;
            let p0p1 = assemble(&p0p3, &p1p2, 0, 0);

            // p01 = lerp(p0, p1, t), p12 = lerp(p1, p2, t), p23 = lerp(p2, p3, t)
            let p01p12 = SimdImpl::add_ps(p0p1, SimdImpl::mul_ps(tttt, SimdImpl::sub_ps(p1p2, p0p1)));
            let pxxp23 = SimdImpl::add_ps(p1p2, SimdImpl::mul_ps(tttt, SimdImpl::sub_ps(p0p3, p1p2)));

            let p12p23 = assemble(&p01p12, &pxxp23, 1, 1);

            // p012 = lerp(p01, p12, t), p123 = lerp(p12, p23, t)
            let p012p123 =
                SimdImpl::add_ps(p01p12, SimdImpl::mul_ps(tttt, SimdImpl::sub_ps(p12p23, p01p12)));

            let p123 = pluck(&p012p123, 1);

            // p0123 = lerp(p012, p123, t)
            let p0123 = SimdImpl::add_ps(p012p123, SimdImpl::mul_ps(tttt, SimdImpl::sub_ps(p123, p012p123)));

            let baseline0 = assemble(&p0p3, &p0123, 0, 0);
            let ctrl0 = assemble(&p01p12, &p012p123, 0, 0);
            let baseline1 = assemble(&p0123, &p0p3, 0, 1);
            let ctrl1 = assemble(&p012p123, &p12p23, 1, 1);

            // FIXME(pcwalton): Set flags appropriately!
            return (
                Segment {
                    baseline: LineSegmentF32(baseline0),
                    ctrl: LineSegmentF32(ctrl0),
                    kind: SegmentKind::Cubic,
                    flags: self.0.flags & SegmentFlags::FIRST_IN_SUBPATH,
                },
                Segment {
                    baseline: LineSegmentF32(baseline1),
                    ctrl: LineSegmentF32(ctrl1),
                    kind: SegmentKind::Cubic,
                    flags: self.0.flags & SegmentFlags::CLOSES_SUBPATH,
                },
            );
        }

        // Constructs a new 4-element vector from two pairs of adjacent lanes in two input vectors.
        unsafe fn assemble(
            a_data: &<SimdImpl as Simd>::Vf32,
            b_data: &<SimdImpl as Simd>::Vf32,
            a_index: usize,
            b_index: usize,
        ) -> <SimdImpl as Simd>::Vf32 {
            let (a_data, b_data) = (SimdImpl::castps_pd(*a_data), SimdImpl::castps_pd(*b_data));
            let mut result = SimdImpl::setzero_pd();
            result[0] = a_data[a_index];
            result[1] = b_data[b_index];
            SimdImpl::castpd_ps(result)
        }

        // Constructs a new 2-element vector from a pair of adjacent lanes in an input vector.
        unsafe fn pluck(data: &<SimdImpl as Simd>::Vf32, index: usize) -> <SimdImpl as Simd>::Vf32 {
            let data = SimdImpl::castps_pd(*data);
            let mut result = SimdImpl::setzero_pd();
            result[0] = data[index];
            SimdImpl::castpd_ps(result)
        }
    }

    #[inline]
    pub fn split_after(self, t: f32) -> Segment {
        self.split(t).1
    }

    #[inline]
    pub fn y_extrema(self) -> (Option<f32>, Option<f32>) {
        let (t0, t1);
        unsafe {
            let mut p0p1p2p3 = SimdImpl::setzero_ps();
            p0p1p2p3[0] = self.0.baseline.from_y();
            p0p1p2p3[1] = self.0.ctrl.from_y();
            p0p1p2p3[2] = self.0.ctrl.to_y();
            p0p1p2p3[3] = self.0.baseline.to_y();

            let pxp0p1p2 = SimdImpl::shuffle_ps(p0p1p2p3, p0p1p2p3, 0b1001_0000);
            let pxv0v1v2 = SimdImpl::sub_ps(p0p1p2p3, pxp0p1p2);
            let (v0, v1, v2) = (pxv0v1v2[1], pxv0v1v2[2], pxv0v1v2[3]);

            let (v0_to_v1, v2_to_v1) = (v0 - v1, v2 - v1);
            let discrim = f32::sqrt(v1 * v1 - v0 * v2);
            let denom = 1.0 / (v0_to_v1 + v2_to_v1);

            t0 = (v0_to_v1 + discrim) * denom;
            t1 = (v0_to_v1 - discrim) * denom;
        }

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
