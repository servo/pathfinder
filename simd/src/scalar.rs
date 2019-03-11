// pathfinder/simd/src/scalar.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::f32;
use std::fmt::{self, Debug, Formatter};
use std::mem;
use std::ops::{Add, Index, IndexMut, Mul, Sub};

// 32-bit floats

#[derive(Clone, Copy, Default, PartialEq)]
pub struct F32x4(pub [f32; 4]);

impl F32x4 {
    #[inline]
    pub fn new(a: f32, b: f32, c: f32, d: f32) -> F32x4 {
        F32x4([a, b, c, d])
    }

    #[inline]
    pub fn splat(x: f32) -> F32x4 {
        F32x4([x; 4])
    }

    // Basic operations

    #[inline]
    pub fn min(self, other: F32x4) -> F32x4 {
        F32x4([
            self[0].min(other[0]),
            self[1].min(other[1]),
            self[2].min(other[2]),
            self[3].min(other[3]),
        ])
    }

    #[inline]
    pub fn max(self, other: F32x4) -> F32x4 {
        F32x4([
            self[0].max(other[0]),
            self[1].max(other[1]),
            self[2].max(other[2]),
            self[3].max(other[3]),
        ])
    }

    #[inline]
    pub fn clamp(self, min: F32x4, max: F32x4) -> F32x4 {
        self.max(min).min(max)
    }

    #[inline]
    pub fn abs(self) -> F32x4 {
        F32x4([self[0].abs(), self[1].abs(), self[2].abs(), self[3].abs()])
    }

    #[inline]
    pub fn floor(self) -> F32x4 {
        F32x4([self[0].floor(), self[1].floor(), self[2].floor(), self[3].floor()])
    }

    #[inline]
    pub fn ceil(self) -> F32x4 {
        F32x4([self[0].ceil(), self[1].ceil(), self[2].ceil(), self[3].ceil()])
    }

    // Packed comparisons

    #[inline]
    pub fn packed_eq(self, other: F32x4) -> U32x4 {
        U32x4([
            if self[0] == other[0] { !0 } else { 0 },
            if self[1] == other[1] { !0 } else { 0 },
            if self[2] == other[2] { !0 } else { 0 },
            if self[3] == other[3] { !0 } else { 0 },
        ])
    }

    #[inline]
    pub fn packed_gt(self, other: F32x4) -> U32x4 {
        U32x4([
            if self[0] > other[0] { !0 } else { 0 },
            if self[1] > other[1] { !0 } else { 0 },
            if self[2] > other[2] { !0 } else { 0 },
            if self[3] > other[3] { !0 } else { 0 },
        ])
    }

    #[inline]
    pub fn packed_le(self, other: F32x4) -> U32x4 {
        U32x4([
            if self[0] <= other[0] { !0 } else { 0 },
            if self[1] <= other[1] { !0 } else { 0 },
            if self[2] <= other[2] { !0 } else { 0 },
            if self[3] <= other[3] { !0 } else { 0 },
        ])
    }

    #[inline]
    pub fn packed_lt(self, other: F32x4) -> U32x4 {
        U32x4([
            if self[0] < other[0] { !0 } else { 0 },
            if self[1] < other[1] { !0 } else { 0 },
            if self[2] < other[2] { !0 } else { 0 },
            if self[3] < other[3] { !0 } else { 0 },
        ])
    }

    // Converts these packed floats to integers.
    #[inline]
    pub fn to_i32x4(self) -> I32x4 {
        I32x4([self[0] as i32, self[1] as i32, self[2] as i32, self[3] as i32])
    }

    // Shuffles

