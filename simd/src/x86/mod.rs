// pathfinder/simd/src/x86.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::arch::x86_64::{self, __m128, __m128i};
use std::cmp::PartialEq;
use std::fmt::{self, Debug, Formatter};
use std::mem;
use std::ops::{Add, BitXor, Index, IndexMut, Mul, Not, Sub};

mod swizzle_f32x4;
mod swizzle_i32x4;

// 32-bit floats

#[derive(Clone, Copy)]
pub struct F32x4(pub __m128);

impl F32x4 {
    // Constructors

    #[inline]
    pub fn new(a: f32, b: f32, c: f32, d: f32) -> F32x4 {
        unsafe {
            let vector = [a, b, c, d];
            F32x4(x86_64::_mm_loadu_ps(vector.as_ptr()))
        }
    }

    #[inline]
    pub fn splat(x: f32) -> F32x4 {
        unsafe { F32x4(x86_64::_mm_set1_ps(x)) }
    }

    // Basic operations

    #[inline]
    pub fn approx_recip(self) -> F32x4 {
        unsafe { F32x4(x86_64::_mm_rcp_ps(self.0)) }
    }

    #[inline]
    pub fn min(self, other: F32x4) -> F32x4 {
        unsafe { F32x4(x86_64::_mm_min_ps(self.0, other.0)) }
    }

    #[inline]
    pub fn max(self, other: F32x4) -> F32x4 {
        unsafe { F32x4(x86_64::_mm_max_ps(self.0, other.0)) }
    }

    #[inline]
    pub fn clamp(self, min: F32x4, max: F32x4) -> F32x4 {
        self.max(min).min(max)
    }

    #[inline]
    pub fn abs(self) -> F32x4 {
        unsafe {
            let tmp = x86_64::_mm_srli_epi32(I32x4::splat(-1).0, 1);
            F32x4(x86_64::_mm_and_ps(x86_64::_mm_castsi128_ps(tmp), self.0))
        }
    }

    #[inline]
    pub fn floor(self) -> F32x4 {
        unsafe { F32x4(x86_64::_mm_floor_ps(self.0)) }
    }

    #[inline]
    pub fn ceil(self) -> F32x4 {
        unsafe { F32x4(x86_64::_mm_ceil_ps(self.0)) }
    }

    // Packed comparisons

    #[inline]
    pub fn packed_eq(self, other: F32x4) -> U32x4 {
        unsafe {
            U32x4(x86_64::_mm_castps_si128(x86_64::_mm_cmpeq_ps(
                self.0, other.0,
            )))
        }
    }

    #[inline]
    pub fn packed_gt(self, other: F32x4) -> U32x4 {
        unsafe {
            U32x4(x86_64::_mm_castps_si128(x86_64::_mm_cmpgt_ps(
                self.0, other.0,
            )))
        }
    }

    #[inline]
    pub fn packed_lt(self, other: F32x4) -> U32x4 {
        other.packed_gt(self)
    }

    #[inline]
    pub fn packed_le(self, other: F32x4) -> U32x4 {
        !self.packed_gt(other)
    }

    // Conversions

    /// Converts these packed floats to integers.
    #[inline]
    pub fn to_i32x4(self) -> I32x4 {
        unsafe { I32x4(x86_64::_mm_cvtps_epi32(self.0)) }
    }

    // Concatenations

    #[inline]
    pub fn concat_xy_xy(self, other: F32x4) -> F32x4 {
        unsafe {
            let this = x86_64::_mm_castps_pd(self.0);
            let other = x86_64::_mm_castps_pd(other.0);
            let result = x86_64::_mm_unpacklo_pd(this, other);
            F32x4(x86_64::_mm_castpd_ps(result))
        }
    }

    #[inline]
    pub fn concat_xy_zw(self, other: F32x4) -> F32x4 {
        unsafe {
            let this = x86_64::_mm_castps_pd(self.0);
            let other = x86_64::_mm_castps_pd(other.0);
            let result = x86_64::_mm_shuffle_pd(this, other, 0b10);
            F32x4(x86_64::_mm_castpd_ps(result))
        }
    }

