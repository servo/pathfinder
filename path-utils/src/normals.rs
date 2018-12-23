// pathfinder/path-utils/src/normals.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use euclid::approxeq::ApproxEq;
use euclid::{Point2D, Vector2D};
use lyon_path::PathEvent;
use orientation::Orientation;

#[derive(Clone, Copy, Debug)]
pub struct SegmentNormals {
    pub from: Vector2D<f32>,
    pub ctrl: Vector2D<f32>,
    pub to: Vector2D<f32>,
}

#[derive(Clone)]
pub struct PathNormals {
    normals: Vec<SegmentNormals>,
}

impl PathNormals {
    #[inline]
    pub fn new() -> PathNormals {
        PathNormals { normals: vec![] }
    }

    #[inline]
    pub fn normals(&self) -> &[SegmentNormals] {
        &self.normals
    }

    pub fn clear(&mut self) {
        self.normals.clear()
    }

    pub fn add_path<I>(&mut self, stream: I)
    where
        I: Iterator<Item = PathEvent>,
    {
        let events: Vec<_> = stream.collect();
        let orientation = Orientation::from_path(events.iter().cloned());

        let (mut path_ops, mut path_points) = (vec![], vec![]);
        let mut stream = events.iter().cloned();
        while let Some(event) = stream.next() {
            path_ops.push(PathOp::from_path_event(&event));
            match event {
                PathEvent::MoveTo(to) => path_points.push(to),
                PathEvent::LineTo(to) => path_points.push(to),
                PathEvent::QuadraticTo(ctrl, to) => path_points.extend_from_slice(&[ctrl, to]),
                PathEvent::CubicTo(..) => {
                    panic!("PathNormals::add_path(): Convert cubics to quadratics first!")
                }
                PathEvent::Arc(..) => {
                    panic!("PathNormals::add_path(): Convert arcs to quadratics first!")
                }
                PathEvent::Close => self.flush(orientation, path_ops.drain(..), &mut path_points),
            }
        }

        self.flush(orientation, path_ops.into_iter(), &mut path_points);
    }

    fn flush<I>(
        &mut self,
        orientation: Orientation,
        path_stream: I,
        path_points: &mut Vec<Point2D<f32>>,
    ) where
        I: Iterator<Item = PathOp>,
    {
        match path_points.len() {
            0 | 1 => path_points.clear(),
            2 => {
                let orientation = -(orientation as i32 as f32);
                self.normals.push(SegmentNormals {
                    from: (path_points[1] - path_points[0]) * orientation,
                    ctrl: Vector2D::zero(),
                    to: (path_points[0] - path_points[1]) * orientation,
                });
                path_points.clear();
            }
            _ => self.flush_slow(orientation, path_stream, path_points),
        }
    }

    fn flush_slow<I>(
        &mut self,
        orientation: Orientation,
        path_stream: I,
        path_points: &mut Vec<Point2D<f32>>,
    ) where
        I: Iterator<Item = PathOp>,
    {
        let mut normals = vec![Vector2D::zero(); path_points.len()];
        for (index, point) in path_points.iter().enumerate() {
            let (mut prev_index, mut next_index) = (index, index);
            while path_points[prev_index].approx_eq(&path_points[index]) {
                prev_index = if prev_index == 0 {
                    path_points.len() - 1
                } else {
                    prev_index - 1
                }
            }
            while path_points[next_index].approx_eq(&path_points[index]) {
                next_index = if next_index == path_points.len() - 1 {
                    0
                } else {
                    next_index + 1
                }
            }
            normals[index] = compute_normal(
                orientation,
                &path_points[prev_index],
                point,
                &path_points[next_index],
            )
        }
        path_points.clear();

        let mut next_normal_index = 0;
        for op in path_stream {
            match op {
                PathOp::MoveTo => next_normal_index += 1,
                PathOp::LineTo => {
                    next_normal_index += 1;
                    self.normals.push(SegmentNormals {
                        from: normals[next_normal_index - 2],
                        ctrl: normals[next_normal_index - 2]
                            .lerp(normals[next_normal_index - 1], 0.5),
                        to: normals[next_normal_index - 1],
                    });
                }
                PathOp::QuadraticTo => {
                    next_normal_index += 2;
                    self.normals.push(SegmentNormals {
                        from: normals[next_normal_index - 3],
                        ctrl: normals[next_normal_index - 2],
                        to: normals[next_normal_index - 1],
                    })
                }
                PathOp::Close => {
                    self.normals.push(SegmentNormals {
                        from: normals[next_normal_index - 1],
                        ctrl: normals[next_normal_index - 1].lerp(normals[0], 0.5),
                        to: normals[0],
                    });
                    break;
                }
            }
        }
    }
}

fn compute_normal(
    orientation: Orientation,
    prev: &Point2D<f32>,
    current: &Point2D<f32>,
    next: &Point2D<f32>,
) -> Vector2D<f32> {
    let vector = ((*current - *prev) + (*next - *current)).normalize();
    Vector2D::new(vector.y, -vector.x) * -(orientation as i32 as f32)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum PathOp {
    MoveTo,
    LineTo,
    QuadraticTo,
    Close,
}

impl PathOp {
    fn from_path_event(event: &PathEvent) -> PathOp {
        match *event {
            PathEvent::MoveTo(..) => PathOp::MoveTo,
            PathEvent::LineTo(..) => PathOp::LineTo,
            PathEvent::QuadraticTo(..) => PathOp::QuadraticTo,
            PathEvent::Close => PathOp::Close,
            PathEvent::Arc(..) | PathEvent::CubicTo(..) => unreachable!(),
        }
    }
}
