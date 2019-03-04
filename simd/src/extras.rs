// pathfinder/simd/src/extras.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::default::F32x4;

impl F32x4 {
    // Constructors

    #[inline]
    pub fn from_slice(slice: &[f32]) -> F32x4 {
        F32x4::new(slice[0], slice[1], slice[2], slice[3])
    }

    // Accessors

    #[inline]
    pub fn x(self) -> f32 {
        self[0]
    }

    #[inline]
    pub fn y(self) -> f32 {
        self[1]
    }

    #[inline]
    pub fn z(self) -> f32 {
        self[2]
    }

    #[inline]
    pub fn w(self) -> f32 {
        self[3]
    }

    // Mutators

    #[inline]
    pub fn set_x(&mut self, x: f32) {
        self[0] = x
    }

    #[inline]
    pub fn set_y(&mut self, y: f32) {
        self[1] = y
    }

    #[inline]
    pub fn set_z(&mut self, z: f32) {
        self[2] = z
    }

    #[inline]
    pub fn set_w(&mut self, w: f32) {
        self[3] = w
    }

    // Comparisons

    #[inline]
    pub fn approx_eq(self, other: F32x4, epsilon: f32) -> bool {
        (self - other)
            .abs()
            .packed_gt(F32x4::splat(epsilon))
            .is_all_zeroes()
    }
}