    /// Constructs a new vector from the first, first, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxxx(self) -> F32x4 {
        F32x4([self[0], self[0], self[0], self[0]])
    }

    /// Constructs a new vector from the second, first, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxxx(self) -> F32x4 {
        F32x4([self[1], self[0], self[0], self[0]])
    }

    /// Constructs a new vector from the third, first, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxxx(self) -> F32x4 {
        F32x4([self[2], self[0], self[0], self[0]])
    }

    /// Constructs a new vector from the fourth, first, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxxx(self) -> F32x4 {
        F32x4([self[3], self[0], self[0], self[0]])
    }

    /// Constructs a new vector from the first, second, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyxx(self) -> F32x4 {
        F32x4([self[0], self[1], self[0], self[0]])
    }

    /// Constructs a new vector from the second, second, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyxx(self) -> F32x4 {
        F32x4([self[1], self[1], self[0], self[0]])
    }

    /// Constructs a new vector from the third, second, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyxx(self) -> F32x4 {
        F32x4([self[2], self[1], self[0], self[0]])
    }

    /// Constructs a new vector from the fourth, second, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyxx(self) -> F32x4 {
        F32x4([self[3], self[1], self[0], self[0]])
    }

    /// Constructs a new vector from the first, third, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzxx(self) -> F32x4 {
        F32x4([self[0], self[2], self[0], self[0]])
    }

    /// Constructs a new vector from the second, third, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzxx(self) -> F32x4 {
        F32x4([self[1], self[2], self[0], self[0]])
    }

    /// Constructs a new vector from the third, third, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzxx(self) -> F32x4 {
        F32x4([self[2], self[2], self[0], self[0]])
    }

    /// Constructs a new vector from the fourth, third, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzxx(self) -> F32x4 {
        F32x4([self[3], self[2], self[0], self[0]])
    }

    /// Constructs a new vector from the first, fourth, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwxx(self) -> F32x4 {
        F32x4([self[0], self[3], self[0], self[0]])
    }

    /// Constructs a new vector from the second, fourth, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywxx(self) -> F32x4 {
        F32x4([self[1], self[3], self[0], self[0]])
    }

    /// Constructs a new vector from the third, fourth, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwxx(self) -> F32x4 {
        F32x4([self[2], self[3], self[0], self[0]])
    }

    /// Constructs a new vector from the fourth, fourth, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwxx(self) -> F32x4 {
        F32x4([self[3], self[3], self[0], self[0]])
    }

    /// Constructs a new vector from the first, first, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxyx(self) -> F32x4 {
        F32x4([self[0], self[0], self[1], self[0]])
    }

    /// Constructs a new vector from the second, first, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxyx(self) -> F32x4 {
        F32x4([self[1], self[0], self[1], self[0]])
    }

    /// Constructs a new vector from the third, first, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxyx(self) -> F32x4 {
        F32x4([self[2], self[0], self[1], self[0]])
    }

    /// Constructs a new vector from the fourth, first, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxyx(self) -> F32x4 {
        F32x4([self[3], self[0], self[1], self[0]])
    }

    /// Constructs a new vector from the first, second, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyyx(self) -> F32x4 {
        F32x4([self[0], self[1], self[1], self[0]])
    }

    /// Constructs a new vector from the second, second, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyyx(self) -> F32x4 {
        F32x4([self[1], self[1], self[1], self[0]])
    }

    /// Constructs a new vector from the third, second, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyyx(self) -> F32x4 {
        F32x4([self[2], self[1], self[1], self[0]])
    }

    /// Constructs a new vector from the fourth, second, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyyx(self) -> F32x4 {
        F32x4([self[3], self[1], self[1], self[0]])
    }

    /// Constructs a new vector from the first, third, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzyx(self) -> F32x4 {
        F32x4([self[0], self[2], self[1], self[0]])
    }

    /// Constructs a new vector from the second, third, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzyx(self) -> F32x4 {
        F32x4([self[1], self[2], self[1], self[0]])
    }

    /// Constructs a new vector from the third, third, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzyx(self) -> F32x4 {
        F32x4([self[2], self[2], self[1], self[0]])
    }

    /// Constructs a new vector from the fourth, third, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzyx(self) -> F32x4 {
        F32x4([self[3], self[2], self[1], self[0]])
    }

    /// Constructs a new vector from the first, fourth, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwyx(self) -> F32x4 {
        F32x4([self[0], self[3], self[1], self[0]])
    }

    /// Constructs a new vector from the second, fourth, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywyx(self) -> F32x4 {
        F32x4([self[1], self[3], self[1], self[0]])
    }

    /// Constructs a new vector from the third, fourth, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwyx(self) -> F32x4 {
        F32x4([self[2], self[3], self[1], self[0]])
    }

    /// Constructs a new vector from the fourth, fourth, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwyx(self) -> F32x4 {
        F32x4([self[3], self[3], self[1], self[0]])
    }

    /// Constructs a new vector from the first, first, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxzx(self) -> F32x4 {
        F32x4([self[0], self[0], self[2], self[0]])
    }

    /// Constructs a new vector from the second, first, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxzx(self) -> F32x4 {
        F32x4([self[1], self[0], self[2], self[0]])
    }

    /// Constructs a new vector from the third, first, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxzx(self) -> F32x4 {
        F32x4([self[2], self[0], self[2], self[0]])
    }

    /// Constructs a new vector from the fourth, first, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxzx(self) -> F32x4 {
        F32x4([self[3], self[0], self[2], self[0]])
    }

    /// Constructs a new vector from the first, second, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyzx(self) -> F32x4 {
        F32x4([self[0], self[1], self[2], self[0]])
    }

    /// Constructs a new vector from the second, second, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyzx(self) -> F32x4 {
        F32x4([self[1], self[1], self[2], self[0]])
    }

    /// Constructs a new vector from the third, second, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyzx(self) -> F32x4 {
        F32x4([self[2], self[1], self[2], self[0]])
    }

    /// Constructs a new vector from the fourth, second, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyzx(self) -> F32x4 {
        F32x4([self[3], self[1], self[2], self[0]])
    }

    /// Constructs a new vector from the first, third, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzzx(self) -> F32x4 {
        F32x4([self[0], self[2], self[2], self[0]])
    }

    /// Constructs a new vector from the second, third, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzzx(self) -> F32x4 {
        F32x4([self[1], self[2], self[2], self[0]])
    }

    /// Constructs a new vector from the third, third, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzzx(self) -> F32x4 {
        F32x4([self[2], self[2], self[2], self[0]])
    }

    /// Constructs a new vector from the fourth, third, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzzx(self) -> F32x4 {
        F32x4([self[3], self[2], self[2], self[0]])
    }

    /// Constructs a new vector from the first, fourth, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwzx(self) -> F32x4 {
        F32x4([self[0], self[3], self[2], self[0]])
    }

    /// Constructs a new vector from the second, fourth, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywzx(self) -> F32x4 {
        F32x4([self[1], self[3], self[2], self[0]])
    }

    /// Constructs a new vector from the third, fourth, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwzx(self) -> F32x4 {
        F32x4([self[2], self[3], self[2], self[0]])
    }

    /// Constructs a new vector from the fourth, fourth, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwzx(self) -> F32x4 {
        F32x4([self[3], self[3], self[2], self[0]])
    }

    /// Constructs a new vector from the first, first, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxwx(self) -> F32x4 {
        F32x4([self[0], self[0], self[3], self[0]])
    }

    /// Constructs a new vector from the second, first, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxwx(self) -> F32x4 {
        F32x4([self[1], self[0], self[3], self[0]])
    }

    /// Constructs a new vector from the third, first, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxwx(self) -> F32x4 {
        F32x4([self[2], self[0], self[3], self[0]])
    }

    /// Constructs a new vector from the fourth, first, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxwx(self) -> F32x4 {
        F32x4([self[3], self[0], self[3], self[0]])
    }

    /// Constructs a new vector from the first, second, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xywx(self) -> F32x4 {
        F32x4([self[0], self[1], self[3], self[0]])
    }

    /// Constructs a new vector from the second, second, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yywx(self) -> F32x4 {
        F32x4([self[1], self[1], self[3], self[0]])
    }

    /// Constructs a new vector from the third, second, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zywx(self) -> F32x4 {
        F32x4([self[2], self[1], self[3], self[0]])
    }

    /// Constructs a new vector from the fourth, second, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wywx(self) -> F32x4 {
        F32x4([self[3], self[1], self[3], self[0]])
    }

    /// Constructs a new vector from the first, third, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzwx(self) -> F32x4 {
        F32x4([self[0], self[2], self[3], self[0]])
    }

    /// Constructs a new vector from the second, third, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzwx(self) -> F32x4 {
        F32x4([self[1], self[2], self[3], self[0]])
    }

    /// Constructs a new vector from the third, third, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzwx(self) -> F32x4 {
        F32x4([self[2], self[2], self[3], self[0]])
    }

    /// Constructs a new vector from the fourth, third, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzwx(self) -> F32x4 {
        F32x4([self[3], self[2], self[3], self[0]])
    }

    /// Constructs a new vector from the first, fourth, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwwx(self) -> F32x4 {
        F32x4([self[0], self[3], self[3], self[0]])
    }

    /// Constructs a new vector from the second, fourth, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywwx(self) -> F32x4 {
        F32x4([self[1], self[3], self[3], self[0]])
    }

    /// Constructs a new vector from the third, fourth, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwwx(self) -> F32x4 {
        F32x4([self[2], self[3], self[3], self[0]])
    }

    /// Constructs a new vector from the fourth, fourth, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwwx(self) -> F32x4 {
        F32x4([self[3], self[3], self[3], self[0]])
    }

    /// Constructs a new vector from the first, first, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxxy(self) -> F32x4 {
        F32x4([self[0], self[0], self[0], self[1]])
    }

    /// Constructs a new vector from the second, first, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxxy(self) -> F32x4 {
        F32x4([self[1], self[0], self[0], self[1]])
    }

    /// Constructs a new vector from the third, first, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxxy(self) -> F32x4 {
        F32x4([self[2], self[0], self[0], self[1]])
    }

    /// Constructs a new vector from the fourth, first, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxxy(self) -> F32x4 {
        F32x4([self[3], self[0], self[0], self[1]])
    }

    /// Constructs a new vector from the first, second, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyxy(self) -> F32x4 {
        F32x4([self[0], self[1], self[0], self[1]])
    }

    /// Constructs a new vector from the second, second, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyxy(self) -> F32x4 {
        F32x4([self[1], self[1], self[0], self[1]])
    }

    /// Constructs a new vector from the third, second, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyxy(self) -> F32x4 {
        F32x4([self[2], self[1], self[0], self[1]])
    }

    /// Constructs a new vector from the fourth, second, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyxy(self) -> F32x4 {
        F32x4([self[3], self[1], self[0], self[1]])
    }

    /// Constructs a new vector from the first, third, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzxy(self) -> F32x4 {
        F32x4([self[0], self[2], self[0], self[1]])
    }

    /// Constructs a new vector from the second, third, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzxy(self) -> F32x4 {
        F32x4([self[1], self[2], self[0], self[1]])
    }

    /// Constructs a new vector from the third, third, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzxy(self) -> F32x4 {
        F32x4([self[2], self[2], self[0], self[1]])
    }

    /// Constructs a new vector from the fourth, third, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzxy(self) -> F32x4 {
        F32x4([self[3], self[2], self[0], self[1]])
    }

    /// Constructs a new vector from the first, fourth, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwxy(self) -> F32x4 {
        F32x4([self[0], self[3], self[0], self[1]])
    }

    /// Constructs a new vector from the second, fourth, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywxy(self) -> F32x4 {
        F32x4([self[1], self[3], self[0], self[1]])
    }

    /// Constructs a new vector from the third, fourth, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwxy(self) -> F32x4 {
        F32x4([self[2], self[3], self[0], self[1]])
    }

    /// Constructs a new vector from the fourth, fourth, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwxy(self) -> F32x4 {
        F32x4([self[3], self[3], self[0], self[1]])
    }

    /// Constructs a new vector from the first, first, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxyy(self) -> F32x4 {
        F32x4([self[0], self[0], self[1], self[1]])
    }

    /// Constructs a new vector from the second, first, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxyy(self) -> F32x4 {
        F32x4([self[1], self[0], self[1], self[1]])
    }

    /// Constructs a new vector from the third, first, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxyy(self) -> F32x4 {
        F32x4([self[2], self[0], self[1], self[1]])
    }

    /// Constructs a new vector from the fourth, first, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxyy(self) -> F32x4 {
        F32x4([self[3], self[0], self[1], self[1]])
    }

    /// Constructs a new vector from the first, second, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyyy(self) -> F32x4 {
        F32x4([self[0], self[1], self[1], self[1]])
    }

    /// Constructs a new vector from the second, second, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyyy(self) -> F32x4 {
        F32x4([self[1], self[1], self[1], self[1]])
    }

    /// Constructs a new vector from the third, second, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyyy(self) -> F32x4 {
        F32x4([self[2], self[1], self[1], self[1]])
    }

    /// Constructs a new vector from the fourth, second, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyyy(self) -> F32x4 {
        F32x4([self[3], self[1], self[1], self[1]])
    }

    /// Constructs a new vector from the first, third, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzyy(self) -> F32x4 {
        F32x4([self[0], self[2], self[1], self[1]])
    }

    /// Constructs a new vector from the second, third, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzyy(self) -> F32x4 {
        F32x4([self[1], self[2], self[1], self[1]])
    }

    /// Constructs a new vector from the third, third, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzyy(self) -> F32x4 {
        F32x4([self[2], self[2], self[1], self[1]])
    }

    /// Constructs a new vector from the fourth, third, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzyy(self) -> F32x4 {
        F32x4([self[3], self[2], self[1], self[1]])
    }

    /// Constructs a new vector from the first, fourth, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwyy(self) -> F32x4 {
        F32x4([self[0], self[3], self[1], self[1]])
    }

    /// Constructs a new vector from the second, fourth, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywyy(self) -> F32x4 {
        F32x4([self[1], self[3], self[1], self[1]])
    }

    /// Constructs a new vector from the third, fourth, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwyy(self) -> F32x4 {
        F32x4([self[2], self[3], self[1], self[1]])
    }

    /// Constructs a new vector from the fourth, fourth, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwyy(self) -> F32x4 {
        F32x4([self[3], self[3], self[1], self[1]])
    }

    /// Constructs a new vector from the first, first, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxzy(self) -> F32x4 {
        F32x4([self[0], self[0], self[2], self[1]])
    }

    /// Constructs a new vector from the second, first, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxzy(self) -> F32x4 {
        F32x4([self[1], self[0], self[2], self[1]])
    }

    /// Constructs a new vector from the third, first, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxzy(self) -> F32x4 {
        F32x4([self[2], self[0], self[2], self[1]])
    }

    /// Constructs a new vector from the fourth, first, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxzy(self) -> F32x4 {
        F32x4([self[3], self[0], self[2], self[1]])
    }

    /// Constructs a new vector from the first, second, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyzy(self) -> F32x4 {
        F32x4([self[0], self[1], self[2], self[1]])
    }

    /// Constructs a new vector from the second, second, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyzy(self) -> F32x4 {
        F32x4([self[1], self[1], self[2], self[1]])
    }

    /// Constructs a new vector from the third, second, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyzy(self) -> F32x4 {
        F32x4([self[2], self[1], self[2], self[1]])
    }

    /// Constructs a new vector from the fourth, second, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyzy(self) -> F32x4 {
        F32x4([self[3], self[1], self[2], self[1]])
    }

    /// Constructs a new vector from the first, third, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzzy(self) -> F32x4 {
        F32x4([self[0], self[2], self[2], self[1]])
    }

    /// Constructs a new vector from the second, third, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzzy(self) -> F32x4 {
        F32x4([self[1], self[2], self[2], self[1]])
    }

    /// Constructs a new vector from the third, third, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzzy(self) -> F32x4 {
        F32x4([self[2], self[2], self[2], self[1]])
    }

    /// Constructs a new vector from the fourth, third, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzzy(self) -> F32x4 {
        F32x4([self[3], self[2], self[2], self[1]])
    }

    /// Constructs a new vector from the first, fourth, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwzy(self) -> F32x4 {
        F32x4([self[0], self[3], self[2], self[1]])
    }

    /// Constructs a new vector from the second, fourth, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywzy(self) -> F32x4 {
        F32x4([self[1], self[3], self[2], self[1]])
    }

    /// Constructs a new vector from the third, fourth, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwzy(self) -> F32x4 {
        F32x4([self[2], self[3], self[2], self[1]])
    }

    /// Constructs a new vector from the fourth, fourth, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwzy(self) -> F32x4 {
        F32x4([self[3], self[3], self[2], self[1]])
    }

    /// Constructs a new vector from the first, first, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxwy(self) -> F32x4 {
        F32x4([self[0], self[0], self[3], self[1]])
    }

    /// Constructs a new vector from the second, first, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxwy(self) -> F32x4 {
        F32x4([self[1], self[0], self[3], self[1]])
    }

    /// Constructs a new vector from the third, first, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxwy(self) -> F32x4 {
        F32x4([self[2], self[0], self[3], self[1]])
    }

    /// Constructs a new vector from the fourth, first, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxwy(self) -> F32x4 {
        F32x4([self[3], self[0], self[3], self[1]])
    }

    /// Constructs a new vector from the first, second, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xywy(self) -> F32x4 {
        F32x4([self[0], self[1], self[3], self[1]])
    }

    /// Constructs a new vector from the second, second, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yywy(self) -> F32x4 {
        F32x4([self[1], self[1], self[3], self[1]])
    }

    /// Constructs a new vector from the third, second, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zywy(self) -> F32x4 {
        F32x4([self[2], self[1], self[3], self[1]])
    }

    /// Constructs a new vector from the fourth, second, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wywy(self) -> F32x4 {
        F32x4([self[3], self[1], self[3], self[1]])
    }

    /// Constructs a new vector from the first, third, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzwy(self) -> F32x4 {
        F32x4([self[0], self[2], self[3], self[1]])
    }

    /// Constructs a new vector from the second, third, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzwy(self) -> F32x4 {
        F32x4([self[1], self[2], self[3], self[1]])
    }

    /// Constructs a new vector from the third, third, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzwy(self) -> F32x4 {
        F32x4([self[2], self[2], self[3], self[1]])
    }

    /// Constructs a new vector from the fourth, third, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzwy(self) -> F32x4 {
        F32x4([self[3], self[2], self[3], self[1]])
    }

    /// Constructs a new vector from the first, fourth, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwwy(self) -> F32x4 {
        F32x4([self[0], self[3], self[3], self[1]])
    }

    /// Constructs a new vector from the second, fourth, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywwy(self) -> F32x4 {
        F32x4([self[1], self[3], self[3], self[1]])
    }

    /// Constructs a new vector from the third, fourth, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwwy(self) -> F32x4 {
        F32x4([self[2], self[3], self[3], self[1]])
    }

    /// Constructs a new vector from the fourth, fourth, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwwy(self) -> F32x4 {
        F32x4([self[3], self[3], self[3], self[1]])
    }

    /// Constructs a new vector from the first, first, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxxz(self) -> F32x4 {
        F32x4([self[0], self[0], self[0], self[2]])
    }

    /// Constructs a new vector from the second, first, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxxz(self) -> F32x4 {
        F32x4([self[1], self[0], self[0], self[2]])
    }

    /// Constructs a new vector from the third, first, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxxz(self) -> F32x4 {
        F32x4([self[2], self[0], self[0], self[2]])
    }

    /// Constructs a new vector from the fourth, first, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxxz(self) -> F32x4 {
        F32x4([self[3], self[0], self[0], self[2]])
    }

    /// Constructs a new vector from the first, second, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyxz(self) -> F32x4 {
        F32x4([self[0], self[1], self[0], self[2]])
    }

    /// Constructs a new vector from the second, second, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyxz(self) -> F32x4 {
        F32x4([self[1], self[1], self[0], self[2]])
    }

    /// Constructs a new vector from the third, second, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyxz(self) -> F32x4 {
        F32x4([self[2], self[1], self[0], self[2]])
    }

    /// Constructs a new vector from the fourth, second, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyxz(self) -> F32x4 {
        F32x4([self[3], self[1], self[0], self[2]])
    }

    /// Constructs a new vector from the first, third, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzxz(self) -> F32x4 {
        F32x4([self[0], self[2], self[0], self[2]])
    }

    /// Constructs a new vector from the second, third, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzxz(self) -> F32x4 {
        F32x4([self[1], self[2], self[0], self[2]])
    }

    /// Constructs a new vector from the third, third, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzxz(self) -> F32x4 {
        F32x4([self[2], self[2], self[0], self[2]])
    }

    /// Constructs a new vector from the fourth, third, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzxz(self) -> F32x4 {
        F32x4([self[3], self[2], self[0], self[2]])
    }

    /// Constructs a new vector from the first, fourth, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwxz(self) -> F32x4 {
        F32x4([self[0], self[3], self[0], self[2]])
    }

    /// Constructs a new vector from the second, fourth, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywxz(self) -> F32x4 {
        F32x4([self[1], self[3], self[0], self[2]])
    }

    /// Constructs a new vector from the third, fourth, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwxz(self) -> F32x4 {
        F32x4([self[2], self[3], self[0], self[2]])
    }

    /// Constructs a new vector from the fourth, fourth, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwxz(self) -> F32x4 {
        F32x4([self[3], self[3], self[0], self[2]])
    }

    /// Constructs a new vector from the first, first, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxyz(self) -> F32x4 {
        F32x4([self[0], self[0], self[1], self[2]])
    }

    /// Constructs a new vector from the second, first, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxyz(self) -> F32x4 {
        F32x4([self[1], self[0], self[1], self[2]])
    }

    /// Constructs a new vector from the third, first, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxyz(self) -> F32x4 {
        F32x4([self[2], self[0], self[1], self[2]])
    }

    /// Constructs a new vector from the fourth, first, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxyz(self) -> F32x4 {
        F32x4([self[3], self[0], self[1], self[2]])
    }

    /// Constructs a new vector from the first, second, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyyz(self) -> F32x4 {
        F32x4([self[0], self[1], self[1], self[2]])
    }

    /// Constructs a new vector from the second, second, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyyz(self) -> F32x4 {
        F32x4([self[1], self[1], self[1], self[2]])
    }

    /// Constructs a new vector from the third, second, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyyz(self) -> F32x4 {
        F32x4([self[2], self[1], self[1], self[2]])
    }

    /// Constructs a new vector from the fourth, second, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyyz(self) -> F32x4 {
        F32x4([self[3], self[1], self[1], self[2]])
    }

    /// Constructs a new vector from the first, third, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzyz(self) -> F32x4 {
        F32x4([self[0], self[2], self[1], self[2]])
    }

    /// Constructs a new vector from the second, third, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzyz(self) -> F32x4 {
        F32x4([self[1], self[2], self[1], self[2]])
    }

    /// Constructs a new vector from the third, third, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzyz(self) -> F32x4 {
        F32x4([self[2], self[2], self[1], self[2]])
    }

    /// Constructs a new vector from the fourth, third, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzyz(self) -> F32x4 {
        F32x4([self[3], self[2], self[1], self[2]])
    }

    /// Constructs a new vector from the first, fourth, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwyz(self) -> F32x4 {
        F32x4([self[0], self[3], self[1], self[2]])
    }

    /// Constructs a new vector from the second, fourth, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywyz(self) -> F32x4 {
        F32x4([self[1], self[3], self[1], self[2]])
    }

    /// Constructs a new vector from the third, fourth, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwyz(self) -> F32x4 {
        F32x4([self[2], self[3], self[1], self[2]])
    }

    /// Constructs a new vector from the fourth, fourth, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwyz(self) -> F32x4 {
        F32x4([self[3], self[3], self[1], self[2]])
    }

    /// Constructs a new vector from the first, first, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxzz(self) -> F32x4 {
        F32x4([self[0], self[0], self[2], self[2]])
    }

    /// Constructs a new vector from the second, first, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxzz(self) -> F32x4 {
        F32x4([self[1], self[0], self[2], self[2]])
    }

    /// Constructs a new vector from the third, first, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxzz(self) -> F32x4 {
        F32x4([self[2], self[0], self[2], self[2]])
    }

    /// Constructs a new vector from the fourth, first, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxzz(self) -> F32x4 {
        F32x4([self[3], self[0], self[2], self[2]])
    }

    /// Constructs a new vector from the first, second, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyzz(self) -> F32x4 {
        F32x4([self[0], self[1], self[2], self[2]])
    }

    /// Constructs a new vector from the second, second, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyzz(self) -> F32x4 {
        F32x4([self[1], self[1], self[2], self[2]])
    }

    /// Constructs a new vector from the third, second, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyzz(self) -> F32x4 {
        F32x4([self[2], self[1], self[2], self[2]])
    }

    /// Constructs a new vector from the fourth, second, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyzz(self) -> F32x4 {
        F32x4([self[3], self[1], self[2], self[2]])
    }

    /// Constructs a new vector from the first, third, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzzz(self) -> F32x4 {
        F32x4([self[0], self[2], self[2], self[2]])
    }

    /// Constructs a new vector from the second, third, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzzz(self) -> F32x4 {
        F32x4([self[1], self[2], self[2], self[2]])
    }

    /// Constructs a new vector from the third, third, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzzz(self) -> F32x4 {
        F32x4([self[2], self[2], self[2], self[2]])
    }

    /// Constructs a new vector from the fourth, third, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzzz(self) -> F32x4 {
        F32x4([self[3], self[2], self[2], self[2]])
    }

    /// Constructs a new vector from the first, fourth, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwzz(self) -> F32x4 {
        F32x4([self[0], self[3], self[2], self[2]])
    }

    /// Constructs a new vector from the second, fourth, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywzz(self) -> F32x4 {
        F32x4([self[1], self[3], self[2], self[2]])
    }

    /// Constructs a new vector from the third, fourth, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwzz(self) -> F32x4 {
        F32x4([self[2], self[3], self[2], self[2]])
    }

    /// Constructs a new vector from the fourth, fourth, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwzz(self) -> F32x4 {
        F32x4([self[3], self[3], self[2], self[2]])
    }

    /// Constructs a new vector from the first, first, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxwz(self) -> F32x4 {
        F32x4([self[0], self[0], self[3], self[2]])
    }

    /// Constructs a new vector from the second, first, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxwz(self) -> F32x4 {
        F32x4([self[1], self[0], self[3], self[2]])
    }

    /// Constructs a new vector from the third, first, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxwz(self) -> F32x4 {
        F32x4([self[2], self[0], self[3], self[2]])
    }

    /// Constructs a new vector from the fourth, first, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxwz(self) -> F32x4 {
        F32x4([self[3], self[0], self[3], self[2]])
    }

    /// Constructs a new vector from the first, second, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xywz(self) -> F32x4 {
        F32x4([self[0], self[1], self[3], self[2]])
    }

    /// Constructs a new vector from the second, second, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yywz(self) -> F32x4 {
        F32x4([self[1], self[1], self[3], self[2]])
    }

    /// Constructs a new vector from the third, second, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zywz(self) -> F32x4 {
        F32x4([self[2], self[1], self[3], self[2]])
    }

    /// Constructs a new vector from the fourth, second, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wywz(self) -> F32x4 {
        F32x4([self[3], self[1], self[3], self[2]])
    }

    /// Constructs a new vector from the first, third, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzwz(self) -> F32x4 {
        F32x4([self[0], self[2], self[3], self[2]])
    }

    /// Constructs a new vector from the second, third, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzwz(self) -> F32x4 {
        F32x4([self[1], self[2], self[3], self[2]])
    }

    /// Constructs a new vector from the third, third, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzwz(self) -> F32x4 {
        F32x4([self[2], self[2], self[3], self[2]])
    }

    /// Constructs a new vector from the fourth, third, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzwz(self) -> F32x4 {
        F32x4([self[3], self[2], self[3], self[2]])
    }

    /// Constructs a new vector from the first, fourth, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwwz(self) -> F32x4 {
        F32x4([self[0], self[3], self[3], self[2]])
    }

    /// Constructs a new vector from the second, fourth, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywwz(self) -> F32x4 {
        F32x4([self[1], self[3], self[3], self[2]])
    }

    /// Constructs a new vector from the third, fourth, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwwz(self) -> F32x4 {
        F32x4([self[2], self[3], self[3], self[2]])
    }

    /// Constructs a new vector from the fourth, fourth, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwwz(self) -> F32x4 {
        F32x4([self[3], self[3], self[3], self[2]])
    }

    /// Constructs a new vector from the first, first, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxxw(self) -> F32x4 {
        F32x4([self[0], self[0], self[0], self[3]])
    }

    /// Constructs a new vector from the second, first, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxxw(self) -> F32x4 {
        F32x4([self[1], self[0], self[0], self[3]])
    }

    /// Constructs a new vector from the third, first, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxxw(self) -> F32x4 {
        F32x4([self[2], self[0], self[0], self[3]])
    }

    /// Constructs a new vector from the fourth, first, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxxw(self) -> F32x4 {
        F32x4([self[3], self[0], self[0], self[3]])
    }

    /// Constructs a new vector from the first, second, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyxw(self) -> F32x4 {
        F32x4([self[0], self[1], self[0], self[3]])
    }

    /// Constructs a new vector from the second, second, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyxw(self) -> F32x4 {
        F32x4([self[1], self[1], self[0], self[3]])
    }

    /// Constructs a new vector from the third, second, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyxw(self) -> F32x4 {
        F32x4([self[2], self[1], self[0], self[3]])
    }

    /// Constructs a new vector from the fourth, second, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyxw(self) -> F32x4 {
        F32x4([self[3], self[1], self[0], self[3]])
    }

    /// Constructs a new vector from the first, third, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzxw(self) -> F32x4 {
        F32x4([self[0], self[2], self[0], self[3]])
    }

    /// Constructs a new vector from the second, third, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzxw(self) -> F32x4 {
        F32x4([self[1], self[2], self[0], self[3]])
    }

    /// Constructs a new vector from the third, third, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzxw(self) -> F32x4 {
        F32x4([self[2], self[2], self[0], self[3]])
    }

    /// Constructs a new vector from the fourth, third, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzxw(self) -> F32x4 {
        F32x4([self[3], self[2], self[0], self[3]])
    }

    /// Constructs a new vector from the first, fourth, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwxw(self) -> F32x4 {
        F32x4([self[0], self[3], self[0], self[3]])
    }

    /// Constructs a new vector from the second, fourth, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywxw(self) -> F32x4 {
        F32x4([self[1], self[3], self[0], self[3]])
    }

    /// Constructs a new vector from the third, fourth, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwxw(self) -> F32x4 {
        F32x4([self[2], self[3], self[0], self[3]])
    }

    /// Constructs a new vector from the fourth, fourth, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwxw(self) -> F32x4 {
        F32x4([self[3], self[3], self[0], self[3]])
    }

    /// Constructs a new vector from the first, first, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxyw(self) -> F32x4 {
        F32x4([self[0], self[0], self[1], self[3]])
    }

    /// Constructs a new vector from the second, first, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxyw(self) -> F32x4 {
        F32x4([self[1], self[0], self[1], self[3]])
    }

    /// Constructs a new vector from the third, first, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxyw(self) -> F32x4 {
        F32x4([self[2], self[0], self[1], self[3]])
    }

    /// Constructs a new vector from the fourth, first, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxyw(self) -> F32x4 {
        F32x4([self[3], self[0], self[1], self[3]])
    }

    /// Constructs a new vector from the first, second, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyyw(self) -> F32x4 {
        F32x4([self[0], self[1], self[1], self[3]])
    }

    /// Constructs a new vector from the second, second, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyyw(self) -> F32x4 {
        F32x4([self[1], self[1], self[1], self[3]])
    }

    /// Constructs a new vector from the third, second, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyyw(self) -> F32x4 {
        F32x4([self[2], self[1], self[1], self[3]])
    }

    /// Constructs a new vector from the fourth, second, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyyw(self) -> F32x4 {
        F32x4([self[3], self[1], self[1], self[3]])
    }

    /// Constructs a new vector from the first, third, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzyw(self) -> F32x4 {
        F32x4([self[0], self[2], self[1], self[3]])
    }

    /// Constructs a new vector from the second, third, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzyw(self) -> F32x4 {
        F32x4([self[1], self[2], self[1], self[3]])
    }

    /// Constructs a new vector from the third, third, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzyw(self) -> F32x4 {
        F32x4([self[2], self[2], self[1], self[3]])
    }

    /// Constructs a new vector from the fourth, third, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzyw(self) -> F32x4 {
        F32x4([self[3], self[2], self[1], self[3]])
    }

    /// Constructs a new vector from the first, fourth, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwyw(self) -> F32x4 {
        F32x4([self[0], self[3], self[1], self[3]])
    }

    /// Constructs a new vector from the second, fourth, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywyw(self) -> F32x4 {
        F32x4([self[1], self[3], self[1], self[3]])
    }

    /// Constructs a new vector from the third, fourth, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwyw(self) -> F32x4 {
        F32x4([self[2], self[3], self[1], self[3]])
    }

    /// Constructs a new vector from the fourth, fourth, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwyw(self) -> F32x4 {
        F32x4([self[3], self[3], self[1], self[3]])
    }

    /// Constructs a new vector from the first, first, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxzw(self) -> F32x4 {
        F32x4([self[0], self[0], self[2], self[3]])
    }

    /// Constructs a new vector from the second, first, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxzw(self) -> F32x4 {
        F32x4([self[1], self[0], self[2], self[3]])
    }

    /// Constructs a new vector from the third, first, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxzw(self) -> F32x4 {
        F32x4([self[2], self[0], self[2], self[3]])
    }

    /// Constructs a new vector from the fourth, first, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxzw(self) -> F32x4 {
        F32x4([self[3], self[0], self[2], self[3]])
    }

    /// Constructs a new vector from the first, second, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyzw(self) -> F32x4 {
        F32x4([self[0], self[1], self[2], self[3]])
    }

    /// Constructs a new vector from the second, second, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyzw(self) -> F32x4 {
        F32x4([self[1], self[1], self[2], self[3]])
    }

    /// Constructs a new vector from the third, second, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyzw(self) -> F32x4 {
        F32x4([self[2], self[1], self[2], self[3]])
    }

    /// Constructs a new vector from the fourth, second, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyzw(self) -> F32x4 {
        F32x4([self[3], self[1], self[2], self[3]])
    }

    /// Constructs a new vector from the first, third, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzzw(self) -> F32x4 {
        F32x4([self[0], self[2], self[2], self[3]])
    }

    /// Constructs a new vector from the second, third, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzzw(self) -> F32x4 {
        F32x4([self[1], self[2], self[2], self[3]])
    }

    /// Constructs a new vector from the third, third, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzzw(self) -> F32x4 {
        F32x4([self[2], self[2], self[2], self[3]])
    }

    /// Constructs a new vector from the fourth, third, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzzw(self) -> F32x4 {
        F32x4([self[3], self[2], self[2], self[3]])
    }

    /// Constructs a new vector from the first, fourth, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwzw(self) -> F32x4 {
        F32x4([self[0], self[3], self[2], self[3]])
    }

    /// Constructs a new vector from the second, fourth, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywzw(self) -> F32x4 {
        F32x4([self[1], self[3], self[2], self[3]])
    }

    /// Constructs a new vector from the third, fourth, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwzw(self) -> F32x4 {
        F32x4([self[2], self[3], self[2], self[3]])
    }

    /// Constructs a new vector from the fourth, fourth, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwzw(self) -> F32x4 {
        F32x4([self[3], self[3], self[2], self[3]])
    }

    /// Constructs a new vector from the first, first, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxww(self) -> F32x4 {
        F32x4([self[0], self[0], self[3], self[3]])
    }

    /// Constructs a new vector from the second, first, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxww(self) -> F32x4 {
        F32x4([self[1], self[0], self[3], self[3]])
    }

    /// Constructs a new vector from the third, first, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxww(self) -> F32x4 {
        F32x4([self[2], self[0], self[3], self[3]])
    }

    /// Constructs a new vector from the fourth, first, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxww(self) -> F32x4 {
        F32x4([self[3], self[0], self[3], self[3]])
    }

    /// Constructs a new vector from the first, second, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyww(self) -> F32x4 {
        F32x4([self[0], self[1], self[3], self[3]])
    }

    /// Constructs a new vector from the second, second, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyww(self) -> F32x4 {
        F32x4([self[1], self[1], self[3], self[3]])
    }

    /// Constructs a new vector from the third, second, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyww(self) -> F32x4 {
        F32x4([self[2], self[1], self[3], self[3]])
    }

    /// Constructs a new vector from the fourth, second, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyww(self) -> F32x4 {
        F32x4([self[3], self[1], self[3], self[3]])
    }

    /// Constructs a new vector from the first, third, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzww(self) -> F32x4 {
        F32x4([self[0], self[2], self[3], self[3]])
    }

    /// Constructs a new vector from the second, third, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzww(self) -> F32x4 {
        F32x4([self[1], self[2], self[3], self[3]])
    }

    /// Constructs a new vector from the third, third, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzww(self) -> F32x4 {
        F32x4([self[2], self[2], self[3], self[3]])
    }

    /// Constructs a new vector from the fourth, third, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzww(self) -> F32x4 {
        F32x4([self[3], self[2], self[3], self[3]])
    }

    /// Constructs a new vector from the first, fourth, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwww(self) -> F32x4 {
        F32x4([self[0], self[3], self[3], self[3]])
    }

    /// Constructs a new vector from the second, fourth, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywww(self) -> F32x4 {
        F32x4([self[1], self[3], self[3], self[3]])
    }

    /// Constructs a new vector from the third, fourth, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwww(self) -> F32x4 {
        F32x4([self[2], self[3], self[3], self[3]])
    }

    /// Constructs a new vector from the fourth, fourth, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwww(self) -> F32x4 {
        F32x4([self[3], self[3], self[3], self[3]])
    }

    // Concatenations

    #[inline]
    pub fn concat_xy_xy(self, other: F32x4) -> F32x4 {
        F32x4([self[0], self[1], other[0], other[1]])
    }

    #[inline]
    pub fn concat_xy_zw(self, other: F32x4) -> F32x4 {
        F32x4([self[0], self[1], other[2], other[3]])
    }

    #[inline]
    pub fn concat_zw_zw(self, other: F32x4) -> F32x4 {
        F32x4([self[2], self[3], other[2], other[3]])
    }

    #[inline]
    pub fn concat_wz_yx(self, other: F32x4) -> F32x4 {
        F32x4([self[3], self[2], other[1], other[0]])
    }

    #[inline]
    pub fn cross(&self, other: F32x4) -> F32x4 {
        unimplemented!()
    }
}

