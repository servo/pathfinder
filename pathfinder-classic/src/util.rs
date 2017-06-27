// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use num_traits::identities::Zero;
use std::ops::{Add, Div, Mul, Neg, Sub};

pub const F26DOT6_ZERO: F26Dot6 = F26Dot6(0);

pub const F2DOT14_ZERO: F2Dot14 = F2Dot14(0);
pub const F2DOT14_ONE:  F2Dot14 = F2Dot14(1 << 14);

/// 26.6 fixed point.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct F26Dot6(pub i32);

impl Zero for F26Dot6 {
    #[inline]
    fn zero() -> F26Dot6 {
        F26DOT6_ZERO
    }

    #[inline]
    fn is_zero(&self) -> bool {
        *self == F26DOT6_ZERO
    }
}

impl Add<F26Dot6> for F26Dot6 {
    type Output = F26Dot6;

    #[inline]
    fn add(self, other: F26Dot6) -> F26Dot6 {
        F26Dot6(self.0 + other.0)
    }
}

impl Sub<F26Dot6> for F26Dot6 {
    type Output = F26Dot6;

    #[inline]
    fn sub(self, other: F26Dot6) -> F26Dot6 {
        F26Dot6(self.0 - other.0)
    }
}

impl Mul<F26Dot6> for F26Dot6 {
    type Output = F26Dot6;

    #[inline]
    fn mul(self, other: F26Dot6) -> F26Dot6 {
        F26Dot6(((self.0 as i64 * other.0 as i64 + 1 << 5) >> 6) as i32)
    }
}

impl Div<F26Dot6> for F26Dot6 {
    type Output = F26Dot6;

    #[inline]
    fn div(self, other: F26Dot6) -> F26Dot6 {
        F26Dot6((((self.0 as i64) << 6) / other.0 as i64) as i32)
    }
}

impl Neg for F26Dot6 {
    type Output = F26Dot6;

    #[inline]
    fn neg(self) -> F26Dot6 {
        F26Dot6(-self.0)
    }
}

/// 2.14 fixed point.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct F2Dot14(pub i16);

impl Zero for F2Dot14 {
    #[inline]
    fn zero() -> F2Dot14 {
        F2DOT14_ZERO
    }

    #[inline]
    fn is_zero(&self) -> bool {
        *self == F2DOT14_ZERO
    }
}

impl Add<F2Dot14> for F2Dot14 {
    type Output = F2Dot14;

    #[inline]
    fn add(self, other: F2Dot14) -> F2Dot14 {
        F2Dot14(self.0 + other.0)
    }
}

impl Mul<i16> for F2Dot14 {
    type Output = i16;

    #[inline]
    fn mul(self, other: i16) -> i16 {
        ((self.0 as i32 * other as i32) >> 14) as i16
    }
}

impl Neg for F2Dot14 {
    type Output = F2Dot14;

    #[inline]
    fn neg(self) -> F2Dot14 {
        F2Dot14(-self.0)
    }
}

/// A faster version of `Seek` that supports only forward motion from the current position.
pub trait Jump {
    /// Moves the pointer forward `n` bytes from the *current* position.
    fn jump(&mut self, n: usize) -> Result<(), ()>;
}

impl<'a> Jump for &'a [u8] {
    #[inline]
    fn jump(&mut self, n: usize) -> Result<(), ()> {
        if n <= self.len() {
            *self = &(*self)[n..];
            Ok(())
        } else {
            Err(())
        }
    }
}

