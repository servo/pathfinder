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
use crate::orientation::Orientation;
use crate::outline::Contour;

pub struct ContourDilator<'a> {
    contour: &'a mut Contour,
    amount: Point2DF32,
    orientation: Orientation,
}

impl<'a> ContourDilator<'a> {
    pub fn new(contour: &'a mut Contour, amount: Point2DF32, orientation: Orientation)
               -> ContourDilator<'a> {
        ContourDilator { contour, amount, orientation }
    }

    pub fn dilate(&mut self) {
        //println!("---");
        let scale = self.amount.scale_xy(match self.orientation {
            Orientation::Ccw => Point2DF32::new( 1.0, -1.0),
            Orientation::Cw  => Point2DF32::new(-1.0,  1.0),
        });

        //let input = self.contour.clone();

        let first_position = self.contour.position_of(0);
        let mut prev_point_index = 0;
        let mut prev_position;

        loop {
            prev_point_index = self.contour.prev_point_index_of(prev_point_index);
            prev_position = self.contour.position_of(prev_point_index);
            if prev_point_index == 0 || prev_position != first_position {
                break;
            }
        }

        // Find the starting position.
        let first_point_index = self.contour.next_point_index_of(prev_point_index);
        let mut current_point_index = first_point_index;
        let mut position = first_position;

        let mut prev_vector = (position - prev_position).normalize();

        loop {
            // Find the next non-degenerate position.
            let mut next_point_index = current_point_index;
            let mut next_position;
            loop {
                next_point_index = self.contour.next_point_index_of(next_point_index);
                if next_point_index == first_point_index {
                    next_position = first_position;
                    break;
                }
                next_position = self.contour.position_of(next_point_index);
                if next_point_index == current_point_index || next_position != position {
                    break;
                }
            }
            let next_vector = (next_position - position).normalize();

            /*
            println!("prev={} cur={} next={}",
                     prev_point_index,
                     current_point_index,
                     next_point_index);
            */

            // Calculate new position by moving the point by the bisector.
            let bisector = prev_vector.yx() + next_vector.yx();
            let bisector_length = bisector.length();
            let scaled_bisector = if bisector_length == 0.0 {
                Point2DF32::default()
            } else {
                bisector.scale_xy(scale).scale(1.0 / bisector_length)
            };
            let new_position = position - scaled_bisector;

            /*
            println!("dilate(): prev={}({:?}) cur={}({:?}) next={}({:?}) bisector={:?}({:?}, {:?})",
                     prev_point_index,
                     prev_position,
                     current_point_index,
                     position,
                     next_point_index,
                     next_position,
                     bisector,
                     bisector_length,
                     scaled_bisector);
            */

            /*if bisector_length == 0.0 {
                println!("dilate({:?}): {:?} -> {:?} (bisector {:?}, length {:?})",
                        self.amount,
                        position,
                        new_position,
                        bisector,
                        bisector_length);
            }*/

            // Update all points.
            let mut point_index = current_point_index;
            while point_index != next_point_index {
                self.contour.points[point_index as usize] = new_position;
                //println!("... updating {:?}", point_index);
                point_index = self.contour.next_point_index_of(point_index);
            }

            // Check to see if we're done.
            if next_point_index == first_point_index {
                break;
            }

            // Continue.
            prev_point_index = next_point_index - 1;
            prev_position = position;
            prev_vector = next_vector;
            position = next_position;
            current_point_index = next_point_index;
        }
    }
}