impl Index<usize> for F32x4 {
    type Output = f32;
    #[inline]
    fn index(&self, index: usize) -> &f32 {
        &self.0[index]
    }
}

impl IndexMut<usize> for F32x4 {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut f32 {
        &mut self.0[index]
    }
}

impl Debug for F32x4 {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "<{}, {}, {}, {}>", self[0], self[1], self[2], self[3])
    }
}

impl Add<F32x4> for F32x4 {
    type Output = F32x4;
    #[inline]
    fn add(self, other: F32x4) -> F32x4 {
        F32x4([self[0] + other[0], self[1] + other[1], self[2] + other[2], self[3] + other[3]])
    }
}

impl Mul<F32x4> for F32x4 {
    type Output = F32x4;
    #[inline]
    fn mul(self, other: F32x4) -> F32x4 {
        F32x4([self[0] * other[0], self[1] * other[1], self[2] * other[2], self[3] * other[3]])
    }
}

impl Sub<F32x4> for F32x4 {
    type Output = F32x4;
    #[inline]
    fn sub(self, other: F32x4) -> F32x4 {
        F32x4([self[0] - other[0], self[1] - other[1], self[2] - other[2], self[3] - other[3]])
    }
}

// 32-bit signed integers

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct I32x4([i32; 4]);

