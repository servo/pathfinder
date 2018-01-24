// pathfinder/partitioner/src/monotonic.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use arrayvec::ArrayVec;
use lyon_geom::{Monotonic, QuadraticBezierSegment};
use lyon_path::iterator::PathIterator;
use lyon_path::{PathEvent, PathState};

/// FIXME(pcwalton): Can this actually return 4 segments? It shouldn't geometrically, but I don't
/// know whether floating point error will cause it to happen.
pub fn quadratic_to_monotonic(segment: &QuadraticBezierSegment<f32>)
                              -> ArrayVec<[Monotonic<QuadraticBezierSegment<f32>>; 4]> {
    // Split at X tangent.
    let mut worklist: ArrayVec<[QuadraticBezierSegment<f32>; 2]> = ArrayVec::new();
    match segment.find_local_x_extremum() {
        None => worklist.push(*segment),
        Some(t) => {
            let subsegments = segment.split(t);
            worklist.push(subsegments.0);
            worklist.push(subsegments.1);
        }
    }

    // Split at Y tangent.
    let mut results: ArrayVec<[Monotonic<QuadraticBezierSegment<f32>>; 4]> = ArrayVec::new();
    for segment in worklist {
        match segment.find_local_y_extremum() {
            None => results.push(segment.assume_monotonic()),
            Some(t) => {
                let subsegments = segment.split(t);
                results.push(subsegments.0.assume_monotonic());
                results.push(subsegments.1.assume_monotonic());
            }
        }
    }

    results
}

pub struct MonotonicPathIterator<I> where I: PathIterator {
    inner: I,
    state: PathState,
    queue: ArrayVec<[Monotonic<QuadraticBezierSegment<f32>>; 4]>,
}

impl<I> MonotonicPathIterator<I> where I: PathIterator {
    #[inline]
    pub fn new(inner: I) -> MonotonicPathIterator<I> {
        MonotonicPathIterator {
            inner: inner,
            state: PathState::new(),
            queue: ArrayVec::new(),
        }
    }
}

impl<I> Iterator for MonotonicPathIterator<I> where I: PathIterator {
    type Item = PathEvent;

    fn next(&mut self) -> Option<PathEvent> {
        if let Some(segment) = self.queue.pop() {
            let event = PathEvent::QuadraticTo(segment.segment().ctrl, segment.segment().to);
            self.state.path_event(event);
            return Some(event)
        }

        let event = match self.inner.next() {
            None => return None,
            Some(event) => event,
        };

        self.state.path_event(event);

        match event {
            PathEvent::CubicTo(..) | PathEvent::Arc(..) => {
                panic!("MonotonicPathIterator: Convert cubics and arcs to quadratics first!")
            }
            PathEvent::Close => Some(PathEvent::Close),
            PathEvent::MoveTo(to) => Some(PathEvent::MoveTo(to)),
            PathEvent::LineTo(to) => Some(PathEvent::LineTo(to)),
            PathEvent::QuadraticTo(ctrl, to) => {
                let segment = QuadraticBezierSegment {
                    from: self.state.current,
                    ctrl: ctrl,
                    to: to,
                };
                self.queue = quadratic_to_monotonic(&segment);
                self.queue.reverse();
                let first_segment = self.queue.pop().unwrap();
                let segment_event = PathEvent::QuadraticTo(first_segment.segment().ctrl,
                                                           first_segment.segment().to);
                self.state.path_event(segment_event);
                Some(segment_event)
            }
        }
    }
}

impl<I> PathIterator for MonotonicPathIterator<I> where I: PathIterator {
    #[inline]
    fn get_state(&self) -> &PathState {
        &self.state
    }
}
