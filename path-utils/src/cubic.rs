// pathfinder/path-utils/src/cubic.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Utilities for cubic Bézier curves.

use euclid::Point2D;

use curve::Curve;

const MAX_APPROXIMATION_ITERATIONS: u8 = 32;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CubicCurve {
    pub endpoints: [Point2D<f32>; 2],
    pub control_points: [Point2D<f32>; 2],
}

impl CubicCurve {
    #[inline]
    pub fn new(endpoint_0: &Point2D<f32>,
               control_point_0: &Point2D<f32>,
               control_point_1: &Point2D<f32>,
               endpoint_1: &Point2D<f32>)
               -> CubicCurve {
        CubicCurve {
            endpoints: [*endpoint_0, *endpoint_1],
            control_points: [*control_point_0, *control_point_1],
        }
    }

    pub fn sample(&self, t: f32) -> Point2D<f32> {
        let (p0, p3) = (&self.endpoints[0], &self.endpoints[1]);
        let (p1, p2) = (&self.control_points[0], &self.control_points[1]);
        let (p0p1, p1p2, p2p3) = (p0.lerp(*p1, t), p1.lerp(*p2, t), p2.lerp(*p3, t));
        let (p0p1p2, p1p2p3) = (p0p1.lerp(p1p2, t), p1p2.lerp(p2p3, t));
        p0p1p2.lerp(p1p2p3, t)
    }

    pub fn subdivide(&self, t: f32) -> (CubicCurve, CubicCurve) {
        let (p0, p3) = (&self.endpoints[0], &self.endpoints[1]);
        let (p1, p2) = (&self.control_points[0], &self.control_points[1]);
        let (p0p1, p1p2, p2p3) = (p0.lerp(*p1, t), p1.lerp(*p2, t), p2.lerp(*p3, t));
        let (p0p1p2, p1p2p3) = (p0p1.lerp(p1p2, t), p1p2.lerp(p2p3, t));
        let p0p1p2p3 = p0p1p2.lerp(p1p2p3, t);
        (CubicCurve::new(&p0, &p0p1, &p0p1p2, &p0p1p2p3),
         CubicCurve::new(&p0p1p2p3, &p1p2p3, &p2p3, &p3))
    }

    pub fn approximate_curve(&self, error_bound: f32) -> ApproximateCurveIter {
        ApproximateCurveIter::new(self, error_bound)
    }
}

pub struct ApproximateCurveIter {
    curves: Vec<CubicCurve>,
    error_bound: f32,
    iteration: u8,
}

impl ApproximateCurveIter {
    fn new(cubic: &CubicCurve, error_bound: f32) -> ApproximateCurveIter {
        let (curve_a, curve_b) = cubic.subdivide(0.5);
        ApproximateCurveIter {
            curves: vec![curve_b, curve_a],
            error_bound: error_bound,
            iteration: 0,
        }
    }
}

impl Iterator for ApproximateCurveIter {
    type Item = Curve;

    fn next(&mut self) -> Option<Curve> {
        let mut cubic = match self.curves.pop() {
            Some(cubic) => cubic,
            None => return None,
        };

        while self.iteration < MAX_APPROXIMATION_ITERATIONS {
            self.iteration += 1;

            // See Sederberg § 2.6, "Distance Between Two Bézier Curves".
            let delta_control_point_0 = (cubic.endpoints[0] - cubic.control_points[0] * 3.0) +
                (cubic.control_points[1] * 3.0 - cubic.endpoints[1]);
            let delta_control_point_1 = (cubic.control_points[0] * 3.0 - cubic.endpoints[0]) +
                (cubic.endpoints[1] - cubic.control_points[1] * 3.0);
            let max_error = f32::max(delta_control_point_1.length(),
                                     delta_control_point_0.length()) / 6.0;
            if max_error < self.error_bound {
                break
            }

            let (cubic_a, cubic_b) = cubic.subdivide(0.5);
            self.curves.push(cubic_b);
            cubic = cubic_a
        }

        let approx_control_point_0 = (cubic.control_points[0] * 3.0 - cubic.endpoints[0]) * 0.5;
        let approx_control_point_1 = (cubic.control_points[1] * 3.0 - cubic.endpoints[1]) * 0.5;

        Some(Curve::new(&cubic.endpoints[0],
                        &approx_control_point_0.lerp(approx_control_point_1, 0.5).to_point(),
                        &cubic.endpoints[1]))
    }
}