impl I32x4 {
    #[inline]
    pub fn new(a: i32, b: i32, c: i32, d: i32) -> I32x4 {
        I32x4([a, b, c, d])
    }

    #[inline]
    pub fn splat(x: i32) -> I32x4 {
        I32x4([x; 4])
    }

    #[inline]
    pub fn as_u8x16(self) -> U8x16 {
        unsafe {
            U8x16(*mem::transmute::<&[i32; 4], &[u8; 16]>(&self.0))
        }
    }

    #[inline]
    pub fn min(self, other: I32x4) -> I32x4 {
        I32x4([
            self[0].min(other[0]),
            self[1].min(other[1]),
            self[2].min(other[2]),
            self[3].min(other[3]),
        ])
    }

    // Packed comparisons

    #[inline]
    pub fn packed_eq(self, other: I32x4) -> U32x4 {
        U32x4([
            if self[0] == other[0] { !0 } else { 0 },
            if self[1] == other[1] { !0 } else { 0 },
            if self[2] == other[2] { !0 } else { 0 },
            if self[3] == other[3] { !0 } else { 0 },
        ])
    }

    #[inline]
    pub fn packed_le(self, other: I32x4) -> U32x4 {
        U32x4([
            if self[0] <= other[0] { !0 } else { 0 },
            if self[1] <= other[1] { !0 } else { 0 },
            if self[2] <= other[2] { !0 } else { 0 },
            if self[3] <= other[3] { !0 } else { 0 },
        ])
    }

