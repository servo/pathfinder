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
use PathCommand;

const MAX_APPROXIMATION_ITERATIONS: u8 = 32;

/// A cubic Bézier curve.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CubicCurve {
    /// The endpoints of the curve.
    pub endpoints: [Point2D<f32>; 2],
    /// The control points of the curve.
    pub control_points: [Point2D<f32>; 2],
}

impl CubicCurve {
    /// Constructs a new cubic Bézier curve from the given points.
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

    /// Returns the curve point at the given t value (from 0.0 to 1.0).
    pub fn sample(&self, t: f32) -> Point2D<f32> {
        let (p0, p3) = (&self.endpoints[0], &self.endpoints[1]);
        let (p1, p2) = (&self.control_points[0], &self.control_points[1]);
        let (p0p1, p1p2, p2p3) = (p0.lerp(*p1, t), p1.lerp(*p2, t), p2.lerp(*p3, t));
        let (p0p1p2, p1p2p3) = (p0p1.lerp(p1p2, t), p1p2.lerp(p2p3, t));
        p0p1p2.lerp(p1p2p3, t)
    }

    /// De Casteljau subdivides this curve into two at the given t value (from 0.0 to 1.0).
    pub fn subdivide(&self, t: f32) -> (CubicCurve, CubicCurve) {
        let (p0, p3) = (&self.endpoints[0], &self.endpoints[1]);
        let (p1, p2) = (&self.control_points[0], &self.control_points[1]);
        let (p0p1, p1p2, p2p3) = (p0.lerp(*p1, t), p1.lerp(*p2, t), p2.lerp(*p3, t));
        let (p0p1p2, p1p2p3) = (p0p1.lerp(p1p2, t), p1p2.lerp(p2p3, t));
        let p0p1p2p3 = p0p1p2.lerp(p1p2p3, t);
        (CubicCurve::new(&p0, &p0p1, &p0p1p2, &p0p1p2p3),
         CubicCurve::new(&p0p1p2p3, &p1p2p3, &p2p3, &p3))
    }

    /// Approximates this curve with a series of quadratic Bézier curves.
    /// 
    /// The quadratic curves are guaranteed not to deviate from this cubic curve by more than
    /// `error_bound`.
    pub fn approx_curve(&self, error_bound: f32) -> ApproxCurveIter {
        ApproxCurveIter::new(self, error_bound)
    }
}

/// A series of path commands that can contain cubic Bézier segments.
#[derive(Clone, Copy, Debug)]
pub enum CubicPathCommand {
    /// Moves the pen to the given point.
    MoveTo(Point2D<f32>),
    /// Draws a line to the given point.
    LineTo(Point2D<f32>),
    /// Draws a quadratic curve with the control point to the endpoint, respectively.
    QuadCurveTo(Point2D<f32>, Point2D<f32>),
    /// Draws a cubic curve with the two control points to the endpoint, respectively.
    CubicCurveTo(Point2D<f32>, Point2D<f32>, Point2D<f32>),
    /// Closes the current subpath by drawing a line from the current point to the first point of
    /// the subpath.
    ClosePath,
}

/// Converts a series of path commands that can contain cubic Bézier segments to a series of path
/// commands that contain only quadratic Bézier segments.
pub struct CubicPathCommandApproxStream<I> {
    inner: I,
    error_bound: f32,
    last_endpoint: Point2D<f32>,
    approx_curve_iter: Option<ApproxCurveIter>,
}

impl<I> CubicPathCommandApproxStream<I> where I: Iterator<Item = CubicPathCommand> {
    /// Creates a stream that approximates the given path commands by converting all cubic Bézier
    /// curves to quadratic Bézier curves.
    /// 
    /// The resulting path command stream is guaranteed not to deviate more than a distance of
    /// `error_bound` from the original path command stream.
    #[inline]
    pub fn new(inner: I, error_bound: f32) -> CubicPathCommandApproxStream<I> {
        CubicPathCommandApproxStream {
            inner: inner,
            error_bound: error_bound,
            last_endpoint: Point2D::zero(),
            approx_curve_iter: None,
        }
    }
}

impl<I> Iterator for CubicPathCommandApproxStream<I> where I: Iterator<Item = CubicPathCommand> {
    type Item = PathCommand;

    fn next(&mut self) -> Option<PathCommand> {
        loop {
            if let Some(ref mut approx_curve_iter) = self.approx_curve_iter {
                if let Some(curve) = approx_curve_iter.next() {
                    return Some(curve.to_path_command())
                }
            }
            self.approx_curve_iter = None;

            let next_command = match self.inner.next() {
                None => return None,
                Some(next_command) => next_command,
            };

            match next_command {
                CubicPathCommand::ClosePath => {
                    self.last_endpoint = Point2D::zero();
                    return Some(PathCommand::ClosePath)
                }
                CubicPathCommand::MoveTo(endpoint) => {
                    self.last_endpoint = endpoint;
                    return Some(PathCommand::MoveTo(endpoint))
                }
                CubicPathCommand::LineTo(endpoint) => {
                    self.last_endpoint = endpoint;
                    return Some(PathCommand::LineTo(endpoint))
                }
                CubicPathCommand::QuadCurveTo(control_point, endpoint) => {
                    self.last_endpoint = endpoint;
                    return Some(PathCommand::CurveTo(control_point, endpoint))
                }
                CubicPathCommand::CubicCurveTo(control_point_0, control_point_1, endpoint) => {
                    let curve = CubicCurve::new(&self.last_endpoint,
                                                &control_point_0,
                                                &control_point_1,
                                                &endpoint);
                    self.last_endpoint = endpoint;
                    self.approx_curve_iter = Some(ApproxCurveIter::new(&curve, self.error_bound));
                }
            }
        }
    }
}

/// Approximates a single cubic Bézier curve with a series of quadratic Bézier curves.
pub struct ApproxCurveIter {
    curves: Vec<CubicCurve>,
    error_bound: f32,
    iteration: u8,
}

impl ApproxCurveIter {
    fn new(cubic: &CubicCurve, error_bound: f32) -> ApproxCurveIter {
        let (curve_a, curve_b) = cubic.subdivide(0.5);
        ApproxCurveIter {
            curves: vec![curve_b, curve_a],
            error_bound: error_bound,
            iteration: 0,
        }
    }
}

impl Iterator for ApproxCurveIter {
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
