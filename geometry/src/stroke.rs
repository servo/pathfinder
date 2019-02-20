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
        for point_index in 0..(self.input.points.len() as u32) {
            let mut prev_point_index = self.input.prev_point_index_of(point_index);
            while prev_point_index != point_index &&
                    self.input.position_of(prev_point_index) ==
                    self.input.position_of(point_index) {
                prev_point_index = self.input.prev_point_index_of(prev_point_index);
            }

            let mut next_point_index = self.input.next_point_index_of(point_index);
            while next_point_index != point_index &&
                    self.input.position_of(next_point_index) ==
                    self.input.position_of(point_index) {
                next_point_index = self.input.next_point_index_of(next_point_index);
            }

            let prev_line_segment = LineSegmentF32::new(&self.input.position_of(prev_point_index),
                                                        &self.input.position_of(point_index));
            let next_line_segment = LineSegmentF32::new(&self.input.position_of(point_index),
                                                        &self.input.position_of(next_point_index));
            let prev_offset_line_segment = prev_line_segment.offset(self.radius);
            let next_offset_line_segment = next_line_segment.offset(self.radius);

            let new_position;
            match prev_offset_line_segment.intersection_t(&next_offset_line_segment) {
                None => new_position = self.input.position_of(point_index),
                Some(t) => new_position = prev_offset_line_segment.sample(t),
            }

            self.output.push_point(new_position, self.input.flags[point_index as usize], true);
        }
    }

    fn offset_backward(&mut self) {
        for point_index in (0..(self.input.points.len() as u32)).rev() {
            let mut prev_point_index = self.input.prev_point_index_of(point_index);
            while prev_point_index != point_index &&
                    self.input.position_of(prev_point_index) ==
                    self.input.position_of(point_index) {
                prev_point_index = self.input.prev_point_index_of(prev_point_index);
            }

            let mut next_point_index = self.input.next_point_index_of(point_index);
            while next_point_index != point_index &&
                    self.input.position_of(next_point_index) ==
                    self.input.position_of(point_index) {
                next_point_index = self.input.next_point_index_of(next_point_index);
            }

            let prev_line_segment = LineSegmentF32::new(&self.input.position_of(prev_point_index),
                                                        &self.input.position_of(point_index));
            let next_line_segment = LineSegmentF32::new(&self.input.position_of(point_index),
                                                        &self.input.position_of(next_point_index));
            let prev_offset_line_segment = prev_line_segment.offset(-self.radius);
            let next_offset_line_segment = next_line_segment.offset(-self.radius);

            let new_position;
            match prev_offset_line_segment.intersection_t(&next_offset_line_segment) {
                None => new_position = self.input.position_of(point_index),
                Some(t) => new_position = prev_offset_line_segment.sample(t),
            }

            self.output.push_point(new_position, self.input.flags[point_index as usize], true);
        }
    }
}
