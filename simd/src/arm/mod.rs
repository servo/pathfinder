// pathfinder/simd/src/arm.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::arch::aarch64::{self, float32x4_t, int32x4_t, uint32x4_t, uint64x2_t, uint8x16_t};
use std::arch::aarch64::{uint8x8_t, uint8x8x2_t};
use std::f32;
use std::fmt::{self, Debug, Formatter};
use std::mem;
use std::ops::{Add, Index, IndexMut, Mul, Sub};

mod swizzle_f32x4;
mod swizzle_i32x4;

// 32-bit floats

#[derive(Clone, Copy)]
pub struct F32x4(pub float32x4_t);

impl F32x4 {
    #[inline]
    pub fn new(a: f32, b: f32, c: f32, d: f32) -> F32x4 {
        unsafe { F32x4(mem::transmute([a, b, c, d])) }
    }

    #[inline]
    pub fn splat(x: f32) -> F32x4 {
        F32x4::new(x, x, x, x)
    }

    // Basic operations

    #[inline]
    pub fn approx_recip(self) -> F32x4 {
        unsafe { F32x4(vrecpe_v4f32(self.0)) }
    }

    #[inline]
    pub fn min(self, other: F32x4) -> F32x4 {
        unsafe { F32x4(simd_fmin(self.0, other.0)) }
    }

    #[inline]
    pub fn max(self, other: F32x4) -> F32x4 {
        unsafe { F32x4(simd_fmax(self.0, other.0)) }
    }

    #[inline]
    pub fn clamp(self, min: F32x4, max: F32x4) -> F32x4 {
        self.max(min).min(max)
    }

    #[inline]
    pub fn abs(self) -> F32x4 {
        unsafe { F32x4(fabs_v4f32(self.0)) }
    }

    #[inline]
    pub fn floor(self) -> F32x4 {
        unsafe { F32x4(floor_v4f32(self.0)) }
    }

    #[inline]
    pub fn ceil(self) -> F32x4 {
        unsafe { F32x4(ceil_v4f32(self.0)) }
    }

    #[inline]
    pub fn round(self) -> F32x4 {
        unsafe { F32x4(round_v4f32(self.0)) }
    }

    #[inline]
    pub fn sqrt(self) -> F32x4 {
        unsafe { F32x4(sqrt_v4f32(self.0)) }
    }

    // Packed comparisons

    #[inline]
    pub fn packed_eq(self, other: F32x4) -> U32x4 {
        unsafe { U32x4(simd_eq(self.0, other.0)) }
    }

    #[inline]
    pub fn packed_gt(self, other: F32x4) -> U32x4 {
        unsafe { U32x4(simd_gt(self.0, other.0)) }
    }

    #[inline]
    pub fn packed_le(self, other: F32x4) -> U32x4 {
        unsafe { U32x4(simd_le(self.0, other.0)) }
    }

    #[inline]
    pub fn packed_lt(self, other: F32x4) -> U32x4 {
        unsafe { U32x4(simd_lt(self.0, other.0)) }
    }

    // Converts these packed floats to integers.
    #[inline]
    pub fn to_i32x4(self) -> I32x4 {
        unsafe { I32x4(simd_cast(self.0)) }
    }

    // Concatenations

    #[inline]
    pub fn concat_xy_xy(self, other: F32x4) -> F32x4 {
        unsafe { F32x4(simd_shuffle4(self.0, other.0, [0, 1, 4, 5])) }
    }

    #[inline]
    pub fn concat_xy_zw(self, other: F32x4) -> F32x4 {
        unsafe { F32x4(simd_shuffle4(self.0, other.0, [0, 1, 6, 7])) }
    }

    #[inline]
    pub fn concat_zw_zw(self, other: F32x4) -> F32x4 {
        unsafe { F32x4(simd_shuffle4(self.0, other.0, [2, 3, 6, 7])) }
    }

    #[inline]
    pub fn concat_wz_yx(self, other: F32x4) -> F32x4 {
        unsafe { F32x4(simd_shuffle4(self.0, other.0, [3, 2, 5, 4])) }
    }

    #[inline]
    pub fn cross(&self, other: F32x4) -> F32x4 {
        unimplemented!()
    }
}

impl Default for F32x4 {
    #[inline]
    fn default() -> F32x4 {
        F32x4::new(0.0, 0.0, 0.0, 0.0)
    }
}

