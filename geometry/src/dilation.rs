// pathfinder/geometry/src/dilation.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::basic::point::Point2DF32;
use crate::outline::Contour;

pub struct ContourDilator<'a> {
    contour: &'a mut Contour,
    amount: Point2DF32,
}

impl<'a> ContourDilator<'a> {
    pub fn new(contour: &'a mut Contour, amount: Point2DF32) -> ContourDilator<'a> {
        ContourDilator { contour, amount }
    }

    pub fn dilate(&mut self) {
        let mut position = self.contour.position_of(0);
        let mut prev_point_index = 0;
        let mut prev_position;
        loop {
            prev_point_index = self.contour.prev_point_index_of(prev_point_index);
            prev_position = self.contour.position_of(prev_point_index);
            if prev_point_index == 0 || prev_position != position {
                break;
            }
        }

        let mut point_index = 0;
        loop {
            // Find the next non-degenerate position.
            let mut next_point_index = point_index;
            let mut next_position;
            loop {
                next_point_index = self.contour.next_point_index_of(next_point_index);
                next_position = self.contour.position_of(next_point_index);
                if next_point_index == point_index || next_position != position {
                    break;
                }
            }

            // Calculate new position by moving the point by the bisector.
            let (prev_vector, next_vector) = (position - prev_position, next_position - position);
            let bisector = prev_vector.scale(next_vector.length()) +
                           next_vector.scale(prev_vector.length());
            let bisector_length = bisector.length();
            let new_position = if bisector_length == 0.0 {
                position
            } else {
                position - bisector.scale_xy(self.amount).scale(1.0 / bisector_length)
            };

            /*println!("dilate({:?}): {:?} -> {:?} (bisector {:?}, length {:?})",
                     self.amount,
                     position,
                     new_position,
                     bisector,
                     bisector_length);*/

            // Update all points.
            for point_index in point_index..next_point_index {
                self.contour.points[point_index as usize] = new_position;
            }

            // We're done if we start to loop around.
            if next_point_index < point_index {
                break;
            }

            // Continue.
            prev_position = position;
            position = next_position;
            point_index = next_point_index;
        }
    }
}
