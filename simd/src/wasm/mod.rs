// pathfinder/simd/src/wasm.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::arch::wasm32::v128;
use std::cmp::PartialEq;
use std::mem;
use std::fmt::{self, Debug, Formatter};
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Index, IndexMut, Mul, Not, Shr, Sub};
use std::sync::OnceLock;

mod swizzle_f32x4;
mod swizzle_i32x4;


// Define all our statics here...

#[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
fn xy_mask() -> &'static v128 {
    static MEM: OnceLock<v128> = OnceLock::new();
    MEM.get_or_init(|| {
        std::arch::wasm32::u32x4(!0, !0, 0, 0)
    })
}

// Two 32-bit floats

#[derive(Clone, Copy)]
#[cfg(target_arch = "wasm32")]
pub struct F32x2(pub std::arch::wasm32::v128);

#[cfg(target_arch = "wasm32")]
impl F32x2 {
    // Constructors

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn new(a: f32, b: f32) -> F32x2 {
        F32x2(std::arch::wasm32::f32x4(a, b, 0.0, 0.0))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn splat(x: f32) -> F32x2 {
        F32x2::new(x, x)
    }

    // Basic operations

    #[inline]
    pub fn approx_recip(self) -> F32x2 {
        self.to_f32x4().approx_recip().xy()
    }

    #[inline]
    pub fn min(self, other: F32x2) -> F32x2 {
        self.to_f32x4().min(other.to_f32x4()).xy()
    }

    #[inline]
    pub fn max(self, other: F32x2) -> F32x2 {
        self.to_f32x4().max(other.to_f32x4()).xy()
    }

    #[inline]
    pub fn clamp(self, min: F32x2, max: F32x2) -> F32x2 {
        self.to_f32x4().clamp(min.to_f32x4(), max.to_f32x4()).xy()
    }

    #[inline]
    pub fn abs(self) -> F32x2 {
        self.to_f32x4().abs().xy()
    }

    #[inline]
    pub fn floor(self) -> F32x2 {
        self.to_f32x4().floor().xy()
    }

    #[inline]
    pub fn ceil(self) -> F32x2 {
        self.to_f32x4().ceil().xy()
    }

    #[inline]
    pub fn sqrt(self) -> F32x2 {
        self.to_f32x4().sqrt().xy()
    }

    // Packed comparisons

    #[inline]
    pub fn packed_eq(self, other: F32x2) -> U32x2 {
        self.to_f32x4().packed_eq(other.to_f32x4()).xy()
    }

    #[inline]
    pub fn packed_gt(self, other: F32x2) -> U32x2 {
        self.to_f32x4().packed_gt(other.to_f32x4()).xy()
    }

    #[inline]
    pub fn packed_lt(self, other: F32x2) -> U32x2 {
        self.to_f32x4().packed_lt(other.to_f32x4()).xy()
    }

    #[inline]
    pub fn packed_le(self, other: F32x2) -> U32x2 {
        self.to_f32x4().packed_le(other.to_f32x4()).xy()
    }

    // Conversions

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn to_f32x4(self) -> F32x4 {
        F32x4(self.0)
    }

    #[inline]
    pub fn to_i32x2(self) -> I32x2 {
        self.to_i32x4().xy()
    }

    #[inline]
    pub fn to_i32x4(self) -> I32x4 {
        self.to_f32x4().to_i32x4()
    }

    // Swizzle

    #[inline]
    pub fn yx(self) -> F32x2 {
        self.to_f32x4().yx()
    }

    // Concatenations

    #[inline]
    pub fn concat_xy_xy(self, other: F32x2) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 4,5>(
            self.0, other.0,
        ))
    }
}

#[cfg(target_arch = "wasm32")]
impl Default for F32x2 {
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn default() -> F32x2 {
        F32x2::new(0.0, 0.0)
    }
}

#[cfg(target_arch = "wasm32")]
impl Index<usize> for F32x2 {
    type Output = f32;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn index(&self, index: usize) -> &f32 {
        unsafe { &mem::transmute::<&std::arch::wasm32::v128, &[f32; 4]>(&self.0)[index] }
    }
}

