// pathfinder/font-renderer/src/freetype/fixed.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Utilities for FreeType 26.6 fixed-point numbers.

use app_units::Au;
use freetype_sys::freetype::FT_F26Dot6;

pub trait FromFtF26Dot6 {
    fn from_ft_f26dot6(value: FT_F26Dot6) -> Self;
}

impl FromFtF26Dot6 for f32 {
    fn from_ft_f26dot6(value: FT_F26Dot6) -> f32 {
        (value as f32) / 64.0
    }
}

pub trait ToFtF26Dot6 {
    fn to_ft_f26dot6(self) -> FT_F26Dot6;
}

impl ToFtF26Dot6 for f32 {
    fn to_ft_f26dot6(self) -> FT_F26Dot6 {
        (self * 64.0 + 0.5) as FT_F26Dot6
    }
}

impl ToFtF26Dot6 for f64 {
    fn to_ft_f26dot6(self) -> FT_F26Dot6 {
        (self * 64.0 + 0.5) as FT_F26Dot6
    }
}

impl ToFtF26Dot6 for Au {
    fn to_ft_f26dot6(self) -> FT_F26Dot6 {
        self.to_f64_px().to_ft_f26dot6()
    }
}

#[inline]
pub fn floor(n: FT_F26Dot6) -> FT_F26Dot6 {
    n & !0x3f
}
