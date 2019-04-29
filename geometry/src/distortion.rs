// pathfinder/geometry/src/distortion.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::basic::point::{Point2DF32, Point2DI32};
use crate::outline::{self, Contour};

#[derive(Clone, Copy, Debug)]
pub struct BarrelDistortionCoefficients {
    pub k0: f32,
    pub k1: f32,
}

impl Default for BarrelDistortionCoefficients {
    // Matches Google Daydream (Cardboard v2.2).
    #[inline]
    fn default() -> BarrelDistortionCoefficients {
        BarrelDistortionCoefficients { k0: 0.34, k1: 0.55 }
    }
}

pub struct ContourBarrelDistorter<'a> {
    contour: &'a mut Contour,
    window_size: Point2DI32,
    coefficients: BarrelDistortionCoefficients,
}

impl<'a> ContourBarrelDistorter<'a> {
    pub fn new(contour: &'a mut Contour,
               coefficients: BarrelDistortionCoefficients,
               window_size: Point2DI32)
               -> ContourBarrelDistorter<'a> {
        ContourBarrelDistorter { contour, window_size, coefficients }
    }

    pub fn distort(&mut self) {
        let one = Point2DF32::splat(1.0);
        let window_size = self.window_size.to_f32();
        let inv_window_size = Point2DF32(window_size.0.approx_recip());
        let BarrelDistortionCoefficients { k0, k1 } = self.coefficients;

        let point_count = self.contour.len();
        for point_index in 0..point_count {
            // Convert from window coordinates to NDC.
            let mut position = self.contour.position_of(point_index);
            position = position.scale_xy(inv_window_size).scale(2.0) - one;

            // Apply distortion.
            let r2 = position.square_length();
            let scaling = 1.0 + k0 * r2 + k1 * r2 * r2;
            position = position.scale(1.0 / scaling);

            // Convert back to window coordinates.
            position = (position + one).scale(0.5).scale_xy(window_size);

            // Store resulting point.
            self.contour.points[point_index as usize] = position;
            outline::union_rect(&mut self.contour.bounds, position, point_index == 0);
        }
    }
}