#[cfg(target_arch = "wasm32")]
impl IndexMut<usize> for F32x2 {
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn index_mut(&mut self, index: usize) -> &mut f32 {
        unsafe {
            &mut mem::transmute::<&mut std::arch::wasm32::v128, &mut [f32; 4]>(&mut self.0)[index]
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl Debug for F32x2 {
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "<{}, {}>",
            std::arch::wasm32::f32x4_extract_lane::<0>(self.0),
            std::arch::wasm32::f32x4_extract_lane::<1>(self.0)
        )
    }
}

#[cfg(target_arch = "wasm32")]
impl PartialEq for F32x2 {
    #[inline]
    fn eq(&self, other: &F32x2) -> bool {
        self.packed_eq(*other).all_true()
    }
}

#[cfg(target_arch = "wasm32")]
impl Add<F32x2> for F32x2 {
    type Output = F32x2;
    #[inline]
    fn add(self, other: F32x2) -> F32x2 {
        (self.to_f32x4() + other.to_f32x4()).xy()
    }
}

#[cfg(target_arch = "wasm32")]
impl Div<F32x2> for F32x2 {
    type Output = F32x2;
    #[inline]
    fn div(self, other: F32x2) -> F32x2 {
        (self.to_f32x4() / other.to_f32x4()).xy()
    }
}

#[cfg(target_arch = "wasm32")]
impl Mul<F32x2> for F32x2 {
    type Output = F32x2;
    #[inline]
    fn mul(self, other: F32x2) -> F32x2 {
        (self.to_f32x4() * other.to_f32x4()).xy()
    }
}

#[cfg(target_arch = "wasm32")]
impl Sub<F32x2> for F32x2 {
    type Output = F32x2;
    #[inline]
    fn sub(self, other: F32x2) -> F32x2 {
        (self.to_f32x4() - other.to_f32x4()).xy()
    }
}

// Four 32-bit floats

#[derive(Clone, Copy)]
#[cfg(target_arch = "wasm32")]
pub struct F32x4(pub std::arch::wasm32::v128);

impl F32x4 {
    // Constructors

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn new(a: f32, b: f32, c: f32, d: f32) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4(a, b, c, d))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn splat(x: f32) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4_splat(x))
    }

    // Basic operations

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn approx_recip(self) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4_div(
            std::arch::wasm32::f32x4_splat(1.0),
            self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn min(self, other: F32x4) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4_min(self.0, other.0))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn max(self, other: F32x4) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4_max(self.0, other.0))
    }

    #[inline]
    pub fn clamp(self, min: F32x4, max: F32x4) -> F32x4 {
        self.max(min).min(max)
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn abs(self) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4_abs(self.0))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn floor(self) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4_floor(self.0))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ceil(self) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4_ceil(self.0))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn sqrt(self) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4_sqrt(self.0))
    }

    // Packed comparisons

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn packed_eq(self, other: F32x4) -> U32x4 {
        U32x4(std::arch::wasm32::i32x4_eq(self.0, other.0))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn packed_gt(self, other: F32x4) -> U32x4 {
        U32x4(std::arch::wasm32::i32x4_gt(self.0, other.0))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn packed_lt(self, other: F32x4) -> U32x4 {
        other.packed_gt(self)
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    pub fn packed_le(self, other: F32x4) -> U32x4 {
        !self.packed_gt(other)
    }

    // Conversions

    /// Converts these packed floats to integers via rounding.
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn to_i32x4(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_trunc_sat_f32x4(
       std::arch::wasm32::f32x4_nearest(self.0)
        ))
    }

    // Extraction

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xy(self) -> F32x2 {
        F32x2(std::arch::wasm32::v128_and(self.0, *xy_mask()))
    }

    #[inline]
    pub fn xw(self) -> F32x2 {
        self.xwyz().xy()
    }

    #[inline]
    pub fn yx(self) -> F32x2 {
        self.yxwz().xy()
    }

    #[inline]
    pub fn zy(self) -> F32x2 {
        self.zyxw().xy()
    }

    #[inline]
    pub fn zw(self) -> F32x2 {
        self.zwxy().xy()
    }

    // Concatenations

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn concat_xy_xy(self, other: F32x4) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 4,5>(
            self.0, other.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn concat_xy_zw(self, other: F32x4) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 6, 7>(
            self.0, other.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn concat_zw_zw(self, other: F32x4) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 6, 7>(
            self.0, other.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    // [x, y, z, w] -> [z, w, x, y]
    pub fn concat_wz_yx(self, other: F32x4) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 5, 4>(
            self.0, other.0,
        ))
    }
}

#[cfg(target_arch = "wasm32")]
impl Default for F32x4 {
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn default() -> F32x4 {
        F32x4::splat(0.0)
    }
}

#[cfg(target_arch = "wasm32")]
impl Index<usize> for F32x4 {
    type Output = f32;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn index(&self, index: usize) -> &f32 {
        unsafe { &mem::transmute::<&std::arch::wasm32::v128, &[f32; 4]>(&self.0)[index] }
    }
}

#[cfg(target_arch = "wasm32")]
impl IndexMut<usize> for F32x4 {
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn index_mut(&mut self, index: usize) -> &mut f32 {
        unsafe {
            &mut mem::transmute::<&mut std::arch::wasm32::v128, &mut [f32; 4]>(&mut self.0)[index]
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
        self.packed_eq(*other).all_true()
    }
}

#[cfg(target_arch = "wasm32")]
impl Add<F32x4> for F32x4 {
    type Output = F32x4;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn add(self, other: F32x4) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4_add(self.0, other.0))
    }
}

#[cfg(target_arch = "wasm32")]
impl Div<F32x4> for F32x4 {
    type Output = F32x4;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn div(self, other: F32x4) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4_div(self.0, other.0))
    }
}

