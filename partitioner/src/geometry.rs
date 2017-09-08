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
use std::cmp::Ordering;

pub(crate) trait ApproxOrdered {
    fn approx_ordered(&self) -> bool;
}

impl ApproxOrdered for [f32] {
    fn approx_ordered(&self) -> bool {
        let mut last_ordering = Ordering::Equal;
        for window in self.windows(2) {
            let (last_value, this_value) = (window[0], window[1]);
            let this_ordering = if last_value - this_value < -f32::approx_epsilon() {
                Ordering::Less
            } else if last_value - this_value > f32::approx_epsilon() {
                Ordering::Greater
            } else {
                Ordering::Equal
            };
            if last_ordering != Ordering::Equal && this_ordering != last_ordering {
                return false
            }
            last_ordering = this_ordering
        }
        true
    }
}

// https://stackoverflow.com/a/565282
pub fn line_line_crossing_point(a_p0: &Point2D<f32>,
                                a_p1: &Point2D<f32>,
                                b_p0: &Point2D<f32>,
                                b_p1: &Point2D<f32>)
                                -> Option<Point2D<f32>> {
    let (p, r) = (*a_p0, *a_p1 - *a_p0);
    let (q, s) = (*b_p0, *b_p1 - *b_p0);

    let rs = r.cross(s);
    if rs.approx_eq(&0.0) {
        return None
    }

    let t = (q - p).cross(s) / rs;
    if t < f32::approx_epsilon() || t > 1.0f32 - f32::approx_epsilon() {
        return None
    }

    let u = (q - p).cross(r) / rs;
    if u < f32::approx_epsilon() || u > 1.0f32 - f32::approx_epsilon() {
        return None
    }

    Some(p + r * t)
}

// TODO(pcwalton): Implement this.
pub fn line_quadratic_bezier_crossing_point(a_p0: &Point2D<f32>,
                                            a_p1: &Point2D<f32>,
                                            b_p0: &Point2D<f32>,
                                            b_p1: &Point2D<f32>,
                                            b_p2: &Point2D<f32>)
                                            -> Option<Point2D<f32>> {
    None
}

// TODO(pcwalton): Implement this.
pub fn quadratic_bezier_quadratic_bezier_crossing_point(_a_p0: &Point2D<f32>,
                                                        _a_p1: &Point2D<f32>,
                                                        _a_p2: &Point2D<f32>,
                                                        _b_p0: &Point2D<f32>,
                                                        _b_p1: &Point2D<f32>,
                                                        _b_p2: &Point2D<f32>)
                                                        -> Option<Point2D<f32>> {
    None
}

pub fn sample_quadratic_bezier(t: f32, p0: &Point2D<f32>, p1: &Point2D<f32>, p2: &Point2D<f32>)
                               -> Point2D<f32> {
    p0.lerp(*p1, t).lerp(p1.lerp(*p2, t), t)
}

pub fn solve_line_t_for_x(x: f32, a: &Point2D<f32>, b: &Point2D<f32>) -> f32 {
    if b.x == a.x {
        0.0
    } else {
        (x - a.x) / (b.x - a.x)
    }
}

// Use the Citardauq Formula to avoid precision problems.
//
// https://math.stackexchange.com/a/311397
pub fn solve_quadratic_bezier_t_for_x(x: f32,
                                      p0: &Point2D<f32>,
                                      p1: &Point2D<f32>,
                                      p2: &Point2D<f32>)
                                      -> f32 {
    let (p0x, p1x, p2x, x) = (p0.x as f64, p1.x as f64, p2.x as f64, x as f64);

    let a = p0x - 2.0 * p1x + p2x;
    let b = -2.0 * p0x + 2.0 * p1x;
    let c = p0x - x;

    let t = 2.0 * c / (-b - (b * b - 4.0 * a * c).sqrt());
    t.max(0.0).min(1.0) as f32
}

pub fn solve_quadratic_bezier_y_for_x(x: f32,
                                      p0: &Point2D<f32>,
                                      p1: &Point2D<f32>,
                                      p2: &Point2D<f32>)
                                      -> f32 {
    sample_quadratic_bezier(solve_quadratic_bezier_t_for_x(x, p0, p1, p2), p0, p1, p2).y
}

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
