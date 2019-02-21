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
use crate::segments::{Segment, SegmentIter};
use lyon_path::PathEvent;
use lyon_path::iterator::PathIterator;
use std::mem;

#[derive(Clone, Copy, Debug)]
pub struct StrokeStyle {
    pub width: f32,
}

impl StrokeStyle {
    #[inline]
    pub fn new(width: f32) -> StrokeStyle {
        StrokeStyle {
            width: width,
        }
    }
}

pub struct StrokeToFillIter<I> where I: PathIterator {
    inner: SegmentIter<I>,
    subpath: Vec<Segment>,
    stack: Vec<PathEvent>,
    state: StrokeToFillState,
    style: StrokeStyle,
    first_point_in_subpath: bool,
}

impl<I> StrokeToFillIter<I> where I: PathIterator {
    #[inline]
    pub fn new(inner: I, style: StrokeStyle) -> StrokeToFillIter<I> {
        StrokeToFillIter {
            inner: SegmentIter::new(inner),
            subpath: vec![],
            stack: vec![],
            state: StrokeToFillState::Forward,
            style: style,
            first_point_in_subpath: true,
        }
    }
}

impl<I> Iterator for StrokeToFillIter<I> where I: PathIterator {
    type Item = PathEvent;

    // TODO(pcwalton): Support miter and round joins. This will probably require the inner iterator
    // to be `Peekable`, I guess.
    fn next(&mut self) -> Option<PathEvent> {
        // If we have path events queued, return the latest.
        if let Some(path_event) = self.stack.pop() {
            return Some(path_event)
        }

        // Fetch the next segment.
        let next_segment = match self.state {
            StrokeToFillState::Forward => {
                match self.inner.next() {
                    None | Some(Segment::EndSubpath(false)) => {
                        if self.subpath.is_empty() {
                            return None
                        }
                        self.state = StrokeToFillState::Backward;
                        return self.next()
                    }
                    Some(Segment::EndSubpath(true)) => {
                        if self.subpath.is_empty() {
                            return None
                        }
                        self.state = StrokeToFillState::Backward;
                        self.first_point_in_subpath = true;
                        return Some(PathEvent::Close)
                    }
                    Some(segment) => {
                        self.subpath.push(segment);
                        segment
                    }
                }
            }
            StrokeToFillState::Backward => {
                match self.subpath.pop() {
                    None | Some(Segment::EndSubpath(_)) => {
                        self.state = StrokeToFillState::Forward;
                        self.first_point_in_subpath = true;
                        return Some(PathEvent::Close)
                    }
                    Some(segment) => segment.flip(),
                }
            }
        };

        next_segment.offset(self.style.width * 0.5, |offset_segment| {
            match *offset_segment {
                Segment::EndSubpath(_) => unreachable!(),
                Segment::Line(ref offset_segment) => {
                    if self.first_point_in_subpath {
                        self.first_point_in_subpath = false;
                        self.stack.push(PathEvent::MoveTo(offset_segment.from))
                    } else if self.stack.is_empty() {
                        self.stack.push(PathEvent::LineTo(offset_segment.from))
                    }
                    self.stack.push(PathEvent::LineTo(offset_segment.to))
                }
                Segment::Quadratic(ref offset_segment) => {
                    if self.first_point_in_subpath {
                        self.first_point_in_subpath = false;
                        self.stack.push(PathEvent::MoveTo(offset_segment.from))
                    } else if self.stack.is_empty() {
                        self.stack.push(PathEvent::LineTo(offset_segment.from))
                    }
                    self.stack.push(PathEvent::QuadraticTo(offset_segment.ctrl, offset_segment.to))
                }
                Segment::Cubic(ref offset_segment) => {
                    if self.first_point_in_subpath {
                        self.first_point_in_subpath = false;
                        self.stack.push(PathEvent::MoveTo(offset_segment.from))
                    } else if self.stack.is_empty() {
                        self.stack.push(PathEvent::LineTo(offset_segment.from))
                    }
                    self.stack.push(PathEvent::CubicTo(offset_segment.ctrl1,
                                                       offset_segment.ctrl2,
                                                       offset_segment.to))
                }
            }
        });

        self.stack.reverse();
        return self.next()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum StrokeToFillState {
    Forward,
    Backward,
}

// Pathfinder 3

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
