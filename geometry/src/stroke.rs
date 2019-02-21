// pathfinder/geometry/src/stroke.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Utilities for converting path strokes to fills.

use crate::basic::line_segment::LineSegmentF32;
use crate::basic::rect::RectF32;
use crate::outline::{Contour, Outline};
use crate::segment::Segment as SegmentPF3;
use std::mem;

pub struct OutlineStrokeToFill {
    pub outline: Outline,
    pub radius: f32,
}

impl OutlineStrokeToFill {
    #[inline]
    pub fn new(outline: Outline, radius: f32) -> OutlineStrokeToFill {
        OutlineStrokeToFill { outline, radius }
    }

    #[inline]
    pub fn offset(&mut self) {
        let mut new_bounds = None;
        for contour in &mut self.outline.contours {
            let input = mem::replace(contour, Contour::new());
            let mut contour_stroke_to_fill =
                ContourStrokeToFill::new(input, Contour::new(), self.radius);
            contour_stroke_to_fill.offset_forward();
            contour_stroke_to_fill.offset_backward();
            *contour = contour_stroke_to_fill.output;
            contour.update_bounds(&mut new_bounds);
        }

        self.outline.bounds = new_bounds.unwrap_or_else(|| RectF32::default());
    }
}

struct ContourStrokeToFill {
    input: Contour,
    output: Contour,
    radius: f32,
}

impl ContourStrokeToFill {
    #[inline]
    fn new(input: Contour, output: Contour, radius: f32) -> ContourStrokeToFill {
        ContourStrokeToFill { input, output, radius }
    }

    fn offset_forward(&mut self) {
        for segment in self.input.iter() {
            segment.offset(self.radius, &mut self.output);
        }
    }

    fn offset_backward(&mut self) {
        // FIXME(pcwalton)
        let mut segments: Vec<_> = self.input.iter().map(|segment| segment.reversed()).collect();
        segments.reverse();
        for segment in &segments {
            segment.offset(self.radius, &mut self.output);
        }
    }
}

trait Offset {
    fn offset(&self, distance: f32, contour: &mut Contour);
}

impl Offset for SegmentPF3 {
    fn offset(&self, distance: f32, contour: &mut Contour) {
        if self.is_line() {
            contour.push_full_segment(&SegmentPF3::line(&self.baseline.offset(distance)), true);
            return;
        }

        if self.is_quadratic() {
            let mut segment_0 = LineSegmentF32::new(&self.baseline.from(), &self.ctrl.from());
            let mut segment_1 = LineSegmentF32::new(&self.ctrl.from(),     &self.baseline.to());
            segment_0 = segment_0.offset(distance);
            segment_1 = segment_1.offset(distance);
            let ctrl = match segment_0.intersection_t(&segment_1) {
                Some(t) => segment_0.sample(t),
                None => segment_0.to().lerp(segment_1.from(), 0.5),
            };
            let baseline = LineSegmentF32::new(&segment_0.from(), &segment_1.to());
            contour.push_full_segment(&SegmentPF3::quadratic(&baseline, &ctrl), true);
            return;
        }

        debug_assert!(self.is_cubic());

        if self.baseline.from() == self.ctrl.from() {
            let mut segment_0 = LineSegmentF32::new(&self.baseline.from(), &self.ctrl.to());
            let mut segment_1 = LineSegmentF32::new(&self.ctrl.to(),     &self.baseline.to());
            segment_0 = segment_0.offset(distance);
            segment_1 = segment_1.offset(distance);
            let ctrl = match segment_0.intersection_t(&segment_1) {
                Some(t) => segment_0.sample(t),
                None => segment_0.to().lerp(segment_1.from(), 0.5),
            };
            let baseline = LineSegmentF32::new(&segment_0.from(), &segment_1.to());
            let ctrl = LineSegmentF32::new(&segment_0.from(), &ctrl);
            contour.push_full_segment(&SegmentPF3::cubic(&baseline, &ctrl), true);
            return;
        }

        if self.ctrl.to() == self.baseline.to() {
            let mut segment_0 = LineSegmentF32::new(&self.baseline.from(), &self.ctrl.from());
            let mut segment_1 = LineSegmentF32::new(&self.ctrl.from(),     &self.baseline.to());
            segment_0 = segment_0.offset(distance);
            segment_1 = segment_1.offset(distance);
            let ctrl = match segment_0.intersection_t(&segment_1) {
                Some(t) => segment_0.sample(t),
                None => segment_0.to().lerp(segment_1.from(), 0.5),
            };
            let baseline = LineSegmentF32::new(&segment_0.from(), &segment_1.to());
            let ctrl = LineSegmentF32::new(&ctrl, &segment_1.to());
            contour.push_full_segment(&SegmentPF3::cubic(&baseline, &ctrl), true);
            return;
        }

        let mut segment_0 = LineSegmentF32::new(&self.baseline.from(), &self.ctrl.from());
        let mut segment_1 = LineSegmentF32::new(&self.ctrl.from(),     &self.ctrl.to());
        let mut segment_2 = LineSegmentF32::new(&self.ctrl.to(),       &self.baseline.to());
        segment_0 = segment_0.offset(distance);
        segment_1 = segment_1.offset(distance);
        segment_2 = segment_2.offset(distance);
        let (ctrl_0, ctrl_1) = match (segment_0.intersection_t(&segment_1),
                                      segment_1.intersection_t(&segment_2)) {
            (Some(t0), Some(t1)) => (segment_0.sample(t0), segment_1.sample(t1)),
            _ => {
                (segment_0.to().lerp(segment_1.from(), 0.5),
                 segment_1.to().lerp(segment_2.from(), 0.5))
            }
        };
        let baseline = LineSegmentF32::new(&segment_0.from(), &segment_2.to());
        let ctrl = LineSegmentF32::new(&ctrl_0, &ctrl_1);
        contour.push_full_segment(&SegmentPF3::cubic(&baseline, &ctrl), true);
    }
}
