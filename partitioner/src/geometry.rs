// pathfinder/partitioner/src/geometry.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use euclid::Point2D;
use euclid::approxeq::ApproxEq;

fn quadratic_bezier_axis_inflection_point(p0: f32, p1: f32, p2: f32) -> Option<f32> {
    let t = (p0 - p1) / (p0 - 2.0 * p1 + p2);
    if t > f32::approx_epsilon() && t < 1.0 - f32::approx_epsilon() {
        Some(t)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug)]
pub struct QuadraticBezierInflectionPoints {
    pub xt: Option<f32>,
    pub yt: Option<f32>,
}

impl QuadraticBezierInflectionPoints {
    pub fn calculate(p0: &Point2D<f32>, p1: &Point2D<f32>, p2: &Point2D<f32>)
                     -> QuadraticBezierInflectionPoints {
        if ((*p1 - *p0).length() + (*p2 - *p1).length()).abs() < f32::approx_epsilon() {
            QuadraticBezierInflectionPoints {
                xt: None,
                yt: None,
            }
        } else {
            QuadraticBezierInflectionPoints {
                xt: quadratic_bezier_axis_inflection_point(p0.x, p1.x, p2.x),
                yt: quadratic_bezier_axis_inflection_point(p0.y, p1.y, p2.y),
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SubdividedQuadraticBezier {
    pub ap0: Point2D<f32>,
    pub ap1: Point2D<f32>,
    pub ap2bp0: Point2D<f32>,
    pub bp1: Point2D<f32>,
    pub bp2: Point2D<f32>,
}

impl SubdividedQuadraticBezier {
    pub fn new(t: f32, p0: &Point2D<f32>, p1: &Point2D<f32>, p2: &Point2D<f32>)
               -> SubdividedQuadraticBezier {
        let (ap1, bp1) = (p0.lerp(*p1, t), p1.lerp(*p2, t));
        let ap2bp0 = ap1.lerp(bp1, t);
        SubdividedQuadraticBezier {
            ap0: *p0,
            ap1: ap1,
            ap2bp0: ap2bp0,
            bp1: bp1,
            bp2: *p2,
        }
    }
}
