// pathfinder/path-utils/src/intersection.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Intersections of two segments.

use euclid::{Point2D, Rect};

use curve::Curve;
use line::Line;
use lerp;

const SUBDIVISION_TOLERANCE: f32 = 0.0001;
const MAX_SUBDIVISIONS: u32 = 1000;

pub struct Intersection {
    pub t_a: f32,
    pub t_b: f32,
}

impl Intersection {
    /// Requires that any curves be monotonic. (See the `monotonic` module for that.)
    ///
    /// This should work for line segments, but it is inefficient.
    ///
    /// See T.W. Sederberg, "Computer Aided Geometric Design Course Notes" § 7.6.
    pub fn calculate<A, B>(a: &A, b: &B) -> Option<Intersection> where A: Intersect, B: Intersect {
        let (mut a_lower_t, mut a_upper_t) = (0.0, 1.0);
        let (mut b_lower_t, mut b_upper_t) = (0.0, 1.0);

        for _ in 0..MAX_SUBDIVISIONS {
            let a_lower_point = a.sample(a_lower_t);
            let a_upper_point = a.sample(a_upper_t);
            let b_lower_point = b.sample(b_lower_t);
            let b_upper_point = b.sample(b_upper_t);

            let a_distance = (a_upper_point - a_lower_point).length();
            let b_distance = (b_upper_point - b_lower_point).length();

            let need_to_subdivide_a = a_distance >= SUBDIVISION_TOLERANCE;
            let need_to_subdivide_b = b_distance >= SUBDIVISION_TOLERANCE;
            if !need_to_subdivide_b && !need_to_subdivide_a {
                break
            }

            let a_rect;
            if need_to_subdivide_a {
                let a_middle_t = lerp(a_lower_t, a_upper_t, 0.5);
                let a_middle_point = a.sample(a_middle_t);

                let a_lower_rect =
                    Rect::from_points(&[a_lower_point, a_middle_point]);
                let a_upper_rect =
                    Rect::from_points(&[a_middle_point, a_upper_point]);
                let b_rect = Rect::from_points(&[b_lower_point, b_upper_point]);

                if a_lower_rect.intersects(&b_rect) {
                    a_upper_t = a_middle_t;
                    a_rect = a_lower_rect;
                } else if a_upper_rect.intersects(&b_rect) {
                    a_lower_t = a_middle_t;
                    a_rect = a_upper_rect;
                } else {
                    return None
                }
            } else {
                a_rect = Rect::from_points(&[a_lower_point, a_upper_point])
            }

            if need_to_subdivide_b {
                let b_middle_t = lerp(b_lower_t, b_upper_t, 0.5);
                let b_middle_point = b.sample(b_middle_t);

                let b_lower_rect = Rect::from_points(&[b_lower_point, b_middle_point]);
                let b_upper_rect = Rect::from_points(&[b_middle_point, b_upper_point]);

                if b_lower_rect.intersects(&a_rect) {
                    b_upper_t = b_middle_t
                } else if b_upper_rect.intersects(&a_rect) {
                    b_lower_t = b_middle_t
                } else {
                    return None
                }
            }
        }

        Some(Intersection {
            t_a: lerp(a_lower_t, a_upper_t, 0.5),
            t_b: lerp(b_lower_t, b_upper_t, 0.5),
        })
    }
}

pub trait Intersect {
    fn sample(&self, t: f32) -> Point2D<f32>;
}

impl Intersect for Line {
    #[inline]
    fn sample(&self, t: f32) -> Point2D<f32> {
        Line::sample(self, t)
    }
}

impl Intersect for Curve {
    #[inline]
    fn sample(&self, t: f32) -> Point2D<f32> {
        Curve::sample(self, t)
    }
}
