// partitionfinder/geometry.rs

use euclid::Point2D;
use euclid::approxeq::ApproxEq;

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

    return Some(p + r * t)
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
    // TODO(pcwalton)
    Point2D::new(0.0, 0.0)
}

pub fn solve_line_y_for_x(x: f32, a: &Point2D<f32>, b: &Point2D<f32>) -> f32 {
    // TODO(pcwalton)
    0.0
}

pub fn solve_cubic_bezier_t_for_x(x: f32,
                                  p0: &Point2D<f32>,
                                  p1: &Point2D<f32>,
                                  p2: &Point2D<f32>,
                                  p3: &Point2D<f32>)
                                  -> f32 {
    // TODO(pcwalton)
    0.0
}

pub fn solve_cubic_bezier_y_for_x(x: f32,
                                  p0: &Point2D<f32>,
                                  p1: &Point2D<f32>,
                                  p2: &Point2D<f32>,
                                  p3: &Point2D<f32>)
                                  -> f32 {
    sample_cubic_bezier(solve_cubic_bezier_t_for_x(x, p0, p1, p2, p3), p0, p1, p2, p3).y
}