    #[inline]
    pub fn concat_zw_zw(self, other: F32x4) -> F32x4 {
        unsafe {
            let this = x86_64::_mm_castps_pd(self.0);
            let other = x86_64::_mm_castps_pd(other.0);
            let result = x86_64::_mm_unpackhi_pd(this, other);
            F32x4(x86_64::_mm_castpd_ps(result))
        }
    }

    #[inline]
    pub fn concat_wz_yx(self, other: F32x4) -> F32x4 {
        unsafe { F32x4(x86_64::_mm_shuffle_ps(self.0, other.0, 0b0001_1011)) }
    }

    // FIXME(pcwalton): Move to `Point3DF32`!
    #[inline]
    pub fn cross(&self, other: F32x4) -> F32x4 {
        self.yzxw() * other.zxyw() - self.zxyw() * other.yzxw()
    }
}

impl Default for F32x4 {
    #[inline]
    fn default() -> F32x4 {
        unsafe { F32x4(x86_64::_mm_setzero_ps()) }
    }
}

impl Index<usize> for F32x4 {
    type Output = f32;
    #[inline]
    fn index(&self, index: usize) -> &f32 {
        unsafe { &mem::transmute::<&__m128, &[f32; 4]>(&self.0)[index] }
    }
}

impl IndexMut<usize> for F32x4 {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut f32 {
        unsafe { &mut mem::transmute::<&mut __m128, &mut [f32; 4]>(&mut self.0)[index] }
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
        unsafe { F32x4(x86_64::_mm_add_ps(self.0, other.0)) }
    }
}

impl Mul<F32x4> for F32x4 {
    type Output = F32x4;
    #[inline]
    fn mul(self, other: F32x4) -> F32x4 {
        unsafe { F32x4(x86_64::_mm_mul_ps(self.0, other.0)) }
    }
}

impl Sub<F32x4> for F32x4 {
    type Output = F32x4;
    #[inline]
    fn sub(self, other: F32x4) -> F32x4 {
        unsafe { F32x4(x86_64::_mm_sub_ps(self.0, other.0)) }
    }
}

// 32-bit signed integers

#[derive(Clone, Copy)]
pub struct I32x4(pub __m128i);

impl I32x4 {
    // Constructors

    #[inline]
    pub fn new(a: i32, b: i32, c: i32, d: i32) -> I32x4 {
        unsafe {
            let vector = [a, b, c, d];
            I32x4(x86_64::_mm_loadu_si128(vector.as_ptr() as *const __m128i))
        }
    }

    #[inline]
    pub fn splat(x: i32) -> I32x4 {
        unsafe { I32x4(x86_64::_mm_set1_epi32(x)) }
    }

    // Concatenations

    #[inline]
    pub fn concat_xy_xy(self, other: I32x4) -> I32x4 {
        unsafe {
            let this = x86_64::_mm_castsi128_pd(self.0);
            let other = x86_64::_mm_castsi128_pd(other.0);
            let result = x86_64::_mm_unpacklo_pd(this, other);
            I32x4(x86_64::_mm_castpd_si128(result))
        }
    }

    // Conversions

    #[inline]
    pub fn as_u8x16(self) -> U8x16 {
        U8x16(self.0)
    }

    /// Converts these packed integers to floats.
    #[inline]
    pub fn to_f32x4(self) -> F32x4 {
        unsafe { F32x4(x86_64::_mm_cvtepi32_ps(self.0)) }
    }

    // Basic operations

    #[inline]
    pub fn min(self, other: I32x4) -> I32x4 {
        unsafe { I32x4(x86_64::_mm_min_epi32(self.0, other.0)) }
    }

    // Packed comparisons

    #[inline]
    pub fn packed_eq(self, other: I32x4) -> U32x4 {
        unsafe { U32x4(x86_64::_mm_cmpeq_epi32(self.0, other.0)) }
    }

    // Comparisons

    #[inline]
    pub fn packed_gt(self, other: I32x4) -> U32x4 {
        unsafe {
            U32x4(x86_64::_mm_cmpgt_epi32(self.0, other.0))
        }
    }

