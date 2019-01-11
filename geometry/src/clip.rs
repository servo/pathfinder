// pathfinder/geometry/src/clip.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

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
        let (mut from, mut path_start, mut first_point) = (Point2D::zero(), None, false);
        let input = mem::replace(output, vec![]);
        for event in input {
            let to = match event {
                PathEvent::MoveTo(to) => {
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
                PathEvent::CubicTo(_, _, to) => to,
                PathEvent::Arc(..) => panic!("Arcs unsupported!"),
            };

            if edge.point_is_inside(&to) {
                if !edge.point_is_inside(&from) {
                    add_line(&edge.line_intersection(&from, &to), output, &mut first_point);
                }
                add_line(&to, output, &mut first_point);
            } else if edge.point_is_inside(&from) {
                add_line(&edge.line_intersection(&from, &to), output, &mut first_point);
            }

            from = to;

            if let PathEvent::Close = event {
                output.push(PathEvent::Close);
                path_start = None;
            }
        }

        fn add_line(to: &Point2D<f32>, output: &mut Vec<PathEvent>, first_point: &mut bool) {
            if *first_point {
                output.push(PathEvent::MoveTo(*to));
                *first_point = false;
            } else {
                output.push(PathEvent::LineTo(*to));
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
    fn point_is_inside(&self, point: &Point2D<f32>) -> bool {
        match *self {
            Edge::Left(x_edge) => point.x >= x_edge,
            Edge::Top(y_edge) => point.y >= y_edge,
            Edge::Right(x_edge) => point.x <= x_edge,
            Edge::Bottom(y_edge) => point.y <= y_edge,
        }
    }

    fn line_intersection(&self, start_point: &Point2D<f32>, endpoint: &Point2D<f32>)
                         -> Point2D<f32> {
        let start_point = Vector3D::new(start_point.x, start_point.y, 1.0);
        let endpoint = Vector3D::new(endpoint.x, endpoint.y, 1.0);
        let edge = match *self {
            Edge::Left(x_edge) | Edge::Right(x_edge) => Vector3D::new(1.0, 0.0, -x_edge),
            Edge::Top(y_edge) | Edge::Bottom(y_edge) => Vector3D::new(0.0, 1.0, -y_edge),
        };
        let intersection = start_point.cross(endpoint).cross(edge);
        Point2D::new(intersection.x / intersection.z, intersection.y / intersection.z)
    }
}
