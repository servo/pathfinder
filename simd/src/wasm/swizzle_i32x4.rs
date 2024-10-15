// pathfinder/simd/src/wasm/swizzle_i32x4.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[cfg(target_arch = "wasm32")]
use super::I32x4;

#[cfg(target_arch = "wasm32")]
impl I32x4 {
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwxx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwyx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwzx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxwx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxwx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxwx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxwx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xywx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yywx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zywx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wywx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzwx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzwx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzwx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzwx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwwx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywwx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwwx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwwx(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwxy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwyy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwzy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxwy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxwy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxwy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxwy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xywy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yywy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zywy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wywy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzwy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzwy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzwy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzwy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwwy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywwy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwwy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwwy(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwxz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwyz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwzz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxwz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxwz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxwz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxwz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xywz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yywz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zywz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wywz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzwz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzwz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzwz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzwz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwwz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywwz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwwz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwwz(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwxw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwyw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwzw(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwww(self) -> I32x4 {
        I32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 3, 3>(
            self.0, self.0,
        ))
    }
}
