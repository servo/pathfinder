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

#[derive(Clone, Copy)]
enum Edge {
    Left(f32),
    Top(f32),
    Right(f32),
    Bottom(f32),
}

impl Edge {
    fn point_is_inside(&self, point: &Point2DF32) -> bool {
        match *self {
            Edge::Left(x_edge) => point.x() >= x_edge,
            Edge::Top(y_edge) => point.y() >= y_edge,
            Edge::Right(x_edge) => point.x() <= x_edge,
            Edge::Bottom(y_edge) => point.y() <= y_edge,
        }
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
        let mut first_point = false;
        let input = mem::replace(&mut self.contour, Contour::new());
        for event in input.iter() {
            let (from, to) = (event.baseline.from(), event.baseline.to());
            if edge.point_is_inside(&to) {
                if !edge.point_is_inside(&from) {
                    //println!("clip: {:?} {:?}", from, to);
                    let intersection = edge.line_intersection(&LineSegmentF32::new(&from, &to));
                    add_line(&intersection, &mut self.contour, &mut first_point);
                }
                add_line(&to, &mut self.contour, &mut first_point);
            } else if edge.point_is_inside(&from) {
                //println!("clip: {:?} {:?}", from, to);
                let intersection = edge.line_intersection(&LineSegmentF32::new(&from, &to));
                add_line(&intersection, &mut self.contour, &mut first_point);
            }
        }

        fn add_line(to: &Point2DF32, output: &mut Contour, first_point: &mut bool) {
            output.push_point(*to, PointFlags::empty());
            *first_point = false;
        }
    }
}
