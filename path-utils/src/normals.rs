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
                        positions.push(to);
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

            self.normals.reserve(positions.len());

            for (this_index, this_position) in positions.iter().enumerate() {
                let mut prev_index = this_index;
                let mut prev_vector;
                loop {
                    if prev_index > 0 {
                        prev_index -= 1
                    } else {
                        prev_index = positions.len() - 1
                    }
                    prev_vector = *this_position - positions[prev_index];
                    if prev_index == this_index {
                        println!("uh-oh, NaN prev!");
                    }
                    if !prev_vector.square_length().approx_eq(&0.0) {
                        break
                    }
                }

                let mut next_index = this_index;
                let mut next_vector;
                loop {
                    if next_index + 1 < positions.len() {
                        next_index += 1
                    } else {
                        next_index = 0
                    }
                    next_vector = positions[next_index] - *this_position;
                    if next_index == this_index {
                        println!("uh-oh, NaN next!");
                    }
                    if !next_vector.square_length().approx_eq(&0.0) {
                        break
                    }
                }

                println!("prev vector {:?} ({:?}) next vector {:?} ({:?})",
                         prev_vector, prev_vector.length(),
                         next_vector, next_vector.length());

                let prev_normal = rotate(&prev_vector).normalize();
                let next_normal = rotate(&next_vector).normalize();
                let mut bisector = (prev_normal + next_normal) * 0.5;
                if bisector.square_length().approx_eq(&0.0) {
                    bisector = Vector2D::new(next_vector.y, next_vector.x)
                }

                self.normals.push(bisector.normalize());
            }
        }
    }
}

fn rotate(vector: &Vector2D<f32>) -> Vector2D<f32> {
    Vector2D::new(-vector.y, vector.x)
}