impl Index<usize> for F32x4 {
    type Output = f32;
    #[inline]
    fn index(&self, index: usize) -> &f32 {
        unsafe {
            assert!(index < 4);
            let ptr = &self.0 as *const float32x4_t as *const f32;
            mem::transmute::<*const f32, &f32>(ptr.offset(index as isize))
        }
    }
}

impl IndexMut<usize> for F32x4 {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut f32 {
        unsafe {
            assert!(index < 4);
            let ptr = &mut self.0 as *mut float32x4_t as *mut f32;
            mem::transmute::<*mut f32, &mut f32>(ptr.offset(index as isize))
        }
    }
}

impl Debug for F32x4 {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "<{}, {}, {}, {}>", self[0], self[1], self[2], self[3])
    }
}

impl PartialEq for F32x4 {
    #[inline]
    fn eq(&self, other: &F32x4) -> bool {
        self.packed_eq(*other).is_all_ones()
    }
}

impl Add<F32x4> for F32x4 {
    type Output = F32x4;
    #[inline]
    fn add(self, other: F32x4) -> F32x4 {
        unsafe { F32x4(simd_add(self.0, other.0)) }
    }
}

impl Mul<F32x4> for F32x4 {
    type Output = F32x4;
    #[inline]
    fn mul(self, other: F32x4) -> F32x4 {
        unsafe { F32x4(simd_mul(self.0, other.0)) }
    }
}

impl Sub<F32x4> for F32x4 {
    type Output = F32x4;
    #[inline]
    fn sub(self, other: F32x4) -> F32x4 {
        unsafe { F32x4(simd_sub(self.0, other.0)) }
    }
}

// 32-bit signed integers

#[derive(Clone, Copy, Debug)]
pub struct I32x4(pub int32x4_t);

impl I32x4 {
    #[inline]
    pub fn new(a: i32, b: i32, c: i32, d: i32) -> I32x4 {
        unsafe { I32x4(mem::transmute([a, b, c, d])) }
    }

    #[inline]
    pub fn splat(x: i32) -> I32x4 {
        I32x4::new(x, x, x, x)
    }

    #[inline]
    pub fn as_u8x16(self) -> U8x16 {
        unsafe { U8x16(*mem::transmute::<&int32x4_t, &uint8x16_t>(&self.0)) }
    }

    #[inline]
    pub fn min(self, other: I32x4) -> I32x4 {
        unsafe { I32x4(simd_fmin(self.0, other.0)) }
    }

    // Packed comparisons

    #[inline]
    pub fn packed_eq(self, other: I32x4) -> U32x4 {
        unsafe { U32x4(simd_eq(self.0, other.0)) }
    }

    #[inline]
    pub fn packed_le(self, other: I32x4) -> U32x4 {
        unsafe { U32x4(simd_le(self.0, other.0)) }
    }

    // Concatenations

    #[inline]
    pub fn concat_xy_xy(self, other: I32x4) -> I32x4 {
        unsafe { I32x4(simd_shuffle4(self.0, other.0, [0, 1, 4, 5])) }
    }

    // Conversions

    /// Converts these packed integers to floats.
    #[inline]
    pub fn to_f32x4(self) -> F32x4 {
        unsafe { F32x4(simd_cast(self.0)) }
    }
}

impl Default for I32x4 {
    #[inline]
    fn default() -> I32x4 {
        I32x4::new(0, 0, 0, 0)
    }
}

impl Index<usize> for I32x4 {
    type Output = i32;
    #[inline]
    fn index(&self, index: usize) -> &i32 {
        unsafe {
            assert!(index < 4);
            let ptr = &self.0 as *const int32x4_t as *const i32;
            mem::transmute::<*const i32, &i32>(ptr.offset(index as isize))
        }
    }
}

impl IndexMut<usize> for I32x4 {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut i32 {
        unsafe {
            assert!(index < 4);
            let ptr = &mut self.0 as *mut int32x4_t as *mut i32;
            mem::transmute::<*mut i32, &mut i32>(ptr.offset(index as isize))
        }
    }
}

impl Add<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    fn add(self, other: I32x4) -> I32x4 {
        unsafe { I32x4(simd_add(self.0, other.0)) }
    }
}

impl Sub<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    fn sub(self, other: I32x4) -> I32x4 {
        unsafe { I32x4(simd_sub(self.0, other.0)) }
    }
}