    // Shuffles

    /// Constructs a new vector from the first, first, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxxx(self) -> I32x4 {
        I32x4([self[0], self[0], self[0], self[0]])
    }

    /// Constructs a new vector from the second, first, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxxx(self) -> I32x4 {
        I32x4([self[1], self[0], self[0], self[0]])
    }

    /// Constructs a new vector from the third, first, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxxx(self) -> I32x4 {
        I32x4([self[2], self[0], self[0], self[0]])
    }

    /// Constructs a new vector from the fourth, first, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxxx(self) -> I32x4 {
        I32x4([self[3], self[0], self[0], self[0]])
    }

    /// Constructs a new vector from the first, second, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyxx(self) -> I32x4 {
        I32x4([self[0], self[1], self[0], self[0]])
    }

    /// Constructs a new vector from the second, second, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyxx(self) -> I32x4 {
        I32x4([self[1], self[1], self[0], self[0]])
    }

    /// Constructs a new vector from the third, second, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyxx(self) -> I32x4 {
        I32x4([self[2], self[1], self[0], self[0]])
    }

    /// Constructs a new vector from the fourth, second, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyxx(self) -> I32x4 {
        I32x4([self[3], self[1], self[0], self[0]])
    }

    /// Constructs a new vector from the first, third, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzxx(self) -> I32x4 {
        I32x4([self[0], self[2], self[0], self[0]])
    }

    /// Constructs a new vector from the second, third, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzxx(self) -> I32x4 {
        I32x4([self[1], self[2], self[0], self[0]])
    }

    /// Constructs a new vector from the third, third, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzxx(self) -> I32x4 {
        I32x4([self[2], self[2], self[0], self[0]])
    }

    /// Constructs a new vector from the fourth, third, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzxx(self) -> I32x4 {
        I32x4([self[3], self[2], self[0], self[0]])
    }

    /// Constructs a new vector from the first, fourth, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwxx(self) -> I32x4 {
        I32x4([self[0], self[3], self[0], self[0]])
    }

    /// Constructs a new vector from the second, fourth, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywxx(self) -> I32x4 {
        I32x4([self[1], self[3], self[0], self[0]])
    }

    /// Constructs a new vector from the third, fourth, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwxx(self) -> I32x4 {
        I32x4([self[2], self[3], self[0], self[0]])
    }

    /// Constructs a new vector from the fourth, fourth, first, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwxx(self) -> I32x4 {
        I32x4([self[3], self[3], self[0], self[0]])
    }

    /// Constructs a new vector from the first, first, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxyx(self) -> I32x4 {
        I32x4([self[0], self[0], self[1], self[0]])
    }

    /// Constructs a new vector from the second, first, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxyx(self) -> I32x4 {
        I32x4([self[1], self[0], self[1], self[0]])
    }

    /// Constructs a new vector from the third, first, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxyx(self) -> I32x4 {
        I32x4([self[2], self[0], self[1], self[0]])
    }

    /// Constructs a new vector from the fourth, first, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxyx(self) -> I32x4 {
        I32x4([self[3], self[0], self[1], self[0]])
    }

    /// Constructs a new vector from the first, second, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyyx(self) -> I32x4 {
        I32x4([self[0], self[1], self[1], self[0]])
    }

    /// Constructs a new vector from the second, second, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyyx(self) -> I32x4 {
        I32x4([self[1], self[1], self[1], self[0]])
    }

    /// Constructs a new vector from the third, second, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyyx(self) -> I32x4 {
        I32x4([self[2], self[1], self[1], self[0]])
    }

    /// Constructs a new vector from the fourth, second, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyyx(self) -> I32x4 {
        I32x4([self[3], self[1], self[1], self[0]])
    }

    /// Constructs a new vector from the first, third, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzyx(self) -> I32x4 {
        I32x4([self[0], self[2], self[1], self[0]])
    }

    /// Constructs a new vector from the second, third, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzyx(self) -> I32x4 {
        I32x4([self[1], self[2], self[1], self[0]])
    }

    /// Constructs a new vector from the third, third, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzyx(self) -> I32x4 {
        I32x4([self[2], self[2], self[1], self[0]])
    }

    /// Constructs a new vector from the fourth, third, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzyx(self) -> I32x4 {
        I32x4([self[3], self[2], self[1], self[0]])
    }

    /// Constructs a new vector from the first, fourth, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwyx(self) -> I32x4 {
        I32x4([self[0], self[3], self[1], self[0]])
    }

    /// Constructs a new vector from the second, fourth, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywyx(self) -> I32x4 {
        I32x4([self[1], self[3], self[1], self[0]])
    }

    /// Constructs a new vector from the third, fourth, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwyx(self) -> I32x4 {
        I32x4([self[2], self[3], self[1], self[0]])
    }

    /// Constructs a new vector from the fourth, fourth, second, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwyx(self) -> I32x4 {
        I32x4([self[3], self[3], self[1], self[0]])
    }

    /// Constructs a new vector from the first, first, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxzx(self) -> I32x4 {
        I32x4([self[0], self[0], self[2], self[0]])
    }

    /// Constructs a new vector from the second, first, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxzx(self) -> I32x4 {
        I32x4([self[1], self[0], self[2], self[0]])
    }

    /// Constructs a new vector from the third, first, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxzx(self) -> I32x4 {
        I32x4([self[2], self[0], self[2], self[0]])
    }

    /// Constructs a new vector from the fourth, first, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxzx(self) -> I32x4 {
        I32x4([self[3], self[0], self[2], self[0]])
    }

    /// Constructs a new vector from the first, second, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyzx(self) -> I32x4 {
        I32x4([self[0], self[1], self[2], self[0]])
    }

    /// Constructs a new vector from the second, second, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyzx(self) -> I32x4 {
        I32x4([self[1], self[1], self[2], self[0]])
    }

    /// Constructs a new vector from the third, second, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyzx(self) -> I32x4 {
        I32x4([self[2], self[1], self[2], self[0]])
    }

    /// Constructs a new vector from the fourth, second, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyzx(self) -> I32x4 {
        I32x4([self[3], self[1], self[2], self[0]])
    }

    /// Constructs a new vector from the first, third, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzzx(self) -> I32x4 {
        I32x4([self[0], self[2], self[2], self[0]])
    }

    /// Constructs a new vector from the second, third, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzzx(self) -> I32x4 {
        I32x4([self[1], self[2], self[2], self[0]])
    }

    /// Constructs a new vector from the third, third, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzzx(self) -> I32x4 {
        I32x4([self[2], self[2], self[2], self[0]])
    }

    /// Constructs a new vector from the fourth, third, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzzx(self) -> I32x4 {
        I32x4([self[3], self[2], self[2], self[0]])
    }

    /// Constructs a new vector from the first, fourth, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwzx(self) -> I32x4 {
        I32x4([self[0], self[3], self[2], self[0]])
    }

    /// Constructs a new vector from the second, fourth, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywzx(self) -> I32x4 {
        I32x4([self[1], self[3], self[2], self[0]])
    }

    /// Constructs a new vector from the third, fourth, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwzx(self) -> I32x4 {
        I32x4([self[2], self[3], self[2], self[0]])
    }

    /// Constructs a new vector from the fourth, fourth, third, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwzx(self) -> I32x4 {
        I32x4([self[3], self[3], self[2], self[0]])
    }

    /// Constructs a new vector from the first, first, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxwx(self) -> I32x4 {
        I32x4([self[0], self[0], self[3], self[0]])
    }

    /// Constructs a new vector from the second, first, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxwx(self) -> I32x4 {
        I32x4([self[1], self[0], self[3], self[0]])
    }

    /// Constructs a new vector from the third, first, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxwx(self) -> I32x4 {
        I32x4([self[2], self[0], self[3], self[0]])
    }

    /// Constructs a new vector from the fourth, first, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxwx(self) -> I32x4 {
        I32x4([self[3], self[0], self[3], self[0]])
    }

    /// Constructs a new vector from the first, second, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xywx(self) -> I32x4 {
        I32x4([self[0], self[1], self[3], self[0]])
    }

    /// Constructs a new vector from the second, second, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yywx(self) -> I32x4 {
        I32x4([self[1], self[1], self[3], self[0]])
    }

    /// Constructs a new vector from the third, second, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zywx(self) -> I32x4 {
        I32x4([self[2], self[1], self[3], self[0]])
    }

    /// Constructs a new vector from the fourth, second, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wywx(self) -> I32x4 {
        I32x4([self[3], self[1], self[3], self[0]])
    }

    /// Constructs a new vector from the first, third, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzwx(self) -> I32x4 {
        I32x4([self[0], self[2], self[3], self[0]])
    }

    /// Constructs a new vector from the second, third, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzwx(self) -> I32x4 {
        I32x4([self[1], self[2], self[3], self[0]])
    }

    /// Constructs a new vector from the third, third, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzwx(self) -> I32x4 {
        I32x4([self[2], self[2], self[3], self[0]])
    }

    /// Constructs a new vector from the fourth, third, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzwx(self) -> I32x4 {
        I32x4([self[3], self[2], self[3], self[0]])
    }

    /// Constructs a new vector from the first, fourth, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwwx(self) -> I32x4 {
        I32x4([self[0], self[3], self[3], self[0]])
    }

    /// Constructs a new vector from the second, fourth, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywwx(self) -> I32x4 {
        I32x4([self[1], self[3], self[3], self[0]])
    }

    /// Constructs a new vector from the third, fourth, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwwx(self) -> I32x4 {
        I32x4([self[2], self[3], self[3], self[0]])
    }

    /// Constructs a new vector from the fourth, fourth, fourth, and first
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwwx(self) -> I32x4 {
        I32x4([self[3], self[3], self[3], self[0]])
    }

    /// Constructs a new vector from the first, first, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxxy(self) -> I32x4 {
        I32x4([self[0], self[0], self[0], self[1]])
    }

    /// Constructs a new vector from the second, first, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxxy(self) -> I32x4 {
        I32x4([self[1], self[0], self[0], self[1]])
    }

    /// Constructs a new vector from the third, first, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxxy(self) -> I32x4 {
        I32x4([self[2], self[0], self[0], self[1]])
    }

    /// Constructs a new vector from the fourth, first, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxxy(self) -> I32x4 {
        I32x4([self[3], self[0], self[0], self[1]])
    }

    /// Constructs a new vector from the first, second, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyxy(self) -> I32x4 {
        I32x4([self[0], self[1], self[0], self[1]])
    }

    /// Constructs a new vector from the second, second, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyxy(self) -> I32x4 {
        I32x4([self[1], self[1], self[0], self[1]])
    }

    /// Constructs a new vector from the third, second, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyxy(self) -> I32x4 {
        I32x4([self[2], self[1], self[0], self[1]])
    }

    /// Constructs a new vector from the fourth, second, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyxy(self) -> I32x4 {
        I32x4([self[3], self[1], self[0], self[1]])
    }

    /// Constructs a new vector from the first, third, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzxy(self) -> I32x4 {
        I32x4([self[0], self[2], self[0], self[1]])
    }

    /// Constructs a new vector from the second, third, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzxy(self) -> I32x4 {
        I32x4([self[1], self[2], self[0], self[1]])
    }

    /// Constructs a new vector from the third, third, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzxy(self) -> I32x4 {
        I32x4([self[2], self[2], self[0], self[1]])
    }

    /// Constructs a new vector from the fourth, third, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzxy(self) -> I32x4 {
        I32x4([self[3], self[2], self[0], self[1]])
    }

    /// Constructs a new vector from the first, fourth, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwxy(self) -> I32x4 {
        I32x4([self[0], self[3], self[0], self[1]])
    }

    /// Constructs a new vector from the second, fourth, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywxy(self) -> I32x4 {
        I32x4([self[1], self[3], self[0], self[1]])
    }

    /// Constructs a new vector from the third, fourth, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwxy(self) -> I32x4 {
        I32x4([self[2], self[3], self[0], self[1]])
    }

    /// Constructs a new vector from the fourth, fourth, first, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwxy(self) -> I32x4 {
        I32x4([self[3], self[3], self[0], self[1]])
    }

    /// Constructs a new vector from the first, first, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxyy(self) -> I32x4 {
        I32x4([self[0], self[0], self[1], self[1]])
    }

    /// Constructs a new vector from the second, first, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxyy(self) -> I32x4 {
        I32x4([self[1], self[0], self[1], self[1]])
    }

    /// Constructs a new vector from the third, first, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxyy(self) -> I32x4 {
        I32x4([self[2], self[0], self[1], self[1]])
    }

    /// Constructs a new vector from the fourth, first, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxyy(self) -> I32x4 {
        I32x4([self[3], self[0], self[1], self[1]])
    }

    /// Constructs a new vector from the first, second, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyyy(self) -> I32x4 {
        I32x4([self[0], self[1], self[1], self[1]])
    }

    /// Constructs a new vector from the second, second, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyyy(self) -> I32x4 {
        I32x4([self[1], self[1], self[1], self[1]])
    }

    /// Constructs a new vector from the third, second, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyyy(self) -> I32x4 {
        I32x4([self[2], self[1], self[1], self[1]])
    }

    /// Constructs a new vector from the fourth, second, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyyy(self) -> I32x4 {
        I32x4([self[3], self[1], self[1], self[1]])
    }

    /// Constructs a new vector from the first, third, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzyy(self) -> I32x4 {
        I32x4([self[0], self[2], self[1], self[1]])
    }

    /// Constructs a new vector from the second, third, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzyy(self) -> I32x4 {
        I32x4([self[1], self[2], self[1], self[1]])
    }

    /// Constructs a new vector from the third, third, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzyy(self) -> I32x4 {
        I32x4([self[2], self[2], self[1], self[1]])
    }

    /// Constructs a new vector from the fourth, third, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzyy(self) -> I32x4 {
        I32x4([self[3], self[2], self[1], self[1]])
    }

    /// Constructs a new vector from the first, fourth, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwyy(self) -> I32x4 {
        I32x4([self[0], self[3], self[1], self[1]])
    }

    /// Constructs a new vector from the second, fourth, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywyy(self) -> I32x4 {
        I32x4([self[1], self[3], self[1], self[1]])
    }

    /// Constructs a new vector from the third, fourth, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwyy(self) -> I32x4 {
        I32x4([self[2], self[3], self[1], self[1]])
    }

    /// Constructs a new vector from the fourth, fourth, second, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwyy(self) -> I32x4 {
        I32x4([self[3], self[3], self[1], self[1]])
    }

    /// Constructs a new vector from the first, first, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxzy(self) -> I32x4 {
        I32x4([self[0], self[0], self[2], self[1]])
    }

    /// Constructs a new vector from the second, first, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxzy(self) -> I32x4 {
        I32x4([self[1], self[0], self[2], self[1]])
    }

    /// Constructs a new vector from the third, first, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxzy(self) -> I32x4 {
        I32x4([self[2], self[0], self[2], self[1]])
    }

    /// Constructs a new vector from the fourth, first, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxzy(self) -> I32x4 {
        I32x4([self[3], self[0], self[2], self[1]])
    }

    /// Constructs a new vector from the first, second, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyzy(self) -> I32x4 {
        I32x4([self[0], self[1], self[2], self[1]])
    }

    /// Constructs a new vector from the second, second, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyzy(self) -> I32x4 {
        I32x4([self[1], self[1], self[2], self[1]])
    }

    /// Constructs a new vector from the third, second, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyzy(self) -> I32x4 {
        I32x4([self[2], self[1], self[2], self[1]])
    }

    /// Constructs a new vector from the fourth, second, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyzy(self) -> I32x4 {
        I32x4([self[3], self[1], self[2], self[1]])
    }

    /// Constructs a new vector from the first, third, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzzy(self) -> I32x4 {
        I32x4([self[0], self[2], self[2], self[1]])
    }

    /// Constructs a new vector from the second, third, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzzy(self) -> I32x4 {
        I32x4([self[1], self[2], self[2], self[1]])
    }

    /// Constructs a new vector from the third, third, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzzy(self) -> I32x4 {
        I32x4([self[2], self[2], self[2], self[1]])
    }

    /// Constructs a new vector from the fourth, third, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzzy(self) -> I32x4 {
        I32x4([self[3], self[2], self[2], self[1]])
    }

    /// Constructs a new vector from the first, fourth, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwzy(self) -> I32x4 {
        I32x4([self[0], self[3], self[2], self[1]])
    }

    /// Constructs a new vector from the second, fourth, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywzy(self) -> I32x4 {
        I32x4([self[1], self[3], self[2], self[1]])
    }

    /// Constructs a new vector from the third, fourth, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwzy(self) -> I32x4 {
        I32x4([self[2], self[3], self[2], self[1]])
    }

    /// Constructs a new vector from the fourth, fourth, third, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwzy(self) -> I32x4 {
        I32x4([self[3], self[3], self[2], self[1]])
    }

    /// Constructs a new vector from the first, first, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxwy(self) -> I32x4 {
        I32x4([self[0], self[0], self[3], self[1]])
    }

    /// Constructs a new vector from the second, first, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxwy(self) -> I32x4 {
        I32x4([self[1], self[0], self[3], self[1]])
    }

    /// Constructs a new vector from the third, first, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxwy(self) -> I32x4 {
        I32x4([self[2], self[0], self[3], self[1]])
    }

    /// Constructs a new vector from the fourth, first, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxwy(self) -> I32x4 {
        I32x4([self[3], self[0], self[3], self[1]])
    }

    /// Constructs a new vector from the first, second, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xywy(self) -> I32x4 {
        I32x4([self[0], self[1], self[3], self[1]])
    }

    /// Constructs a new vector from the second, second, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yywy(self) -> I32x4 {
        I32x4([self[1], self[1], self[3], self[1]])
    }

    /// Constructs a new vector from the third, second, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zywy(self) -> I32x4 {
        I32x4([self[2], self[1], self[3], self[1]])
    }

    /// Constructs a new vector from the fourth, second, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wywy(self) -> I32x4 {
        I32x4([self[3], self[1], self[3], self[1]])
    }

    /// Constructs a new vector from the first, third, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzwy(self) -> I32x4 {
        I32x4([self[0], self[2], self[3], self[1]])
    }

    /// Constructs a new vector from the second, third, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzwy(self) -> I32x4 {
        I32x4([self[1], self[2], self[3], self[1]])
    }

    /// Constructs a new vector from the third, third, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzwy(self) -> I32x4 {
        I32x4([self[2], self[2], self[3], self[1]])
    }

    /// Constructs a new vector from the fourth, third, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzwy(self) -> I32x4 {
        I32x4([self[3], self[2], self[3], self[1]])
    }

    /// Constructs a new vector from the first, fourth, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwwy(self) -> I32x4 {
        I32x4([self[0], self[3], self[3], self[1]])
    }

    /// Constructs a new vector from the second, fourth, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywwy(self) -> I32x4 {
        I32x4([self[1], self[3], self[3], self[1]])
    }

    /// Constructs a new vector from the third, fourth, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwwy(self) -> I32x4 {
        I32x4([self[2], self[3], self[3], self[1]])
    }

    /// Constructs a new vector from the fourth, fourth, fourth, and second
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwwy(self) -> I32x4 {
        I32x4([self[3], self[3], self[3], self[1]])
    }

    /// Constructs a new vector from the first, first, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxxz(self) -> I32x4 {
        I32x4([self[0], self[0], self[0], self[2]])
    }

    /// Constructs a new vector from the second, first, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxxz(self) -> I32x4 {
        I32x4([self[1], self[0], self[0], self[2]])
    }

    /// Constructs a new vector from the third, first, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxxz(self) -> I32x4 {
        I32x4([self[2], self[0], self[0], self[2]])
    }

    /// Constructs a new vector from the fourth, first, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxxz(self) -> I32x4 {
        I32x4([self[3], self[0], self[0], self[2]])
    }

    /// Constructs a new vector from the first, second, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyxz(self) -> I32x4 {
        I32x4([self[0], self[1], self[0], self[2]])
    }

    /// Constructs a new vector from the second, second, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyxz(self) -> I32x4 {
        I32x4([self[1], self[1], self[0], self[2]])
    }

    /// Constructs a new vector from the third, second, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyxz(self) -> I32x4 {
        I32x4([self[2], self[1], self[0], self[2]])
    }

    /// Constructs a new vector from the fourth, second, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyxz(self) -> I32x4 {
        I32x4([self[3], self[1], self[0], self[2]])
    }

    /// Constructs a new vector from the first, third, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzxz(self) -> I32x4 {
        I32x4([self[0], self[2], self[0], self[2]])
    }

    /// Constructs a new vector from the second, third, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzxz(self) -> I32x4 {
        I32x4([self[1], self[2], self[0], self[2]])
    }

    /// Constructs a new vector from the third, third, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzxz(self) -> I32x4 {
        I32x4([self[2], self[2], self[0], self[2]])
    }

    /// Constructs a new vector from the fourth, third, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzxz(self) -> I32x4 {
        I32x4([self[3], self[2], self[0], self[2]])
    }

    /// Constructs a new vector from the first, fourth, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwxz(self) -> I32x4 {
        I32x4([self[0], self[3], self[0], self[2]])
    }

    /// Constructs a new vector from the second, fourth, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywxz(self) -> I32x4 {
        I32x4([self[1], self[3], self[0], self[2]])
    }

    /// Constructs a new vector from the third, fourth, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwxz(self) -> I32x4 {
        I32x4([self[2], self[3], self[0], self[2]])
    }

    /// Constructs a new vector from the fourth, fourth, first, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwxz(self) -> I32x4 {
        I32x4([self[3], self[3], self[0], self[2]])
    }

    /// Constructs a new vector from the first, first, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxyz(self) -> I32x4 {
        I32x4([self[0], self[0], self[1], self[2]])
    }

    /// Constructs a new vector from the second, first, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxyz(self) -> I32x4 {
        I32x4([self[1], self[0], self[1], self[2]])
    }

    /// Constructs a new vector from the third, first, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxyz(self) -> I32x4 {
        I32x4([self[2], self[0], self[1], self[2]])
    }

    /// Constructs a new vector from the fourth, first, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxyz(self) -> I32x4 {
        I32x4([self[3], self[0], self[1], self[2]])
    }

    /// Constructs a new vector from the first, second, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyyz(self) -> I32x4 {
        I32x4([self[0], self[1], self[1], self[2]])
    }

    /// Constructs a new vector from the second, second, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyyz(self) -> I32x4 {
        I32x4([self[1], self[1], self[1], self[2]])
    }

    /// Constructs a new vector from the third, second, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyyz(self) -> I32x4 {
        I32x4([self[2], self[1], self[1], self[2]])
    }

    /// Constructs a new vector from the fourth, second, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyyz(self) -> I32x4 {
        I32x4([self[3], self[1], self[1], self[2]])
    }

    /// Constructs a new vector from the first, third, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzyz(self) -> I32x4 {
        I32x4([self[0], self[2], self[1], self[2]])
    }

    /// Constructs a new vector from the second, third, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzyz(self) -> I32x4 {
        I32x4([self[1], self[2], self[1], self[2]])
    }

    /// Constructs a new vector from the third, third, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzyz(self) -> I32x4 {
        I32x4([self[2], self[2], self[1], self[2]])
    }

    /// Constructs a new vector from the fourth, third, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzyz(self) -> I32x4 {
        I32x4([self[3], self[2], self[1], self[2]])
    }

    /// Constructs a new vector from the first, fourth, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwyz(self) -> I32x4 {
        I32x4([self[0], self[3], self[1], self[2]])
    }

    /// Constructs a new vector from the second, fourth, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywyz(self) -> I32x4 {
        I32x4([self[1], self[3], self[1], self[2]])
    }

    /// Constructs a new vector from the third, fourth, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwyz(self) -> I32x4 {
        I32x4([self[2], self[3], self[1], self[2]])
    }

    /// Constructs a new vector from the fourth, fourth, second, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwyz(self) -> I32x4 {
        I32x4([self[3], self[3], self[1], self[2]])
    }

    /// Constructs a new vector from the first, first, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxzz(self) -> I32x4 {
        I32x4([self[0], self[0], self[2], self[2]])
    }

    /// Constructs a new vector from the second, first, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxzz(self) -> I32x4 {
        I32x4([self[1], self[0], self[2], self[2]])
    }

    /// Constructs a new vector from the third, first, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxzz(self) -> I32x4 {
        I32x4([self[2], self[0], self[2], self[2]])
    }

    /// Constructs a new vector from the fourth, first, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxzz(self) -> I32x4 {
        I32x4([self[3], self[0], self[2], self[2]])
    }

    /// Constructs a new vector from the first, second, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyzz(self) -> I32x4 {
        I32x4([self[0], self[1], self[2], self[2]])
    }

    /// Constructs a new vector from the second, second, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyzz(self) -> I32x4 {
        I32x4([self[1], self[1], self[2], self[2]])
    }

    /// Constructs a new vector from the third, second, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyzz(self) -> I32x4 {
        I32x4([self[2], self[1], self[2], self[2]])
    }

    /// Constructs a new vector from the fourth, second, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyzz(self) -> I32x4 {
        I32x4([self[3], self[1], self[2], self[2]])
    }

    /// Constructs a new vector from the first, third, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzzz(self) -> I32x4 {
        I32x4([self[0], self[2], self[2], self[2]])
    }

    /// Constructs a new vector from the second, third, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzzz(self) -> I32x4 {
        I32x4([self[1], self[2], self[2], self[2]])
    }

    /// Constructs a new vector from the third, third, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzzz(self) -> I32x4 {
        I32x4([self[2], self[2], self[2], self[2]])
    }

    /// Constructs a new vector from the fourth, third, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzzz(self) -> I32x4 {
        I32x4([self[3], self[2], self[2], self[2]])
    }

    /// Constructs a new vector from the first, fourth, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwzz(self) -> I32x4 {
        I32x4([self[0], self[3], self[2], self[2]])
    }

    /// Constructs a new vector from the second, fourth, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywzz(self) -> I32x4 {
        I32x4([self[1], self[3], self[2], self[2]])
    }

    /// Constructs a new vector from the third, fourth, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwzz(self) -> I32x4 {
        I32x4([self[2], self[3], self[2], self[2]])
    }

    /// Constructs a new vector from the fourth, fourth, third, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwzz(self) -> I32x4 {
        I32x4([self[3], self[3], self[2], self[2]])
    }

    /// Constructs a new vector from the first, first, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxwz(self) -> I32x4 {
        I32x4([self[0], self[0], self[3], self[2]])
    }

    /// Constructs a new vector from the second, first, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxwz(self) -> I32x4 {
        I32x4([self[1], self[0], self[3], self[2]])
    }

    /// Constructs a new vector from the third, first, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxwz(self) -> I32x4 {
        I32x4([self[2], self[0], self[3], self[2]])
    }

    /// Constructs a new vector from the fourth, first, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxwz(self) -> I32x4 {
        I32x4([self[3], self[0], self[3], self[2]])
    }

    /// Constructs a new vector from the first, second, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xywz(self) -> I32x4 {
        I32x4([self[0], self[1], self[3], self[2]])
    }

    /// Constructs a new vector from the second, second, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yywz(self) -> I32x4 {
        I32x4([self[1], self[1], self[3], self[2]])
    }

    /// Constructs a new vector from the third, second, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zywz(self) -> I32x4 {
        I32x4([self[2], self[1], self[3], self[2]])
    }

    /// Constructs a new vector from the fourth, second, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wywz(self) -> I32x4 {
        I32x4([self[3], self[1], self[3], self[2]])
    }

    /// Constructs a new vector from the first, third, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzwz(self) -> I32x4 {
        I32x4([self[0], self[2], self[3], self[2]])
    }

    /// Constructs a new vector from the second, third, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzwz(self) -> I32x4 {
        I32x4([self[1], self[2], self[3], self[2]])
    }

    /// Constructs a new vector from the third, third, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzwz(self) -> I32x4 {
        I32x4([self[2], self[2], self[3], self[2]])
    }

    /// Constructs a new vector from the fourth, third, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzwz(self) -> I32x4 {
        I32x4([self[3], self[2], self[3], self[2]])
    }

    /// Constructs a new vector from the first, fourth, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwwz(self) -> I32x4 {
        I32x4([self[0], self[3], self[3], self[2]])
    }

    /// Constructs a new vector from the second, fourth, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywwz(self) -> I32x4 {
        I32x4([self[1], self[3], self[3], self[2]])
    }

    /// Constructs a new vector from the third, fourth, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwwz(self) -> I32x4 {
        I32x4([self[2], self[3], self[3], self[2]])
    }

    /// Constructs a new vector from the fourth, fourth, fourth, and third
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwwz(self) -> I32x4 {
        I32x4([self[3], self[3], self[3], self[2]])
    }

    /// Constructs a new vector from the first, first, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxxw(self) -> I32x4 {
        I32x4([self[0], self[0], self[0], self[3]])
    }

    /// Constructs a new vector from the second, first, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxxw(self) -> I32x4 {
        I32x4([self[1], self[0], self[0], self[3]])
    }

    /// Constructs a new vector from the third, first, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxxw(self) -> I32x4 {
        I32x4([self[2], self[0], self[0], self[3]])
    }

    /// Constructs a new vector from the fourth, first, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxxw(self) -> I32x4 {
        I32x4([self[3], self[0], self[0], self[3]])
    }

    /// Constructs a new vector from the first, second, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyxw(self) -> I32x4 {
        I32x4([self[0], self[1], self[0], self[3]])
    }

    /// Constructs a new vector from the second, second, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyxw(self) -> I32x4 {
        I32x4([self[1], self[1], self[0], self[3]])
    }

    /// Constructs a new vector from the third, second, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyxw(self) -> I32x4 {
        I32x4([self[2], self[1], self[0], self[3]])
    }

    /// Constructs a new vector from the fourth, second, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyxw(self) -> I32x4 {
        I32x4([self[3], self[1], self[0], self[3]])
    }

    /// Constructs a new vector from the first, third, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzxw(self) -> I32x4 {
        I32x4([self[0], self[2], self[0], self[3]])
    }

    /// Constructs a new vector from the second, third, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzxw(self) -> I32x4 {
        I32x4([self[1], self[2], self[0], self[3]])
    }

    /// Constructs a new vector from the third, third, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzxw(self) -> I32x4 {
        I32x4([self[2], self[2], self[0], self[3]])
    }

    /// Constructs a new vector from the fourth, third, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzxw(self) -> I32x4 {
        I32x4([self[3], self[2], self[0], self[3]])
    }

    /// Constructs a new vector from the first, fourth, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwxw(self) -> I32x4 {
        I32x4([self[0], self[3], self[0], self[3]])
    }

    /// Constructs a new vector from the second, fourth, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywxw(self) -> I32x4 {
        I32x4([self[1], self[3], self[0], self[3]])
    }

    /// Constructs a new vector from the third, fourth, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwxw(self) -> I32x4 {
        I32x4([self[2], self[3], self[0], self[3]])
    }

    /// Constructs a new vector from the fourth, fourth, first, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwxw(self) -> I32x4 {
        I32x4([self[3], self[3], self[0], self[3]])
    }

    /// Constructs a new vector from the first, first, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxyw(self) -> I32x4 {
        I32x4([self[0], self[0], self[1], self[3]])
    }

    /// Constructs a new vector from the second, first, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxyw(self) -> I32x4 {
        I32x4([self[1], self[0], self[1], self[3]])
    }

    /// Constructs a new vector from the third, first, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxyw(self) -> I32x4 {
        I32x4([self[2], self[0], self[1], self[3]])
    }

    /// Constructs a new vector from the fourth, first, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxyw(self) -> I32x4 {
        I32x4([self[3], self[0], self[1], self[3]])
    }

    /// Constructs a new vector from the first, second, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyyw(self) -> I32x4 {
        I32x4([self[0], self[1], self[1], self[3]])
    }

    /// Constructs a new vector from the second, second, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyyw(self) -> I32x4 {
        I32x4([self[1], self[1], self[1], self[3]])
    }

    /// Constructs a new vector from the third, second, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyyw(self) -> I32x4 {
        I32x4([self[2], self[1], self[1], self[3]])
    }

    /// Constructs a new vector from the fourth, second, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyyw(self) -> I32x4 {
        I32x4([self[3], self[1], self[1], self[3]])
    }

    /// Constructs a new vector from the first, third, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzyw(self) -> I32x4 {
        I32x4([self[0], self[2], self[1], self[3]])
    }

    /// Constructs a new vector from the second, third, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzyw(self) -> I32x4 {
        I32x4([self[1], self[2], self[1], self[3]])
    }

    /// Constructs a new vector from the third, third, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzyw(self) -> I32x4 {
        I32x4([self[2], self[2], self[1], self[3]])
    }

    /// Constructs a new vector from the fourth, third, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzyw(self) -> I32x4 {
        I32x4([self[3], self[2], self[1], self[3]])
    }

    /// Constructs a new vector from the first, fourth, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwyw(self) -> I32x4 {
        I32x4([self[0], self[3], self[1], self[3]])
    }

    /// Constructs a new vector from the second, fourth, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywyw(self) -> I32x4 {
        I32x4([self[1], self[3], self[1], self[3]])
    }

    /// Constructs a new vector from the third, fourth, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwyw(self) -> I32x4 {
        I32x4([self[2], self[3], self[1], self[3]])
    }

    /// Constructs a new vector from the fourth, fourth, second, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwyw(self) -> I32x4 {
        I32x4([self[3], self[3], self[1], self[3]])
    }

    /// Constructs a new vector from the first, first, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxzw(self) -> I32x4 {
        I32x4([self[0], self[0], self[2], self[3]])
    }

    /// Constructs a new vector from the second, first, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxzw(self) -> I32x4 {
        I32x4([self[1], self[0], self[2], self[3]])
    }

    /// Constructs a new vector from the third, first, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxzw(self) -> I32x4 {
        I32x4([self[2], self[0], self[2], self[3]])
    }

    /// Constructs a new vector from the fourth, first, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxzw(self) -> I32x4 {
        I32x4([self[3], self[0], self[2], self[3]])
    }

    /// Constructs a new vector from the first, second, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyzw(self) -> I32x4 {
        I32x4([self[0], self[1], self[2], self[3]])
    }

    /// Constructs a new vector from the second, second, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyzw(self) -> I32x4 {
        I32x4([self[1], self[1], self[2], self[3]])
    }

    /// Constructs a new vector from the third, second, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyzw(self) -> I32x4 {
        I32x4([self[2], self[1], self[2], self[3]])
    }

    /// Constructs a new vector from the fourth, second, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyzw(self) -> I32x4 {
        I32x4([self[3], self[1], self[2], self[3]])
    }

    /// Constructs a new vector from the first, third, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzzw(self) -> I32x4 {
        I32x4([self[0], self[2], self[2], self[3]])
    }

    /// Constructs a new vector from the second, third, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzzw(self) -> I32x4 {
        I32x4([self[1], self[2], self[2], self[3]])
    }

    /// Constructs a new vector from the third, third, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzzw(self) -> I32x4 {
        I32x4([self[2], self[2], self[2], self[3]])
    }

    /// Constructs a new vector from the fourth, third, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzzw(self) -> I32x4 {
        I32x4([self[3], self[2], self[2], self[3]])
    }

    /// Constructs a new vector from the first, fourth, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwzw(self) -> I32x4 {
        I32x4([self[0], self[3], self[2], self[3]])
    }

    /// Constructs a new vector from the second, fourth, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywzw(self) -> I32x4 {
        I32x4([self[1], self[3], self[2], self[3]])
    }

    /// Constructs a new vector from the third, fourth, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwzw(self) -> I32x4 {
        I32x4([self[2], self[3], self[2], self[3]])
    }

    /// Constructs a new vector from the fourth, fourth, third, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwzw(self) -> I32x4 {
        I32x4([self[3], self[3], self[2], self[3]])
    }

    /// Constructs a new vector from the first, first, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xxww(self) -> I32x4 {
        I32x4([self[0], self[0], self[3], self[3]])
    }

    /// Constructs a new vector from the second, first, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yxww(self) -> I32x4 {
        I32x4([self[1], self[0], self[3], self[3]])
    }

    /// Constructs a new vector from the third, first, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zxww(self) -> I32x4 {
        I32x4([self[2], self[0], self[3], self[3]])
    }

    /// Constructs a new vector from the fourth, first, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wxww(self) -> I32x4 {
        I32x4([self[3], self[0], self[3], self[3]])
    }

    /// Constructs a new vector from the first, second, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xyww(self) -> I32x4 {
        I32x4([self[0], self[1], self[3], self[3]])
    }

    /// Constructs a new vector from the second, second, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yyww(self) -> I32x4 {
        I32x4([self[1], self[1], self[3], self[3]])
    }

    /// Constructs a new vector from the third, second, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zyww(self) -> I32x4 {
        I32x4([self[2], self[1], self[3], self[3]])
    }

    /// Constructs a new vector from the fourth, second, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wyww(self) -> I32x4 {
        I32x4([self[3], self[1], self[3], self[3]])
    }

    /// Constructs a new vector from the first, third, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xzww(self) -> I32x4 {
        I32x4([self[0], self[2], self[3], self[3]])
    }

    /// Constructs a new vector from the second, third, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn yzww(self) -> I32x4 {
        I32x4([self[1], self[2], self[3], self[3]])
    }

    /// Constructs a new vector from the third, third, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zzww(self) -> I32x4 {
        I32x4([self[2], self[2], self[3], self[3]])
    }

    /// Constructs a new vector from the fourth, third, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wzww(self) -> I32x4 {
        I32x4([self[3], self[2], self[3], self[3]])
    }

    /// Constructs a new vector from the first, fourth, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn xwww(self) -> I32x4 {
        I32x4([self[0], self[3], self[3], self[3]])
    }

    /// Constructs a new vector from the second, fourth, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn ywww(self) -> I32x4 {
        I32x4([self[1], self[3], self[3], self[3]])
    }

    /// Constructs a new vector from the third, fourth, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn zwww(self) -> I32x4 {
        I32x4([self[2], self[3], self[3], self[3]])
    }

    /// Constructs a new vector from the fourth, fourth, fourth, and fourth
    /// lanes in this vector, respectively.
    #[inline]
    pub fn wwww(self) -> I32x4 {
        I32x4([self[3], self[3], self[3], self[3]])
    }

    // Concatenations

    #[inline]
    pub fn concat_xy_xy(self, other: I32x4) -> I32x4 {
        I32x4([self[0], self[1], other[0], other[1]])
    }

    // Conversions

    /// Converts these packed integers to floats.
    #[inline]
    pub fn to_f32x4(self) -> F32x4 {
        F32x4([self[0] as f32, self[1] as f32, self[2] as f32, self[3] as f32])
    }
}