#[cfg(target_arch = "wasm32")]
impl Mul<F32x4> for F32x4 {
    type Output = F32x4;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn mul(self, other: F32x4) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4_mul(self.0, other.0))
    }
}

#[cfg(target_arch = "wasm32")]
impl Sub<F32x4> for F32x4 {
    type Output = F32x4;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn sub(self, other: F32x4) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4_sub(self.0, other.0))
    }
}

// Two 32-bit signed integers

#[derive(Clone, Copy)]
#[cfg(target_arch = "wasm32")]
pub struct I32x2(pub std::arch::wasm32::v128);

impl I32x2 {
    // Constructors

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn new(a: i32, b: i32) -> I32x2 {
        I32x2(std::arch::wasm32::i32x4(a, b, 0, 0))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn splat(x: i32) -> I32x2 {
        I32x2::new(x, x)
    }

    // Accessors

    #[inline]
    pub fn x(self) -> i32 {
        self[0]
    }

    #[inline]
    pub fn y(self) -> i32 {
        self[1]
    }

    // Concatenations

    #[inline]
    pub fn concat_xy_xy(self, other: I32x2) -> I32x4 {
        self.to_i32x4().concat_xy_xy(other.to_i32x4())
    }

    // Conversions

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn to_i32x4(self) -> I32x4 {
        I32x4(self.0)
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn to_f32x4(self) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4_convert_i32x4(self.0))
    }

    /// Converts these packed integers to floats.
    #[inline]
    pub fn to_f32x2(self) -> F32x2 {
        self.to_f32x4().xy()
    }

    // Basic operations

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn max(self, other: I32x2) -> I32x2 {
        I32x2(std::arch::wasm32::i32x4_max(self.0, other.0))
    }

    #[inline]
    pub fn min(self, other: I32x2) -> I32x2 {
        I32x2(std::arch::wasm32::i32x4_min(self.0, other.0))
    }

    // Comparisons

