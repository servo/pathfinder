// pathfinder/geometry/src/monotonic.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Converts paths to monotonically increasing/decreasing segments in Y.

use crate::segment::{Segment, SegmentKind};
use arrayvec::ArrayVec;

pub struct MonotonicConversionIter<I>
where
    I: Iterator<Item = Segment>,
{
    iter: I,
    buffer: ArrayVec<[Segment; 2]>,
}

impl<I> Iterator for MonotonicConversionIter<I>
where
    I: Iterator<Item = Segment>,
{
    type Item = Segment;

    #[inline]
    fn next(&mut self) -> Option<Segment> {
        if let Some(segment) = self.buffer.pop() {
            return Some(segment);
        }

        let segment = self.iter.next()?;
        match segment.kind {
            SegmentKind::None => self.next(),
            SegmentKind::Line => Some(segment),
            SegmentKind::Cubic => self.handle_cubic(&segment),
            SegmentKind::Quadratic => {
                // TODO(pcwalton): Don't degree elevate!
                self.handle_cubic(&segment.to_cubic())
            }
        }
    }
}

impl<I> MonotonicConversionIter<I>
where
    I: Iterator<Item = Segment>,
{
    #[inline]
    pub fn new(iter: I) -> MonotonicConversionIter<I> {
        MonotonicConversionIter {
            iter,
            buffer: ArrayVec::new(),
        }
    }

    pub fn handle_cubic(&mut self, segment: &Segment) -> Option<Segment> {
        match segment.as_cubic_segment().y_extrema() {
            (Some(t0), Some(t1)) => {
                let (segments_01, segment_2) = segment.as_cubic_segment().split(t1);
                self.buffer.push(segment_2);
                let (segment_0, segment_1) = segments_01.as_cubic_segment().split(t0 / t1);
                self.buffer.push(segment_1);
                Some(segment_0)
            }
            (Some(t0), None) | (None, Some(t0)) => {
                let (segment_0, segment_1) = segment.as_cubic_segment().split(t0);
                self.buffer.push(segment_1);
                Some(segment_0)
            }
            (None, None) => Some(*segment),
        }
    }
}
