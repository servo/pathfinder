// pathfinder/simd/src/x86/swizzle_i32x4.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::x86::I32x4;
use std::arch::x86_64;

// TODO(pcwalton): Add the remaining swizzles.
impl I32x4 {
    #[inline]
    pub fn xyxy(self) -> I32x4 {
        unsafe {
            let this = x86_64::_mm_castsi128_ps(self.0);
            I32x4(x86_64::_mm_castps_si128(x86_64::_mm_shuffle_ps(this, this, 68)))
        }
    }

    #[inline]
    pub fn xwzy(self) -> I32x4 {
        unsafe {
            let this = x86_64::_mm_castsi128_ps(self.0);
            I32x4(x86_64::_mm_castps_si128(x86_64::_mm_shuffle_ps(this, this, 108)))
        }
    }

    #[inline]
    pub fn zyxw(self) -> I32x4 {
        unsafe {
            let this = x86_64::_mm_castsi128_ps(self.0);
            I32x4(x86_64::_mm_castps_si128(x86_64::_mm_shuffle_ps(this, this, 198)))
        }
    }

    #[inline]
    pub fn zwxy(self) -> I32x4 {
        unsafe {
            let this = x86_64::_mm_castsi128_ps(self.0);
            I32x4(x86_64::_mm_castps_si128(x86_64::_mm_shuffle_ps(this, this, 78)))
        }
    }
}