    #[inline]
    pub fn packed_le(self, other: I32x4) -> U32x4 {
        !self.packed_gt(other)
    }
}

impl Default for I32x4 {
    #[inline]
    fn default() -> I32x4 {
        unsafe { I32x4(x86_64::_mm_setzero_si128()) }
    }
}

impl Index<usize> for I32x4 {
    type Output = i32;
    #[inline]
    fn index(&self, index: usize) -> &i32 {
        unsafe { &mem::transmute::<&__m128i, &[i32; 4]>(&self.0)[index] }
    }
}

impl IndexMut<usize> for I32x4 {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut i32 {
        unsafe { &mut mem::transmute::<&mut __m128i, &mut [i32; 4]>(&mut self.0)[index] }
    }
}

impl Add<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    fn add(self, other: I32x4) -> I32x4 {
        unsafe { I32x4(x86_64::_mm_add_epi32(self.0, other.0)) }
    }
}

impl Sub<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    fn sub(self, other: I32x4) -> I32x4 {
        unsafe { I32x4(x86_64::_mm_sub_epi32(self.0, other.0)) }
    }
}

impl Mul<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    fn mul(self, other: I32x4) -> I32x4 {
        unsafe { I32x4(x86_64::_mm_mullo_epi32(self.0, other.0)) }
    }
}

impl Debug for I32x4 {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "<{}, {}, {}, {}>", self[0], self[1], self[2], self[3])
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
pub struct U32x4(pub __m128i);

impl U32x4 {
    // Constructors

    #[inline]
    pub fn new(a: u32, b: u32, c: u32, d: u32) -> U32x4 {
        unsafe {
            let vector = [a, b, c, d];
            U32x4(x86_64::_mm_loadu_si128(vector.as_ptr() as *const __m128i))
        }
    }

    #[inline]
    pub fn splat(x: u32) -> U32x4 {
        unsafe { U32x4(x86_64::_mm_set1_epi32(x as i32)) }
    }

    // Basic operations

    #[inline]
    pub fn is_all_ones(self) -> bool {
        unsafe { x86_64::_mm_test_all_ones(self.0) != 0 }
    }

    #[inline]
    pub fn is_all_zeroes(self) -> bool {
        unsafe { x86_64::_mm_test_all_zeros(self.0, self.0) != 0 }
    }

    // Packed comparisons

    #[inline]
    pub fn packed_eq(self, other: U32x4) -> U32x4 {
        unsafe { U32x4(x86_64::_mm_cmpeq_epi32(self.0, other.0)) }
    }
}

impl Debug for U32x4 {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "<{}, {}, {}, {}>", self[0], self[1], self[2], self[3])
    }
}

impl Index<usize> for U32x4 {
    type Output = u32;
    #[inline]
    fn index(&self, index: usize) -> &u32 {
        unsafe { &mem::transmute::<&__m128i, &[u32; 4]>(&self.0)[index] }
    }
}

impl PartialEq for U32x4 {
    #[inline]
    fn eq(&self, other: &U32x4) -> bool {
        self.packed_eq(*other).is_all_ones()
    }
}

impl Not for U32x4 {
    type Output = U32x4;
    #[inline]
    fn not(self) -> U32x4 {
        self ^ U32x4::splat(!0)
    }
}

impl BitXor<U32x4> for U32x4 {
    type Output = U32x4;
    #[inline]
    fn bitxor(self, other: U32x4) -> U32x4 {
        unsafe {
            U32x4(x86_64::_mm_xor_si128(self.0, other.0))
        }
    }
}

// 8-bit unsigned integers

#[derive(Clone, Copy)]
pub struct U8x16(pub __m128i);

impl U8x16 {
    #[inline]
    pub fn as_i32x4(self) -> I32x4 {
        I32x4(self.0)
    }

    #[inline]
    pub fn shuffle(self, indices: U8x16) -> U8x16 {
        unsafe { U8x16(x86_64::_mm_shuffle_epi8(self.0, indices.0)) }
    }
}