impl Mul<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    fn mul(self, other: I32x4) -> I32x4 {
        unsafe { I32x4(simd_mul(self.0, other.0)) }
    }
}

impl PartialEq for I32x4 {
    #[inline]
    fn eq(&self, other: &I32x4) -> bool {
        self.packed_eq(*other).is_all_ones()
    }
}

// 32-bit unsigned integers

#[derive(Clone, Copy)]
pub struct U32x4(pub uint32x4_t);

impl U32x4 {
    #[inline]
    pub fn is_all_ones(&self) -> bool {
        unsafe { aarch64::vminvq_u32(self.0) == !0 }
    }

    #[inline]
    pub fn is_all_zeroes(&self) -> bool {
        unsafe { aarch64::vmaxvq_u32(self.0) == 0 }
    }
}

impl Index<usize> for U32x4 {
    type Output = u32;
    #[inline]
    fn index(&self, index: usize) -> &u32 {
        unsafe {
            assert!(index < 4);
            let ptr = &self.0 as *const uint32x4_t as *const u32;
            mem::transmute::<*const u32, &u32>(ptr.offset(index as isize))
        }
    }
}

// 8-bit unsigned integers

#[derive(Clone, Copy)]
pub struct U8x16(pub uint8x16_t);

impl U8x16 {
    #[inline]
    pub fn as_i32x4(self) -> I32x4 {
        unsafe { I32x4(*mem::transmute::<&uint8x16_t, &int32x4_t>(&self.0)) }
    }

    #[inline]
    pub fn shuffle(self, indices: U8x16) -> U8x16 {
        unsafe {
            let table = mem::transmute::<uint8x16_t, uint8x8x2_t>(self.0);
            let low = aarch64::vtbl2_u8(table, indices.extract_low());
            let high = aarch64::vtbl2_u8(table, indices.extract_high());
            U8x16(aarch64::vcombine_u8(low, high))
        }
    }

    #[inline]
    fn extract_low(self) -> uint8x8_t {
        unsafe {
            let low = simd_extract(mem::transmute::<uint8x16_t, uint64x2_t>(self.0), 0);
            mem::transmute::<u64, uint8x8_t>(low)
        }
    }

    #[inline]
    fn extract_high(self) -> uint8x8_t {
        unsafe {
            let high = simd_extract(mem::transmute::<uint8x16_t, uint64x2_t>(self.0), 1);
            mem::transmute::<u64, uint8x8_t>(high)
        }
    }
}

// Intrinsics

extern "platform-intrinsic" {
    fn simd_add<T>(x: T, y: T) -> T;
    fn simd_mul<T>(x: T, y: T) -> T;
    fn simd_sub<T>(x: T, y: T) -> T;

    fn simd_fmin<T>(x: T, y: T) -> T;
    fn simd_fmax<T>(x: T, y: T) -> T;

    fn simd_eq<T, U>(x: T, y: T) -> U;
    fn simd_gt<T, U>(x: T, y: T) -> U;
    fn simd_le<T, U>(x: T, y: T) -> U;
    fn simd_lt<T, U>(x: T, y: T) -> U;

    fn simd_shuffle4<T, U>(x: T, y: T, idx: [u32; 4]) -> U;

    fn simd_cast<T, U>(x: T) -> U;

    fn simd_insert<T, U>(x: T, index: u32, value: U) -> T;
    fn simd_extract<T, U>(x: T, index: u32) -> U;
}

extern "C" {
    #[link_name = "llvm.fabs.v4f32"]
    fn fabs_v4f32(a: float32x4_t) -> float32x4_t;
    #[link_name = "llvm.floor.v4f32"]
    fn floor_v4f32(a: float32x4_t) -> float32x4_t;
    #[link_name = "llvm.ceil.v4f32"]
    fn ceil_v4f32(a: float32x4_t) -> float32x4_t;
    #[link_name = "llvm.round.v4f32"]
    fn round_v4f32(a: float32x4_t) -> float32x4_t;
    #[link_name = "llvm.sqrt.v4f32"]
    fn sqrt_v4f32(a: float32x4_t) -> float32x4_t;

    #[link_name = "llvm.aarch64.neon.frecpe.v4f32"]
    fn vrecpe_v4f32(a: float32x4_t) -> float32x4_t;
}
