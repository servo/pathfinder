// pathfinder/path-utils/src/segments.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Returns each segment of a path.

use euclid::approxeq::ApproxEq;
use euclid::{Point2D, Vector2D};
use lyon_geom::{CubicBezierSegment, LineSegment, QuadraticBezierSegment};
use lyon_path::iterator::PathIterator;
use lyon_path::PathEvent;

pub struct SegmentIter<I> where I: PathIterator {
    inner: I,
    subpath_done: bool,
    is_first_subpath: bool,
}

impl<I> SegmentIter<I> where I: PathIterator {
    #[inline]
    pub fn new(inner: I) -> SegmentIter<I> {
        SegmentIter {
            inner: inner,
            subpath_done: false,
            is_first_subpath: true,
        }
    }
}

impl<I> Iterator for SegmentIter<I> where I: PathIterator {
    type Item = Segment;

    fn next(&mut self) -> Option<Segment> {
        if self.subpath_done {
            self.subpath_done = false;
            return Some(Segment::EndSubpath)
        }

        let current_point = self.inner.get_state().current;

        match self.inner.next() {
            None => None,
            Some(PathEvent::Close) => {
                let state = self.inner.get_state();
                if state.first == current_point {
                    return Some(Segment::EndSubpath)
                }
                self.subpath_done = true;
                Some(Segment::Line(LineSegment {
                    from: state.current,
                    to: state.first,
                }))
            }
            Some(PathEvent::MoveTo(_)) => {
                if self.is_first_subpath {
                    self.is_first_subpath = false;
                    return self.next();
                }

                self.is_first_subpath = false;
                Some(Segment::EndSubpath)
            }
            Some(PathEvent::LineTo(to)) => {
                Some(Segment::Line(LineSegment {
                    from: current_point,
                    to: to,
                }))
            }
            Some(PathEvent::QuadraticTo(ctrl, to)) => {
                Some(Segment::Quadratic(QuadraticBezierSegment {
                    from: current_point,
                    ctrl: ctrl,
                    to: to,
                }))
            }
            Some(PathEvent::CubicTo(ctrl1, ctrl2, to)) => {
                Some(Segment::Cubic(CubicBezierSegment {
                    from: current_point,
                    ctrl1: ctrl1,
                    ctrl2: ctrl2,
                    to: to,
                }))
            }
            Some(PathEvent::Arc(..)) => {
                panic!("SegmentIter doesn't support cubics and arcs yet!")
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum Segment {
    Line(LineSegment<f32>),
    Quadratic(QuadraticBezierSegment<f32>),
    Cubic(CubicBezierSegment<f32>),
    EndSubpath,
}

impl Segment {
    pub fn flip(&self) -> Segment {
        match *self {
            Segment::EndSubpath => Segment::EndSubpath,
            Segment::Line(line_segment) => Segment::Line(line_segment.flip()),
            Segment::Quadratic(quadratic_segment) => Segment::Quadratic(quadratic_segment.flip()),
            Segment::Cubic(cubic_segment) => Segment::Cubic(cubic_segment.flip()),
        }
    }

    pub fn offset<F>(&self, distance: f32, mut sink: F) where F: FnMut(&Segment) {
        match *self {
            Segment::EndSubpath => {}
            Segment::Line(ref segment) => {
                sink(&Segment::Line(offset_line_segment(segment, distance)))
            }

            Segment::Quadratic(ref quadratic_segment) => {
                // This is the Tiller & Hanson 1984 algorithm for approximate Bézier offset curves.
                // We take the cage (i.e. convex hull) and push its edges out along their normals,
                // then recompute the control point with a miter join.
                let line_segments = (LineSegment {
                    from: quadratic_segment.from,
                    to: quadratic_segment.ctrl,
                }, LineSegment {
                    from: quadratic_segment.ctrl,
                    to: quadratic_segment.to,
                });

                // Miter join.
                let (from, intersection, to) = match offset_and_join_line_segments(line_segments.0,
                                                                                   line_segments.1,
                                                                                   distance) {
                    None => return sink(self),
                    Some(intersection) => intersection,
                };

                sink(&Segment::Quadratic(QuadraticBezierSegment {
                    from: from,
                    ctrl: intersection,
                    to: to,
                }))
            }

            Segment::Cubic(ref cubic_segment) if points_overlap(&cubic_segment.from,
                                                                &cubic_segment.ctrl1) => {
                // As above.
                let line_segments = (LineSegment {
                    from: cubic_segment.from,
                    to: cubic_segment.ctrl2,
                }, LineSegment {
                    from: cubic_segment.ctrl2,
                    to: cubic_segment.to,
                });

                // Miter join.
                let (from, intersection, to) = match offset_and_join_line_segments(line_segments.0,
                                                                                   line_segments.1,
                                                                                   distance) {
                    None => return sink(self),
                    Some(intersection) => intersection,
                };

                sink(&Segment::Cubic(CubicBezierSegment {
                    from: from,
                    ctrl1: from,
                    ctrl2: intersection,
                    to: to,
                }))
            }

            Segment::Cubic(ref cubic_segment) if points_overlap(&cubic_segment.ctrl2,
                                                                &cubic_segment.to) => {
                // As above.
                let line_segments = (LineSegment {
                    from: cubic_segment.from,
                    to: cubic_segment.ctrl1,
                }, LineSegment {
                    from: cubic_segment.ctrl1,
                    to: cubic_segment.to,
                });

                // Miter join.
                let (from, intersection, to) = match offset_and_join_line_segments(line_segments.0,
                                                                                   line_segments.1,
                                                                                   distance) {
                    None => return sink(self),
                    Some(intersection) => intersection,
                };

                sink(&Segment::Cubic(CubicBezierSegment {
                    from: from,
                    ctrl1: intersection,
                    ctrl2: to,
                    to: to,
                }))
            }

            Segment::Cubic(ref cubic_segment) => {
                // As above.
                let line_segments = (LineSegment {
                    from: cubic_segment.from,
                    to: cubic_segment.ctrl1,
                }, LineSegment {
                    from: cubic_segment.ctrl1,
                    to: cubic_segment.ctrl2,
                }, LineSegment {
                    from: cubic_segment.ctrl2,
                    to: cubic_segment.to,
                });

                let (from, intersection_0, _) =
                        match offset_and_join_line_segments(line_segments.0,
                                                            line_segments.1,
                                                            distance) {
                            None => return sink(self),
                            Some(intersection) => intersection,
                        };
                let (_, intersection_1, to) = match offset_and_join_line_segments(line_segments.1,
                                                                                  line_segments.2,
                                                                                  distance) {
                    None => return sink(self),
                    Some(intersection) => intersection,
                };

                sink(&Segment::Cubic(CubicBezierSegment {
                    from: from,
                    ctrl1: intersection_0,
                    ctrl2: intersection_1,
                    to: to,
                }))
            }
        }
    }
}

fn offset_line_segment(segment: &LineSegment<f32>, distance: f32) -> LineSegment<f32> {
    let mut segment = *segment;
    let vector = segment.to_vector();
    if vector.square_length() < f32::approx_epsilon() {
        return segment
    }
    let tangent = vector.normalize() * distance;
    segment.translate(Vector2D::new(-tangent.y, tangent.x))
}

// Performs a miter join.
fn offset_and_join_line_segments(mut line_segment_0: LineSegment<f32>,
                                 mut line_segment_1: LineSegment<f32>,
                                 distance: f32)
                                 -> Option<(Point2D<f32>, Point2D<f32>, Point2D<f32>)> {
    line_segment_0 = offset_line_segment(&line_segment_0, distance);
    line_segment_1 = offset_line_segment(&line_segment_1, distance);
    match line_segment_0.to_line().intersection(&line_segment_1.to_line()) {
        None => None,
        Some(intersection) => Some((line_segment_0.from, intersection, line_segment_1.to)),
    }
}

fn points_overlap(a: &Point2D<f32>, b: &Point2D<f32>) -> bool {
    a.x.approx_eq(&b.x) && a.y.approx_eq(&b.y)
}