    #[inline]
    pub fn packed_eq(self, other: I32x2) -> U32x2 {
        U32x2(std::arch::wasm32::i32x4_eq(self.0, other.0))
    }

    #[inline]
    pub fn packed_gt(self, other: I32x2) -> U32x2 {
        U32x2(std::arch::wasm32::i32x4_gt(self.0, other.0))
    }

    #[inline]
    pub fn packed_le(self, other: I32x2) -> U32x2 {
        U32x2(std::arch::wasm32::i32x4_le(self.0, other.0))
    }
}

#[cfg(target_arch = "wasm32")]
impl Default for I32x2 {
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn default() -> I32x2 {
        I32x2(std::arch::wasm32::i32x4(0, 0, 0, 0))
    }
}

#[cfg(target_arch = "wasm32")]
impl Index<usize> for I32x2 {
    type Output = i32;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn index(&self, index: usize) -> &i32 {
        unsafe { &mem::transmute::<&std::arch::wasm32::v128, &[i32; 4]>(&self.0)[index] }
    }
}

#[cfg(target_arch = "wasm32")]
impl IndexMut<usize> for I32x2 {
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn index_mut(&mut self, index: usize) -> &mut i32 {
        unsafe {
            &mut mem::transmute::<&mut std::arch::wasm32::v128, &mut [i32; 4]>(&mut self.0)[index]
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl Add<I32x2> for I32x2 {
    type Output = I32x2;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn add(self, other: I32x2) -> I32x2 {
        I32x2(std::arch::wasm32::i32x4_add(self.0, other.0))
    }
}

#[cfg(target_arch = "wasm32")]
impl Sub<I32x2> for I32x2 {
    type Output = I32x2;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn sub(self, other: I32x2) -> I32x2 {
        I32x2(std::arch::wasm32::i32x4_sub(self.0, other.0))
    }
}

#[cfg(target_arch = "wasm32")]
impl Mul<I32x2> for I32x2 {
    type Output = I32x2;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn mul(self, other: I32x2) -> I32x2 {
        I32x2(std::arch::wasm32::i32x4_mul(self.0, other.0))
    }
}

impl Debug for I32x2 {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "<{}, {}>", self[0], self[1])
    }
}

impl PartialEq for I32x2 {
    #[inline]
    fn eq(&self, other: &I32x2) -> bool {
        self.packed_eq(*other).all_true()
    }
}

// Four 32-bit signed integers

#[derive(Clone, Copy)]
#[cfg(target_arch = "wasm32")]
pub struct I32x4(pub std::arch::wasm32::v128);

impl I32x4 {
    // Constructors

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn new(a: i32, b: i32, c: i32, d: i32) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4(a, b, c, d))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn splat(x: i32) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_splat(x))
    }

    // Extraction

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xy(self) -> I32x2 {
        I32x2(std::arch::wasm32::v128_and(self.0, *xy_mask()))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xw(self) -> I32x2 {
        self.xwyz().xy()
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yx(self) -> I32x2 {
        self.yxwz().xy()
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zy(self) -> I32x2 {
        self.zyxw().xy()
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zw(self) -> I32x2 {
        self.zwxy().xy()
    }

    // Concatenations

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn concat_xy_xy(self, other: I32x4) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 4, 5>(
            self.0, other.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn concat_zw_zw(self, other: I32x4) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 6, 7>(
            self.0, other.0,
        ))
    }

    // Conversions

    /// Converts these packed integers to floats.
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn to_f32x4(self) -> F32x4 {
        F32x4(std::arch::wasm32::f32x4_convert_i32x4(self.0))
    }

    /// Converts these packed signed integers to unsigned integers.
    ///
    /// Overflowing values will wrap around.
    #[inline]
    pub fn to_u32x4(self) -> U32x4 {
        U32x4(self.0)
    }

    // Basic operations

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn max(self, other: I32x4) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_max(self.0, other.0))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn min(self, other: I32x4) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_min(self.0, other.0))
    }

    // Packed comparisons

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn packed_eq(self, other: I32x4) -> U32x4 {
        U32x4(std::arch::wasm32::i32x4_eq(self.0, other.0))
    }

    // Comparisons

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn packed_gt(self, other: I32x4) -> U32x4 {
        U32x4(std::arch::wasm32::i32x4_gt(self.0, other.0))
    }

    #[inline]
    pub fn packed_lt(self, other: I32x4) -> U32x4 {
        other.packed_gt(self)
    }

    #[inline]
    pub fn packed_le(self, other: I32x4) -> U32x4 {
        !self.packed_gt(other)
    }
}

