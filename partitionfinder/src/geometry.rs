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
pub fn line_quadratic_bezier_crossing_point(_a_p0: &Point2D<f32>,
                                            _a_p1: &Point2D<f32>,
                                            _b_p0: &Point2D<f32>,
                                            _b_p1: &Point2D<f32>,
                                            _b_p2: &Point2D<f32>)
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

fn sample_quadratic_bezier(t: f32, p0: &Point2D<f32>, p1: &Point2D<f32>, p2: &Point2D<f32>)
                           -> Point2D<f32> {
    p0.lerp(*p1, t).lerp(p1.lerp(*p2, t), t)
}

pub fn sample_quadratic_bezier_deriv(t: f32,
                                     p0: &Point2D<f32>,
                                     p1: &Point2D<f32>,
                                     p2: &Point2D<f32>)
                                     -> Vector2D<f32> {
    // https://en.wikipedia.org/wiki/B%C3%A9zier_curve#Quadratic_B.C3.A9zier_curves
    // FIXME(pcwalton): Can this be made faster?
    return ((*p1 - *p0) * (1.0 - t) + (*p2 - *p1) * t) * 2.0
}

pub fn sample_quadratic_bezier_deriv_deriv(p0: &Point2D<f32>,
                                           p1: &Point2D<f32>,
                                           p2: &Point2D<f32>)
                                           -> Vector2D<f32> {
    // https://en.wikipedia.org/wiki/B%C3%A9zier_curve#Quadratic_B.C3.A9zier_curves
    // FIXME(pcwalton): Can this be made faster?
    (*p2 - *p1 * 2.0 + p0.to_vector()) * 2.0
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

pub fn solve_quadratic_bezier_t_for_x(x: f32,
                                      p0: &Point2D<f32>,
                                      p1: &Point2D<f32>,
                                      p2: &Point2D<f32>)
                                      -> f32 {
    // TODO(pcwalton): Use the quadratic equation instead.
    newton_raphson(|t| sample_quadratic_bezier(t, p0, p1, p2).x - x,
                   |t| sample_quadratic_bezier_deriv(t, p0, p1, p2).x,
                   0.5)
}

pub fn solve_quadratic_bezier_y_for_x(x: f32,
                                      p0: &Point2D<f32>,
                                      p1: &Point2D<f32>,
                                      p2: &Point2D<f32>)
                                      -> f32 {
    sample_quadratic_bezier(solve_quadratic_bezier_t_for_x(x, p0, p1, p2), p0, p1, p2).y
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