impl Index<usize> for I32x4 {
    type Output = i32;
    #[inline]
    fn index(&self, index: usize) -> &i32 {
        &self.0[index]
    }
}

impl IndexMut<usize> for I32x4 {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut i32 {
        &mut self.0[index]
    }
}

impl Add<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    fn add(self, other: I32x4) -> I32x4 {
        I32x4([self[0] + other[0], self[1] + other[1], self[2] + other[2], self[3] + other[3]])
    }
}

impl Sub<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    fn sub(self, other: I32x4) -> I32x4 {
        I32x4([self[0] - other[0], self[1] - other[1], self[2] - other[2], self[3] - other[3]])
    }
}

impl Mul<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    fn mul(self, other: I32x4) -> I32x4 {
        I32x4([self[0] * other[0], self[1] * other[1], self[2] * other[2], self[3] * other[3]])
    }
}

// 32-bit unsigned integers

#[derive(Clone, Copy)]
pub struct U32x4(pub [u32; 4]);

impl U32x4 {
    #[inline]
    pub fn is_all_ones(&self) -> bool {
        self[0] == !0 && self[1] == !0 && self[2] == !0 && self[3] == !0
    }

    #[inline]
    pub fn is_all_zeroes(&self) -> bool {
        self[0] ==  0 && self[1] ==  0 && self[2] ==  0 && self[3] ==  0
    }
}

impl Index<usize> for U32x4 {
    type Output = u32;
    #[inline]
    fn index(&self, index: usize) -> &u32 {
        &self.0[index]
    }
}

// 8-bit unsigned integers

#[derive(Clone, Copy)]
pub struct U8x16([u8; 16]);

impl U8x16 {
    #[inline]
    pub fn as_i32x4(self) -> I32x4 {
        unsafe {
            I32x4(*mem::transmute::<&[u8; 16], &[i32; 4]>(&self.0))
        }
    }

    #[inline]
    pub fn shuffle(self, table: U8x16) -> U8x16 {
        let mut result = [0; 16];
        for index in 0..16 {
            result[index] = self.0[(table.0[index] & 0x0f) as usize]
        }
        U8x16(result)
    }
}
