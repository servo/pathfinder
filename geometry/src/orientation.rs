// pathfinder/geometry/src/orientation.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::outline::Outline;
use euclid::Point2D;
use lyon_path::PathEvent;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Orientation {
    Ccw = -1,
    Cw = 1,
}

impl Orientation {
    /// This follows the FreeType algorithm.
    pub fn from_path<I>(stream: I) -> Orientation where I: Iterator<Item = PathEvent> {
        let (mut from, mut subpath_start) = (Point2D::zero(), Point2D::zero());
        let mut area = 0.0;
        for event in stream {
            match event {
                PathEvent::MoveTo(to) => {
                    from = to;
                    subpath_start = to;
                }
                PathEvent::LineTo(to) => {
                    area += det(&from, &to);
                    from = to;
                }
                PathEvent::QuadraticTo(ctrl, to) => {
                    area += det(&from, &ctrl) + det(&ctrl, &to);
                    from = to;
                }
                PathEvent::CubicTo(ctrl0, ctrl1, to) => {
                    area += det(&from, &ctrl0) + det(&ctrl0, &ctrl1) + det(&ctrl1, &to);
                    from = to;
                }
                PathEvent::Arc(..) => {
                    // TODO(pcwalton)
                }
                PathEvent::Close => {
                    area += det(&from, &subpath_start);
                    from = subpath_start;
                }
            }
        }
        Orientation::from_area(area)
    }

    // Pathfinder 3 version
    pub fn from_outline(outline: &Outline) -> Orientation {
        let mut area = 0.0;
        for contour in &outline.contours {
            let mut prev_position = match contour.last_position() {
                None => continue,
                Some(position) => position,
            };
            for &next_position in &contour.points {
                area += prev_position.det(next_position);
                prev_position = next_position;
            }
        }
        Orientation::from_area(area)
    }

    fn from_area(area: f32) -> Orientation {
        if area <= 0.0 {
            Orientation::Ccw
        } else {
            Orientation::Cw
        }
    }
}

fn det(a: &Point2D<f32>, b: &Point2D<f32>) -> f32 {
    a.x * b.y - a.y * b.x
}

