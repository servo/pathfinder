// pathfinder/geometry/src/util.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Various utilities.

use std::f32;
use crate::transform2d::{Transform2F, Matrix2x2F};
use crate::vector::Vector2F;

pub const EPSILON: f32 = 0.001;

/// Approximate equality.
#[inline]
pub fn approx_eq(a: f32, b: f32) -> bool {
    f32::abs(a - b) <= EPSILON
}

/// Linear interpolation.
#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Clamping.
#[inline]
pub fn clamp(x: f32, min_val: f32, max_val: f32) -> f32 {
    f32::min(max_val, f32::max(min_val, x))
}

/// Divides `a` by `b`, rounding up.
#[inline]
pub fn alignup_i32(a: i32, b: i32) -> i32 {
    (a + b - 1) / b
}

pub fn reflection(a: Vector2F, b: Vector2F) -> Transform2F {
    let l = b - a;
    let l2 = l * l;
    let l2_yx = l2.yx();
    let d = l2 - l2_yx;
    let lxy2 = 2.0 * l.x() * l.y();
    let s = 1.0 / (l2.x() + l2.y());

    Transform2F::from_translation(-a) * Transform2F {
        matrix: Matrix2x2F::row_major(d.x(), lxy2, lxy2, d.y()).scale(s),
        vector: a
    }
}
