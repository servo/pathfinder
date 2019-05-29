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

mod swizzle_f32x4;
mod swizzle_i32x4;

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
    pub fn approx_recip(self) -> F32x4 {
        F32x4([1.0 / self[0], 1.0 / self[1], 1.0 / self[2], 1.0 / self[3]])
    }

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
        F32x4([
            self[0].floor(),
            self[1].floor(),
            self[2].floor(),
            self[3].floor(),
        ])
    }

    #[inline]
    pub fn ceil(self) -> F32x4 {
        F32x4([
            self[0].ceil(),
            self[1].ceil(),
            self[2].ceil(),
            self[3].ceil(),
        ])
    }

    #[inline]
    pub fn round(self) -> F32x4 {
        F32x4([
            self[0].round(),
            self[1].round(),
            self[2].round(),
            self[3].round(),
        ])
    }

    #[inline]
    pub fn sqrt(self) -> F32x4 {
        F32x4([
            self[0].sqrt(),
            self[1].sqrt(),
            self[2].sqrt(),
            self[3].sqrt(),
        ])
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
        I32x4([
            self[0] as i32,
            self[1] as i32,
            self[2] as i32,
            self[3] as i32,
        ])
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
        F32x4([
            self[0] + other[0],
            self[1] + other[1],
            self[2] + other[2],
            self[3] + other[3],
        ])
    }
}

impl Mul<F32x4> for F32x4 {
    type Output = F32x4;
    #[inline]
    fn mul(self, other: F32x4) -> F32x4 {
        F32x4([
            self[0] * other[0],
            self[1] * other[1],
            self[2] * other[2],
            self[3] * other[3],
        ])
    }
}

impl Sub<F32x4> for F32x4 {
    type Output = F32x4;
    #[inline]
    fn sub(self, other: F32x4) -> F32x4 {
        F32x4([
            self[0] - other[0],
            self[1] - other[1],
            self[2] - other[2],
            self[3] - other[3],
        ])
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
        unsafe { U8x16(*mem::transmute::<&[i32; 4], &[u8; 16]>(&self.0)) }
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

    // Concatenations

    #[inline]
    pub fn concat_xy_xy(self, other: I32x4) -> I32x4 {
        I32x4([self[0], self[1], other[0], other[1]])
    }

    // Conversions

    /// Converts these packed integers to floats.
    #[inline]
    pub fn to_f32x4(self) -> F32x4 {
        F32x4([
            self[0] as f32,
            self[1] as f32,
            self[2] as f32,
            self[3] as f32,
        ])
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
        I32x4([
            self[0] + other[0],
            self[1] + other[1],
            self[2] + other[2],
            self[3] + other[3],
        ])
    }
}

impl Sub<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    fn sub(self, other: I32x4) -> I32x4 {
        I32x4([
            self[0] - other[0],
            self[1] - other[1],
            self[2] - other[2],
            self[3] - other[3],
        ])
    }
}

impl Mul<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    fn mul(self, other: I32x4) -> I32x4 {
        I32x4([
            self[0] * other[0],
            self[1] * other[1],
            self[2] * other[2],
            self[3] * other[3],
        ])
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
        self[0] == 0 && self[1] == 0 && self[2] == 0 && self[3] == 0
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
        unsafe { I32x4(*mem::transmute::<&[u8; 16], &[i32; 4]>(&self.0)) }
    }

    #[inline]
    pub fn shuffle(self, indices: U8x16) -> U8x16 {
        let mut result = [0; 16];
        for index in 0..16 {
            result[index] = self.0[(indices.0[index] & 0x0f) as usize]
        }
        U8x16(result)
    }
}
