// pathfinder/geometry/src/clip.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::line_segment::LineSegmentF32;
use crate::outline::{Contour, PointFlags};
use crate::point::{Point2DF32, Point3DF32};
use crate::segment::Segment;
use crate::simd::F32x4;
use crate::util::lerp;
use euclid::Rect;
use lyon_path::PathEvent;
use smallvec::SmallVec;
use std::mem;

pub struct RectClipper<'a> {
    clip_rect: Rect<f32>,
    subject: &'a [PathEvent],
}

impl<'a> RectClipper<'a> {
    pub fn new<'aa>(clip_rect: &Rect<f32>, subject: &'aa [PathEvent]) -> RectClipper<'aa> {
        RectClipper {
            clip_rect: *clip_rect,
            subject,
        }
    }

    pub fn clip(&self) -> Vec<PathEvent> {
        let mut output = self.subject.to_vec();
        self.clip_against(Edge::left(&self.clip_rect), &mut output);
        self.clip_against(Edge::top(&self.clip_rect), &mut output);
        self.clip_against(Edge::right(&self.clip_rect), &mut output);
        self.clip_against(Edge::bottom(&self.clip_rect), &mut output);
        output
    }

    fn clip_against(&self, edge: Edge, output: &mut Vec<PathEvent>) {
        let (mut from, mut path_start, mut first_point) = (Point2DF32::default(), None, false);
        let input = mem::replace(output, vec![]);
        for event in input {
            let to = match event {
                PathEvent::MoveTo(to) => {
                    let to = Point2DF32::from_euclid(to);
                    path_start = Some(to);
                    from = to;
                    first_point = true;
                    continue
                }
                PathEvent::Close => {
                    match path_start {
                        None => continue,
                        Some(path_start) => path_start,
                    }
                }
                PathEvent::LineTo(to) |
                PathEvent::QuadraticTo(_, to) |
                PathEvent::CubicTo(_, _, to) => Point2DF32::from_euclid(to),
                PathEvent::Arc(..) => panic!("Arcs unsupported!"),
            };

            if edge.point_is_inside(&to) {
                if !edge.point_is_inside(&from) {
                    let line_segment = LineSegmentF32::new(&from, &to);
                    if let Some(intersection) = edge.line_intersection(&line_segment) {
                        add_line(&intersection, output, &mut first_point);
                    }
                }
                add_line(&to, output, &mut first_point);
            } else if edge.point_is_inside(&from) {
                let line_segment = LineSegmentF32::new(&from, &to);
                if let Some(intersection) = edge.line_intersection(&line_segment) {
                    add_line(&intersection, output, &mut first_point);
                }
            }

            from = to;

            if let PathEvent::Close = event {
                output.push(PathEvent::Close);
                path_start = None;
            }
        }

        fn add_line(to: &Point2DF32, output: &mut Vec<PathEvent>, first_point: &mut bool) {
            let to = to.as_euclid();
            if *first_point {
                output.push(PathEvent::MoveTo(to));
                *first_point = false;
            } else {
                output.push(PathEvent::LineTo(to));
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Edge(LineSegmentF32);

impl Edge {
    #[inline]
    fn left(rect: &Rect<f32>) -> Edge {
        Edge(LineSegmentF32::new(&Point2DF32::from_euclid(rect.bottom_left()),
                                 &Point2DF32::from_euclid(rect.origin)))
    }

    #[inline]
    fn top(rect: &Rect<f32>) -> Edge {
        Edge(LineSegmentF32::new(&Point2DF32::from_euclid(rect.origin),
                                 &Point2DF32::from_euclid(rect.top_right())))
    }

    #[inline]
    fn right(rect: &Rect<f32>) -> Edge {
        Edge(LineSegmentF32::new(&Point2DF32::from_euclid(rect.top_right()),
                                 &Point2DF32::from_euclid(rect.bottom_right())))
    }

    #[inline]
    fn bottom(rect: &Rect<f32>) -> Edge {
        Edge(LineSegmentF32::new(&Point2DF32::from_euclid(rect.bottom_right()),
                                 &Point2DF32::from_euclid(rect.bottom_left())))
    }

    #[inline]
    fn point_is_inside(&self, point: &Point2DF32) -> bool {
        (self.0.to() - self.0.from()).det(*point - self.0.from()) >= 0.0
    }

    fn trivially_test_segment(&self, segment: &Segment) -> EdgeRelativeLocation {
        let from_inside = self.point_is_inside(&segment.baseline.from());
        //println!("point {:?} inside {:?}: {:?}", segment.baseline.from(), self, from_inside);
        if from_inside != self.point_is_inside(&segment.baseline.to()) {
            return EdgeRelativeLocation::Intersecting;
        }
        if !segment.is_line() {
            if from_inside != self.point_is_inside(&segment.ctrl.from()) {
                return EdgeRelativeLocation::Intersecting;
            }
            if !segment.is_quadratic() {
                if from_inside != self.point_is_inside(&segment.ctrl.to()) {
                    return EdgeRelativeLocation::Intersecting;
                }
            }
        }
        if from_inside { EdgeRelativeLocation::Inside } else { EdgeRelativeLocation::Outside }
    }

    fn line_intersection(&self, other: &LineSegmentF32) -> Option<Point2DF32> {
        let (this_line, other_line) = (self.0.line_coords(), other.line_coords());
        let result = this_line.cross(other_line);
        let z = result[2];
        if z == 0.0 {
            return None;
        }
        let result = Point2DF32((result * F32x4::splat(1.0 / z)).xyxy());
        if result.x() <= other.min_x() || result.x() >= other.max_x() {
            None
        } else {
            Some(result)
        }
    }

    fn split_segment(&self, segment: &Segment) -> Option<(Segment, Segment)> {
        if segment.is_line() {
            return self.split_line_segment(segment);
        }

        let mut segment = *segment;
        if segment.is_quadratic() {
            segment = segment.to_cubic();
        }

        self.intersect_cubic_segment(&segment, 0.0, 1.0).map(|t| {
            self.fixup_clipped_segments(&segment.as_cubic_segment().split(t))
        })
    }

    fn split_line_segment(&self, segment: &Segment) -> Option<(Segment, Segment)> {
        let intersection = self.line_intersection(&segment.baseline)?;
        Some((Segment::line(&LineSegmentF32::new(&segment.baseline.from(), &intersection)),
              Segment::line(&LineSegmentF32::new(&intersection, &segment.baseline.to()))))
    }

    fn intersect_cubic_segment(&self, segment: &Segment, t_min: f32, t_max: f32) -> Option<f32> {
        /*println!("... intersect_cubic_segment({:?}, {:?}, t=({}, {}))",
                 self, segment, t_min, t_max);*/
        let t_mid = lerp(t_min, t_max, 0.5);
        if t_max - t_min < 0.001 {
            return Some(t_mid);
        }

        let (prev_segment, next_segment) = segment.as_cubic_segment().split(t_mid);

        let prev_cubic_segment = prev_segment.as_cubic_segment();
        let next_cubic_segment = next_segment.as_cubic_segment();

        if self.line_intersection(&prev_segment.baseline).is_some() {
            self.intersect_cubic_segment(segment, t_min, t_mid)
        } else if self.line_intersection(&next_segment.baseline).is_some() {
            self.intersect_cubic_segment(segment, t_mid, t_max)
        } else if prev_segment.baseline.to() == self.0.from() ||
                prev_segment.baseline.to() == self.0.to() {
            Some(t_mid)
        } else {
            None
        }
    }

    fn fixup_clipped_segments(&self, segment: &(Segment, Segment)) -> (Segment, Segment) {
        let (mut prev, mut next) = *segment;
        let point = prev.baseline.to();

        let line_coords = self.0.line_coords();
        let (a, b, c) = (line_coords[0], line_coords[1], line_coords[2]);
        let denom = 1.0 / (a * a + b * b);
        let factor = b * point.x() - a * point.y();
        let snapped = Point2DF32::new(b * factor - a * c, a * -factor - b * c) *
            Point2DF32::splat(denom);

        prev.baseline.set_to(&snapped);
        next.baseline.set_from(&snapped);

        /*match *self {
            Edge::Left(x) | Edge::Right(x) => {
                before.baseline.set_to_x(x);
                after.baseline.set_from_x(x);
            }
            Edge::Top(y) | Edge::Bottom(y) => {
                before.baseline.set_to_y(y);
                after.baseline.set_from_y(y);
            }
        }*/

        (prev, next)
    }
}

pub(crate) struct ContourClipper {
    clip_polygon: SmallVec<[Point2DF32; 4]>,
    contour: Contour,
}

impl ContourClipper {
    #[inline]
    pub(crate) fn new(clip_polygon: &[Point2DF32], contour: Contour) -> ContourClipper {
        debug_assert!(!clip_polygon.is_empty());
        ContourClipper { clip_polygon: SmallVec::from_slice(clip_polygon), contour }
    }

    #[inline]
    pub(crate) fn from_rect(clip_rect: &Rect<f32>, contour: Contour) -> ContourClipper {
        ContourClipper::new(&[
            Point2DF32::from_euclid(clip_rect.origin),
            Point2DF32::from_euclid(clip_rect.top_right()),
            Point2DF32::from_euclid(clip_rect.bottom_right()),
            Point2DF32::from_euclid(clip_rect.bottom_left()),
        ], contour)
    }

    pub(crate) fn clip(mut self) -> Contour {
        // TODO(pcwalton): Reenable this optimization.
        /*if self.clip_rect.contains_rect(&self.contour.bounds()) {
            return self.contour
        }*/

        let clip_polygon = mem::replace(&mut self.clip_polygon, SmallVec::default());
        let mut prev = match clip_polygon.last() {
            None => return Contour::new(),
            Some(prev) => *prev,
        };
        for &next in &clip_polygon {
            self.clip_against(Edge(LineSegmentF32::new(&prev, &next)));
            prev = next;
        }

        /*
        let top = Point2DF32::new(lerp(self.clip_rect.origin.x, self.clip_rect.max_x(), 0.5),
                                  self.clip_rect.origin.y);
        self.clip_against(Edge(LineSegmentF32::new(&Point2DF32::from_euclid(self.clip_rect
                                                                                .bottom_left()),
                                                   &top)));
        self.clip_against(Edge(LineSegmentF32::new(&top,
                                                   &Point2DF32::from_euclid(self.clip_rect
                                                                                .bottom_right()))));
        self.clip_against(Edge::bottom(&self.clip_rect));
        */

        self.contour
    }

    fn clip_against(&mut self, edge: Edge) {
        let input = mem::replace(&mut self.contour, Contour::new());
        for mut segment in input.iter() {
            // Easy cases.
            match edge.trivially_test_segment(&segment) {
                EdgeRelativeLocation::Outside => continue,
                EdgeRelativeLocation::Inside => {
                    //println!("trivial test inside, pushing segment");
                    push_segment(&mut self.contour, &segment, edge);
                    continue;
                }
                EdgeRelativeLocation::Intersecting => {}
            }

            // We have a potential intersection.
            //println!("potential intersection: {:?} edge: {:?}", segment, edge);
            let mut starts_inside = edge.point_is_inside(&segment.baseline.from());
            while let Some((before_split, after_split)) = edge.split_segment(&segment) {
                // Push the split segment if appropriate.
                /*
                println!("... ... before_split={:?} after_split={:?} starts_inside={:?}",
                         before_split,
                         after_split,
                         starts_inside);
                  */
                if starts_inside {
                    //println!("... split segment case, pushing segment");
                    push_segment(&mut self.contour, &before_split, edge);
                }

                // We've now transitioned from inside to outside or vice versa.
                starts_inside = !starts_inside;
                segment = after_split;
            }

            // No more intersections. Push the last segment if applicable.
            if starts_inside {
                //println!("... last segment case, pushing segment");
                push_segment(&mut self.contour, &segment, edge);
            }
        }

        fn push_segment(contour: &mut Contour, segment: &Segment, edge: Edge) {
            //println!("... push_segment({:?}, edge={:?}", segment, edge);
            if let Some(last_position) = contour.last_position() {
                if last_position != segment.baseline.from() {
                    // Add a line to join up segments.
                    //check_point(&segment.baseline.from(), edge);
                    contour.push_point(segment.baseline.from(), PointFlags::empty());
                }
            }

            //check_point(&segment.baseline.to(), edge);
            contour.push_segment(*segment);
        }

        /*
        fn check_point(point: &Point2DF32, edge: Edge) {
            match edge {
                Edge::Left(x) if point.x() + 0.1 >= x => return,
                Edge::Top(y) if point.y() + 0.1 >= y => return,
                Edge::Right(x) if point.x() - 0.1 <= x => return,
                Edge::Bottom(y) if point.y() - 0.1 <= y => return,
                _ => {}
            }
            panic!("point {:?} outside edge {:?}", point, edge);
        }
        */
    }
}

enum EdgeRelativeLocation {
    Intersecting,
    Inside,
    Outside,
}

// 3D quad clipping

pub struct PolygonClipper3D {
    subject: Vec<Point3DF32>,
}

impl PolygonClipper3D {
    #[inline]
    pub fn new(subject: Vec<Point3DF32>) -> PolygonClipper3D {
        PolygonClipper3D { subject }
    }

    pub fn clip(mut self) -> Vec<Point3DF32> {
        // TODO(pcwalton): Fast path for completely contained polygon?

        self.clip_against(Edge3D::Left);
        self.clip_against(Edge3D::Right);
        self.clip_against(Edge3D::Bottom);
        self.clip_against(Edge3D::Top);
        self.clip_against(Edge3D::Near);
        self.clip_against(Edge3D::Far);

        self.subject
    }

    fn clip_against(&mut self, edge: Edge3D) {
        let input = mem::replace(&mut self.subject, vec![]);
        let mut prev = match input.last() {
            None => return,
            Some(point) => *point,
        };
        for next in input {
            if edge.point_is_inside(next) {
                if !edge.point_is_inside(prev) {
                    self.subject.push(edge.line_intersection(prev, next));
                }
                self.subject.push(next);
            } else if edge.point_is_inside(prev) {
                self.subject.push(edge.line_intersection(prev, next));
            }
            prev = next;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Edge3D {
    Left,
    Right,
    Bottom,
    Top,
    Near,
    Far
}

impl Edge3D {
    #[inline]
    fn point_is_inside(self, point: Point3DF32) -> bool {
        match self {
            Edge3D::Left   => point.x() >= -1.0, Edge3D::Right => point.x() <= 1.0,
            Edge3D::Bottom => point.y() >= -1.0, Edge3D::Top   => point.y() <= 1.0,
            Edge3D::Near   => point.z() >= -1.0, Edge3D::Far   => point.z() <= 1.0,
        }
    }

    fn line_intersection(self, prev: Point3DF32, next: Point3DF32) -> Point3DF32 {
        let (x0, x1) = match self {
            Edge3D::Left   | Edge3D::Right => (prev.x(), next.x()),
            Edge3D::Bottom | Edge3D::Top   => (prev.y(), next.y()),
            Edge3D::Near   | Edge3D::Far   => (prev.z(), next.z()),
        };
        let x = match self {
            Edge3D::Left  | Edge3D::Bottom | Edge3D::Near => -1.0,
            Edge3D::Right | Edge3D::Top    | Edge3D::Far  =>  1.0,
        };
        prev.lerp(next, (x - x0) / (x1 - x0))
    }
}
