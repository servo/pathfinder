// pathfinder/simd/src/wasm/swizzle_f32x4.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[cfg(target_arch = "wasm32")]
use super::F32x4;

#[cfg(target_arch = "wasm32")]
impl F32x4 {
    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwxx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 0, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwyx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 1, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwzx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 2, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxwx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxwx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxwx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxwx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xywx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yywx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zywx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wywx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzwx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzwx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzwx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzwx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwwx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywwx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwwx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwwx(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 3, 0>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwxy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 0, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwyy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 1, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwzy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 2, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxwy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxwy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxwy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxwy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xywy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yywy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zywy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wywy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzwy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzwy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzwy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzwy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwwy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywwy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwwy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwwy(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 3, 1>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwxz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 0, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwyz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 1, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwzz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 2, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxwz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxwz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxwz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxwz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xywz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yywz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zywz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wywz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzwz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzwz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzwz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzwz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwwz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywwz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwwz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwwz(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 3, 2>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwxw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 0, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwyw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 1, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwzw(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 2, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xxww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 0, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yxww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 0, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zxww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 0, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wxww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 0, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xyww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 1, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yyww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 1, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zyww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 1, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wyww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 1, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xzww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 2, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn yzww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 2, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zzww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 2, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wzww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 2, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn xwww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<0, 3, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn ywww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<1, 3, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn zwww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<2, 3, 3, 3>(
            self.0, self.0,
        ))
    }

    #[inline]
    #[cfg(target_arch = "wasm32")]
    #[target_feature(enable = "simd128")]
    pub fn wwww(self) -> F32x4 {
        F32x4(std::arch::wasm32::i32x4_shuffle::<3, 3, 3, 3>(
            self.0, self.0,
        ))
    }
}