#[cfg(target_arch = "wasm32")]
impl Default for I32x4 {
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn default() -> I32x4 {
        I32x4::splat(0)
    }
}

#[cfg(target_arch = "wasm32")]
impl Index<usize> for I32x4 {
    type Output = i32;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn index(&self, index: usize) -> &i32 {
        unsafe { &mem::transmute::<&std::arch::wasm32::v128, &[i32; 4]>(&self.0)[index] }
    }
}

#[cfg(target_arch = "wasm32")]
impl IndexMut<usize> for I32x4 {
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn index_mut(&mut self, index: usize) -> &mut i32 {
        unsafe {
            &mut mem::transmute::<&mut std::arch::wasm32::v128, &mut [i32; 4]>(&mut self.0)[index]
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl Add<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn add(self, other: I32x4) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_add(self.0, other.0))
    }
}

#[cfg(target_arch = "wasm32")]
impl Sub<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn sub(self, other: I32x4) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_sub(self.0, other.0))
    }
}

#[cfg(target_arch = "wasm32")]
impl Mul<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn mul(self, other: I32x4) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_mul(self.0, other.0))
    }
}

#[cfg(target_arch = "wasm32")]
impl BitAnd<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn bitand(self, other: I32x4) -> I32x4 {
        I32x4(std::arch::wasm32::v128_and(self.0, other.0))
    }
}

#[cfg(target_arch = "wasm32")]
impl BitOr<I32x4> for I32x4 {
    type Output = I32x4;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn bitor(self, other: I32x4) -> I32x4 {
        I32x4(std::arch::wasm32::v128_or(self.0, other.0))
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
        self.packed_eq(*other).all_true()
    }
}

// Two 32-bit unsigned integers

#[derive(Clone, Copy)]
#[cfg(target_arch = "wasm32")]
pub struct U32x2(pub std::arch::wasm32::v128);

impl U32x2 {
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn new(x: u32, y: u32) -> U32x2 {
        U32x2(std::arch::wasm32::u32x4(x, y, 0, 0))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn splat(x: u32) -> U32x2 {
        U32x2::new(x, x)
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn packed_eq(self, other: U32x2) -> U32x2 {
        U32x2(std::arch::wasm32::i32x4_eq(self.0, other.0))
    }

    /// Returns true if both booleans in this vector are true.
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn all_true(self) -> bool {
        std::arch::wasm32::i32x4_all_true(
            std::arch::wasm32::i32x4_shuffle::<0, 1, 0, 1>(self.0, self.0)
        )
    }

    /// Returns true if both booleans in this vector are false.
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn all_false(self) -> bool {
        !std::arch::wasm32::v128_any_true(
            std::arch::wasm32::i32x4_shuffle::<0, 1, 0, 1>(self.0, self.0)
        )
    }

    #[inline]
    pub fn to_i32x2(self) -> I32x2 {
        I32x2(self.0)
    }
}

#[cfg(target_arch = "wasm32")]
impl Not for U32x2 {
    type Output = U32x2;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn not(self) -> U32x2 {
        U32x2(std::arch::wasm32::v128_bitselect(
            std::arch::wasm32::v128_not(self.0),
            std::arch::wasm32::u32x4_splat(0),
            std::arch::wasm32::i32x4(!0, !0, 0, 0),
        ))
    }
}

#[cfg(target_arch = "wasm32")]
impl Debug for U32x2 {
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "<{}, {}>",
            std::arch::wasm32::u32x4_extract_lane::<0>(self.0),
            std::arch::wasm32::u32x4_extract_lane::<1>(self.0)
        )
    }
}

