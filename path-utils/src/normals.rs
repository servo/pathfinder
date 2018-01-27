// pathfinder/path-utils/src/normals.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use euclid::Vector2D;
use euclid::approxeq::ApproxEq;
use lyon_path::PathEvent;

#[derive(Clone)]
pub struct PathNormals {
    normals: Vec<Vector2D<f32>>,
}

impl PathNormals {
    #[inline]
    pub fn new() -> PathNormals {
        PathNormals {
            normals: vec![],
        }
    }

    #[inline]
    pub fn normals(&self) -> &[Vector2D<f32>] {
        &self.normals
    }

    pub fn clear(&mut self) {
        self.normals.clear()
    }

    pub fn add_path<I>(&mut self, path: I) where I: Iterator<Item = PathEvent> {
        let mut path = path.peekable();
        while path.peek().is_some() {
            let mut positions = vec![];
            loop {
                match path.next() {
                    Some(PathEvent::MoveTo(to)) | Some(PathEvent::LineTo(to)) => {
                        positions.push(to)
                    }
                    Some(PathEvent::QuadraticTo(ctrl, to)) => {
                        positions.push(ctrl);
                        positions.push(to);
                    }
                    Some(PathEvent::CubicTo(ctrl1, ctrl2, to)) => { 
                        positions.push(ctrl1);
                        positions.push(ctrl2);
                        positions.push(to);
                    }
                    Some(PathEvent::Arc(..)) => panic!("PathNormals: Arcs currently unsupported!"),
                    None | Some(PathEvent::Close) => break,
                }

                if let Some(&PathEvent::MoveTo(..)) = path.peek() {
                    break
                }
            }

            let mut last_nonzero_normal = Vector2D::zero();
            self.normals.reserve(positions.len());

            for (this_index, this_position) in positions.iter().enumerate() {
                let prev_index = if this_index > 0 {
                    this_index - 1
                } else {
                    positions.len() - 1
                };
                let next_index = if this_index + 1 < positions.len() {
                    this_index + 1
                } else {
                    0
                };
                let prev_vector = *this_position - positions[prev_index];
                let next_vector = positions[next_index] - *this_position;
                let bisector = prev_vector * next_vector.length() +
                    next_vector * prev_vector.length();

                let mut normal = bisector.normalize();
                if normal.square_length().approx_eq(&0.0) {
                    normal = last_nonzero_normal
                } else {
                    last_nonzero_normal = normal
                }

                self.normals.push(normal)
            }
        }
    }
}
