// partitionfinder/geometry.rs

use euclid::approxeq::ApproxEq;
use euclid::{Point2D, Vector2D};
use std::cmp::Ordering;

const NEWTON_RAPHSON_ITERATIONS: u8 = 8;

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
pub fn line_cubic_bezier_crossing_point(_a_p0: &Point2D<f32>,
                                        _a_p1: &Point2D<f32>,
                                        _b_p0: &Point2D<f32>,
                                        _b_p1: &Point2D<f32>,
                                        _b_p2: &Point2D<f32>,
                                        _b_p3: &Point2D<f32>)
                                        -> Option<Point2D<f32>> {
    None
}

// TODO(pcwalton): Implement this.
pub fn cubic_bezier_cubic_bezier_crossing_point(_a_p0: &Point2D<f32>,
                                                _a_p1: &Point2D<f32>,
                                                _a_p2: &Point2D<f32>,
                                                _a_p3: &Point2D<f32>,
                                                _b_p0: &Point2D<f32>,
                                                _b_p1: &Point2D<f32>,
                                                _b_p2: &Point2D<f32>,
                                                _b_p3: &Point2D<f32>)
                                                -> Option<Point2D<f32>> {
    None
}

fn sample_cubic_bezier(t: f32,
                       p0: &Point2D<f32>,
                       p1: &Point2D<f32>,
                       p2: &Point2D<f32>,
                       p3: &Point2D<f32>)
                       -> Point2D<f32> {
    let (p0p1, p1p2, p2p3) = (p0.lerp(*p1, t), p1.lerp(*p2, t), p2.lerp(*p3, t));
    let (p0p1p2, p1p2p3) = (p0p1.lerp(p1p2, t), p1p2.lerp(p2p3, t));
    p0p1p2.lerp(p1p2p3, t)
}

pub fn sample_cubic_bezier_deriv(t: f32,
                                 p0: &Point2D<f32>,
                                 p1: &Point2D<f32>,
                                 p2: &Point2D<f32>,
                                 p3: &Point2D<f32>)
                                 -> Vector2D<f32> {
    // https://en.wikipedia.org/wiki/B%C3%A9zier_curve#Cubic_B.C3.A9zier_curves
    // FIXME(pcwalton): Can this be made faster?
    let tt = 1.0 - t;
    return (*p1 - *p0) * 3.0 * tt * tt + (*p2 - *p1) * 6.0 * tt * t + (*p3 - *p2) * 3.0 * t * t
}

pub fn sample_cubic_bezier_deriv_deriv(t: f32,
                                       p0: &Point2D<f32>,
                                       p1: &Point2D<f32>,
                                       p2: &Point2D<f32>,
                                       p3: &Point2D<f32>)
                                       -> Vector2D<f32> {
    // https://en.wikipedia.org/wiki/B%C3%A9zier_curve#Cubic_B.C3.A9zier_curves
    // FIXME(pcwalton): Can this be made faster?
    (*p2 - *p1 * 2.0 + p0.to_vector()).lerp(*p3 - *p2 * 2.0 + p1.to_vector(), t) * 6.0
}

pub fn solve_line_y_for_x(x: f32, a: &Point2D<f32>, b: &Point2D<f32>) -> f32 {
    a.lerp(*b, (x - a.x) / (b.x - a.x)).y
}

pub(crate) fn newton_raphson<F, DFDX>(f: F, dfdx: DFDX, mut x_guess: f32) -> f32
                                      where F: Fn(f32) -> f32, DFDX: Fn(f32) -> f32 {
    for _ in 0..NEWTON_RAPHSON_ITERATIONS {
        let y = f(x_guess);
        if y.approx_eq(&0.0) {
            break
        }
        let yy = dfdx(x_guess);
        x_guess -= y / yy
    }
    x_guess
}

pub fn solve_cubic_bezier_t_for_x(x: f32,
                                  p0: &Point2D<f32>,
                                  p1: &Point2D<f32>,
                                  p2: &Point2D<f32>,
                                  p3: &Point2D<f32>)
                                  -> f32 {
    newton_raphson(|t| sample_cubic_bezier(t, p0, p1, p2, p3).x - x,
                   |t| sample_cubic_bezier_deriv(t, p0, p1, p2, p3).x,
                   0.5)
}

pub fn solve_cubic_bezier_y_for_x(x: f32,
                                  p0: &Point2D<f32>,
                                  p1: &Point2D<f32>,
                                  p2: &Point2D<f32>,
                                  p3: &Point2D<f32>)
                                  -> f32 {
    sample_cubic_bezier(solve_cubic_bezier_t_for_x(x, p0, p1, p2, p3), p0, p1, p2, p3).y
}

#[derive(Clone, Copy, Debug)]
pub struct SubdividedCubicBezier {
    pub ap0: Point2D<f32>,
    pub ap1: Point2D<f32>,
    pub ap2: Point2D<f32>,
    pub ap3bp0: Point2D<f32>,
    pub bp1: Point2D<f32>,
    pub bp2: Point2D<f32>,
    pub bp3: Point2D<f32>,
}

impl SubdividedCubicBezier {
    pub fn new(t: f32, p0: &Point2D<f32>, p1: &Point2D<f32>, p2: &Point2D<f32>, p3: &Point2D<f32>)
               -> SubdividedCubicBezier {
        let (ap1, p1p2, bp2) = (p0.lerp(*p1, t), p1.lerp(*p2, t), p2.lerp(*p3, t));
        let (ap2, bp1) = (ap1.lerp(p1p2, t), (p1p2.lerp(bp2, t)));
        let ap3bp0 = ap2.lerp(bp1, t);
        SubdividedCubicBezier {
            ap0: *p0,
            ap1: ap1,
            ap2: ap2,
            ap3bp0: ap3bp0,
            bp1: bp1,
            bp2: bp2,
            bp3: *p3,
        }
    }
}