#[cfg(target_arch = "wasm32")]
impl Index<usize> for U32x2 {
    type Output = u32;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn index(&self, index: usize) -> &u32 {
        unsafe { &mem::transmute::<&std::arch::wasm32::v128, &[u32; 4]>(&self.0)[index] }
    }
}

#[cfg(target_arch = "wasm32")]
impl BitAnd<U32x2> for U32x2 {
    type Output = U32x2;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn bitand(self, other: U32x2) -> U32x2 {
        U32x2(std::arch::wasm32::v128_and(self.0, other.0))
    }
}

#[cfg(target_arch = "wasm32")]
impl BitOr<U32x2> for U32x2 {
    type Output = U32x2;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn bitor(self, other: U32x2) -> U32x2 {
        U32x2(std::arch::wasm32::v128_or(self.0, other.0))
    }
}

#[cfg(target_arch = "wasm32")]
impl PartialEq for U32x2 {
    #[inline]
    fn eq(&self, other: &U32x2) -> bool {
        self.packed_eq(*other).all_true()
    }
}

// Four 32-bit unsigned integers

#[derive(Clone, Copy)]
#[cfg(target_arch = "wasm32")]
pub struct U32x4(pub std::arch::wasm32::v128);

impl U32x4 {
    // Constructors

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn new(a: u32, b: u32, c: u32, d: u32) -> U32x4 {
        U32x4(std::arch::wasm32::u32x4(a, b, c, d))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn splat(x: u32) -> U32x4 {
        U32x4(std::arch::wasm32::u32x4_splat(x))
    }

    // Conversions

    /// Converts these packed unsigned integers to signed integers.
    ///
    /// Overflowing values will wrap around.
    #[inline]
    pub fn to_i32x4(self) -> I32x4 {
        I32x4(self.0)
    }

    // Basic operations

    /// Returns true if all four booleans in this vector are true.
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn all_true(self) -> bool {
        std::arch::wasm32::u32x4_all_true(self.0)
    }

    /// Returns true if all four booleans in this vector are false.
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn all_false(self) -> bool {
        !std::arch::wasm32::v128_any_true(self.0)
    }

    // Extraction

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xy(self) -> U32x2 {
        U32x2(std::arch::wasm32::v128_and(self.0, *xy_mask()))
    }

    // Packed comparisons

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn packed_eq(self, other: U32x4) -> U32x4 {
        U32x4(std::arch::wasm32::i32x4_eq(self.0, other.0))
    }
}

impl Debug for U32x4 {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "<{}, {}, {}, {}>", self[0], self[1], self[2], self[3])
    }
}

#[cfg(target_arch = "wasm32")]
impl Index<usize> for U32x4 {
    type Output = u32;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn index(&self, index: usize) -> &u32 {
        unsafe { &mem::transmute::<&std::arch::wasm32::v128, &[u32; 4]>(&self.0)[index] }
    }
}

impl PartialEq for U32x4 {
    #[inline]
    fn eq(&self, other: &U32x4) -> bool {
        self.packed_eq(*other).all_true()
    }
}

#[cfg(target_arch = "wasm32")]
impl Not for U32x4 {
    type Output = U32x4;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn not(self) -> U32x4 {
        self ^ U32x4::splat(!0)
    }
}

#[cfg(target_arch = "wasm32")]
impl BitXor<U32x4> for U32x4 {
    type Output = U32x4;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn bitxor(self, other: U32x4) -> U32x4 {
        U32x4(std::arch::wasm32::v128_xor(self.0, other.0))
    }
}

#[cfg(target_arch = "wasm32")]
impl Shr<u32> for U32x4 {
    type Output = U32x4;
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    fn shr(self, amount: u32) -> U32x4 {
        U32x4(std::arch::wasm32::u32x4_shr(self.0, amount))
    }
}