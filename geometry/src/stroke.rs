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

const TOLERANCE: f32 = 0.01;

pub struct OutlineStrokeToFill {
    pub outline: Outline,
    pub stroke_width: f32,
}

impl OutlineStrokeToFill {
    #[inline]
    pub fn new(outline: Outline, stroke_width: f32) -> OutlineStrokeToFill {
        OutlineStrokeToFill { outline, stroke_width }
    }

    #[inline]
    pub fn offset(&mut self) {
        let mut new_bounds = None;
        for contour in &mut self.outline.contours {
            let input = mem::replace(contour, Contour::new());
            let mut contour_stroke_to_fill =
                ContourStrokeToFill::new(input, Contour::new(), self.stroke_width * 0.5);
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
    fn offset_once(&self, distance: f32) -> Self;
    fn error_is_within_tolerance(&self, other: &SegmentPF3, distance: f32) -> bool;
}

impl Offset for SegmentPF3 {
    fn offset(&self, distance: f32, contour: &mut Contour) {
        if self.baseline.square_length() < TOLERANCE * TOLERANCE {
            contour.push_full_segment(self, true);
            return;
        }

        let candidate = self.offset_once(distance);
        if self.error_is_within_tolerance(&candidate, distance) {
            contour.push_full_segment(&candidate, true);
            return;
        }

        debug!("--- SPLITTING ---");
        debug!("... PRE-SPLIT: {:?}", self);
        let (before, after) = self.split(0.5);
        debug!("... AFTER-SPLIT: {:?} {:?}", before, after);
        before.offset(distance, contour);
        after.offset(distance, contour);
    }

    fn offset_once(&self, distance: f32) -> SegmentPF3 {
        if self.is_line() {
            return SegmentPF3::line(&self.baseline.offset(distance));
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
            return SegmentPF3::quadratic(&baseline, &ctrl);
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
            return SegmentPF3::cubic(&baseline, &ctrl);
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
            return SegmentPF3::cubic(&baseline, &ctrl);
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
        SegmentPF3::cubic(&baseline, &ctrl)
    }

    fn error_is_within_tolerance(&self, other: &SegmentPF3, distance: f32) -> bool {
        let (mut min, mut max) = (f32::abs(distance) - TOLERANCE, f32::abs(distance) + TOLERANCE);
        min = if min <= 0.0 { 0.0 } else { min * min };
        max = if max <= 0.0 { 0.0 } else { max * max };

        for t_num in 0..(SAMPLE_COUNT + 1) {
            let t = t_num as f32 / SAMPLE_COUNT as f32;
            // FIXME(pcwalton): Use signed distance!
            let (this_p, other_p) = (self.sample(t), other.sample(t));
            let vector = this_p - other_p;
            let square_distance = vector.square_length();
            debug!("this_p={:?} other_p={:?} vector={:?} sqdist={:?} min={:?} max={:?}",
                   this_p, other_p, vector, square_distance, min, max);
            if square_distance < min || square_distance > max {
                return false;
            }
        }

        return true;

        const SAMPLE_COUNT: u32 = 16;
    }
}
