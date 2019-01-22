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
use crate::point::Point2DF32;
use crate::segment::Segment;
use crate::util::lerp;
use arrayvec::ArrayVec;
use euclid::{Point2D, Rect, Vector3D};
use lyon_path::PathEvent;
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
        self.clip_against(Edge::Left(self.clip_rect.origin.x), &mut output);
        self.clip_against(Edge::Top(self.clip_rect.origin.y), &mut output);
        self.clip_against(Edge::Right(self.clip_rect.max_x()), &mut output);
        self.clip_against(Edge::Bottom(self.clip_rect.max_y()), &mut output);
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
                    let intersection = edge.line_intersection(&LineSegmentF32::new(&from, &to));
                    add_line(&intersection, output, &mut first_point);
                }
                add_line(&to, output, &mut first_point);
            } else if edge.point_is_inside(&from) {
                let intersection = edge.line_intersection(&LineSegmentF32::new(&from, &to));
                add_line(&intersection, output, &mut first_point);
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
enum Edge {
    Left(f32),
    Top(f32),
    Right(f32),
    Bottom(f32),
}

impl Edge {
    fn point_is_inside(&self, point: &Point2DF32) -> bool {
        match *self {
            Edge::Left(x_edge) => point.x() > x_edge,
            Edge::Top(y_edge) => point.y() > y_edge,
            Edge::Right(x_edge) => point.x() < x_edge,
            Edge::Bottom(y_edge) => point.y() < y_edge,
        }
    }

    fn trivially_test_segment(&self, segment: &Segment) -> EdgeRelativeLocation {
        let from_inside = self.point_is_inside(&segment.baseline.from());
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

    fn line_intersection(&self, line_segment: &LineSegmentF32) -> Point2DF32 {
        match *self {
            Edge::Left(x_edge) | Edge::Right(x_edge) => {
                Point2DF32::new(x_edge, line_segment.solve_y_for_x(x_edge))
            }
            Edge::Top(y_edge) | Edge::Bottom(y_edge) => {
                Point2DF32::new(line_segment.solve_x_for_y(y_edge), y_edge)
            }
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
        let intersection;
        match *self {
            Edge::Left(x_edge) | Edge::Right(x_edge) => {
                if (segment.baseline.from_x() <= x_edge && segment.baseline.to_x() <= x_edge) ||
                        (segment.baseline.from_x() >= x_edge &&
                         segment.baseline.to_x() >= x_edge) {
                    return None
                }
                intersection = Point2DF32::new(x_edge, segment.baseline.solve_y_for_x(x_edge));
            }
            Edge::Top(y_edge) | Edge::Bottom(y_edge) => {
                if (segment.baseline.from_y() <= y_edge && segment.baseline.to_y() <= y_edge) ||
                        (segment.baseline.from_y() >= y_edge &&
                         segment.baseline.to_y() >= y_edge) {
                    return None
                }
                intersection = Point2DF32::new(segment.baseline.solve_x_for_y(y_edge), y_edge);
            }
        };
        Some((Segment::line(&LineSegmentF32::new(&segment.baseline.from(), &intersection)),
              Segment::line(&LineSegmentF32::new(&intersection, &segment.baseline.to()))))
    }

    fn intersect_cubic_segment(&self, segment: &Segment, t_min: f32, t_max: f32) -> Option<f32> {
        /*
        println!("... intersect_cubic_segment({:?}, {:?}, t=({}, {}))",
                 self, segment, t_min, t_max);
        */
        let t_mid = lerp(t_min, t_max, 0.5);
        if t_max - t_min < 0.001 {
            return Some(t_mid);
        }

        let (prev_segment, next_segment) = segment.as_cubic_segment().split(t_mid);

        let prev_cubic_segment = prev_segment.as_cubic_segment();
        let next_cubic_segment = next_segment.as_cubic_segment();

        let (prev_min, prev_max, next_min, next_max, edge);
        match *self {
            Edge::Left(x) | Edge::Right(x) => {
                prev_min = prev_cubic_segment.min_x();
                prev_max = prev_cubic_segment.max_x();
                next_min = next_cubic_segment.min_x();
                next_max = next_cubic_segment.max_x();
                edge = x;
            }
            Edge::Top(y) | Edge::Bottom(y) => {
                prev_min = prev_cubic_segment.min_y();
                prev_max = prev_cubic_segment.max_y();
                next_min = next_cubic_segment.min_y();
                next_max = next_cubic_segment.max_y();
                edge = y;
            }
        }

        if prev_min < edge && edge < prev_max {
            self.intersect_cubic_segment(segment, t_min, t_mid)
        } else if next_min < edge && edge < next_max {
            self.intersect_cubic_segment(segment, t_mid, t_max)
        } else if (prev_max == edge && next_min == edge) ||
                (prev_min == edge && next_max == edge) {
            Some(t_mid)
        } else {
            None
        }
    }

    fn fixup_clipped_segments(&self, segment: &(Segment, Segment)) -> (Segment, Segment) {
        let (mut before, mut after) = *segment;
        match *self {
            Edge::Left(x) | Edge::Right(x) => {
                before.baseline.set_to_x(x);
                after.baseline.set_from_x(x);
            }
            Edge::Top(y) | Edge::Bottom(y) => {
                before.baseline.set_to_y(y);
                after.baseline.set_from_y(y);
            }
        }
        (before, after)
    }
}

pub(crate) struct ContourRectClipper {
    clip_rect: Rect<f32>,
    contour: Contour,
}

impl ContourRectClipper {
    #[inline]
    pub(crate) fn new(clip_rect: &Rect<f32>, contour: Contour) -> ContourRectClipper {
        ContourRectClipper { clip_rect: *clip_rect, contour }
    }

    pub(crate) fn clip(mut self) -> Contour {
        if self.clip_rect.contains_rect(&self.contour.bounds()) {
            return self.contour
        }

        self.clip_against(Edge::Left(self.clip_rect.origin.x));
        self.clip_against(Edge::Top(self.clip_rect.origin.y));
        self.clip_against(Edge::Right(self.clip_rect.max_x()));
        self.clip_against(Edge::Bottom(self.clip_rect.max_y()));
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
