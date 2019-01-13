// pathfinder/geometry/src/simd.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub type F32x4 = x86::F32x4;
pub type I32x4 = x86::I32x4;
pub type U32x4 = x86::U32x4;
pub type U8x16 = x86::U8x16;

mod x86 {
    use std::arch::x86_64::{self, __m128, __m128i};
    use std::cmp::PartialEq;
    use std::fmt::{self, Debug, Formatter};
    use std::mem;
    use std::ops::{Add, Index, IndexMut, Mul, Sub};

    // 32-bit floats

    #[derive(Clone, Copy)]
    pub struct F32x4(pub __m128);

    impl F32x4 {
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

        #[inline]
        pub fn min(self, other: F32x4) -> F32x4 {
            unsafe { F32x4(x86_64::_mm_min_ps(self.0, other.0)) }
        }

        #[inline]
        pub fn max(self, other: F32x4) -> F32x4 {
            unsafe { F32x4(x86_64::_mm_max_ps(self.0, other.0)) }
        }

        #[inline]
        pub fn packed_eq(self, other: F32x4) -> U32x4 {
            unsafe {
                U32x4(x86_64::_mm_castps_si128(x86_64::_mm_cmpeq_ps(
                    self.0, other.0,
                )))
            }
        }

        #[inline]
        pub fn swap_halves(self) -> F32x4 {
            unsafe { F32x4(x86_64::_mm_shuffle_ps(self.0, self.0, 0b0100_1110)) }
        }

        #[inline]
        pub fn splat_low_half(self) -> F32x4 {
            unsafe { F32x4(x86_64::_mm_shuffle_ps(self.0, self.0, 0b0100_0100)) }
        }

        #[inline]
        pub fn splat_high_half(self) -> F32x4 {
            unsafe { F32x4(x86_64::_mm_shuffle_ps(self.0, self.0, 0b1110_1110)) }
        }

        #[inline]
        pub fn interleave(self, other: F32x4) -> (F32x4, F32x4) {
            unsafe {
                (
                    F32x4(x86_64::_mm_unpacklo_ps(self.0, other.0)),
                    F32x4(x86_64::_mm_unpackhi_ps(self.0, other.0)),
                )
            }
        }

        #[inline]
        pub fn to_i32x4(self) -> I32x4 {
            unsafe { I32x4(x86_64::_mm_cvtps_epi32(self.0)) }
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

        #[inline]
        pub fn as_u8x16(self) -> U8x16 {
            U8x16(self.0)
        }

        #[inline]
        pub fn min(self, other: I32x4) -> I32x4 {
            unsafe { I32x4(x86_64::_mm_min_epi32(self.0, other.0)) }
        }
    }

    impl Index<usize> for I32x4 {
        type Output = i32;
        #[inline]
        fn index(&self, index: usize) -> &i32 {
            unsafe { &mem::transmute::<&__m128i, &[i32; 4]>(&self.0)[index] }
        }
    }

    impl Sub<I32x4> for I32x4 {
        type Output = I32x4;
        #[inline]
        fn sub(self, other: I32x4) -> I32x4 {
            unsafe { I32x4(x86_64::_mm_sub_epi32(self.0, other.0)) }
        }
    }

    // 32-bit unsigned integers

    #[derive(Clone, Copy)]
    pub struct U32x4(pub __m128i);

    impl U32x4 {
        #[inline]
        fn is_all_ones(&self) -> bool {
            unsafe { x86_64::_mm_test_all_ones(self.0) != 0 }
        }
    }

    impl Index<usize> for U32x4 {
        type Output = u32;
        #[inline]
        fn index(&self, index: usize) -> &u32 {
            unsafe { &mem::transmute::<&__m128i, &[u32; 4]>(&self.0)[index] }
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
}
