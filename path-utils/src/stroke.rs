// pathfinder/path-utils/src/stroke.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Utilities for converting path strokes to fills.

use euclid::Vector2D;
use lyon_geom::{LineSegment, QuadraticBezierSegment};
use lyon_path::PathEvent;
use lyon_path::iterator::{PathEvents, PathIterator};
use std::u32;

use segments::{Segment, SegmentIter};

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

/*pub fn stroke_to_fill<I, F>(path: I, style: StrokeStyle, mut sink: F)
                            where I: PathIterator, F: FnMut(&PathEvent) {
    loop {
        match path.next() {
            None => {
                
            }
        }
    }
}*/

pub struct StrokeToFillIter<I> where I: PathIterator {
    inner: SegmentIter<I>,
    subpath: Vec<Segment>,
    stack: Vec<PathEvent>,
    state: StrokeToFillState,
    style: StrokeStyle,
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
                    None | Some(Segment::EndSubpath) => {
                        if self.subpath.is_empty() {
                            return None
                        }
                        self.state = StrokeToFillState::Backward;
                        return self.next()
                    }
                    Some(segment) => {
                        self.subpath.push(segment);
                        segment
                    }
                }
            }
            StrokeToFillState::Backward => {
                match self.subpath.pop() {
                    None | Some(Segment::EndSubpath) => {
                        self.state = StrokeToFillState::Forward;
                        return Some(PathEvent::Close)
                    }
                    Some(segment) => segment.flip(),
                }
            }
        };

        next_segment.offset(self.style.width, |offset_segment| {
            match *offset_segment {
                Segment::EndSubpath => unreachable!(),
                Segment::Line(ref offset_segment) => {
                    if self.subpath.len() == 1 && self.state == StrokeToFillState::Forward {
                        self.stack.push(PathEvent::MoveTo(offset_segment.from))
                    } else if self.stack.is_empty() {
                        self.stack.push(PathEvent::LineTo(offset_segment.from))
                    }
                    self.stack.push(PathEvent::LineTo(offset_segment.to))
                }
                Segment::Quadratic(ref offset_segment) => {
                    if self.subpath.len() == 1 && self.state == StrokeToFillState::Forward {
                        self.stack.push(PathEvent::MoveTo(offset_segment.from))
                    } else if self.stack.is_empty() {
                        self.stack.push(PathEvent::LineTo(offset_segment.from))
                    }
                    self.stack.push(PathEvent::QuadraticTo(offset_segment.ctrl, offset_segment.to))
                }
                Segment::Cubic(ref offset_segment) => {
                    if self.subpath.len() == 1 && self.state == StrokeToFillState::Forward {
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
