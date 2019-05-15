// pathfinder/geometry/src/color.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use pathfinder_simd::default::F32x4;
use std::fmt::{self, Debug, Formatter};

// TODO(pcwalton): Maybe this should be a u32?
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ColorU {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ColorU {
    #[inline]
    pub fn from_u32(rgba: u32) -> ColorU {
        ColorU {
            r: (rgba >> 24) as u8,
            g: ((rgba >> 16) & 0xff) as u8,
            b: ((rgba >> 8) & 0xff) as u8,
            a: (rgba & 0xff) as u8,
        }
    }

    #[inline]
    pub fn black() -> ColorU {
        ColorU {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }

    #[inline]
    pub fn to_f32(&self) -> ColorF {
        let color = F32x4::new(self.r as f32, self.g as f32, self.b as f32, self.a as f32);
        ColorF(color * F32x4::splat(1.0 / 255.0))
    }

    #[inline]
    pub fn is_opaque(&self) -> bool {
        self.a == 255
    }
}

impl Debug for ColorU {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        if self.a == 255 {
            write!(formatter, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            write!(
                formatter,
                "rgba({}, {}, {}, {})",
                self.r,
                self.g,
                self.b,
                self.a as f32 / 255.0
            )
        }
    }
}

#[derive(Clone, Copy)]
pub struct ColorF(pub F32x4);

impl ColorF {
    #[inline]
    pub fn transparent_black() -> ColorF {
        ColorF(F32x4::default())
    }

    #[inline]
    pub fn white() -> ColorF {
        ColorF(F32x4::splat(1.0))
    }

    #[inline]
    pub fn to_u8(&self) -> ColorU {
        let color = (self.0 * F32x4::splat(255.0)).round().to_i32x4();
        ColorU { r: color[0] as u8, g: color[1] as u8, b: color[2] as u8, a: color[3] as u8 }
    }

    #[inline]
    pub fn lerp(&self, other: ColorF, t: f32) -> ColorF {
        ColorF(self.0 + (other.0 - self.0) * F32x4::splat(t))
    }

    #[inline]
    pub fn r(&self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub fn g(&self) -> f32 {
        self.0[1]
    }

    #[inline]
    pub fn b(&self) -> f32 {
        self.0[2]
    }

    #[inline]
    pub fn a(&self) -> f32 {
        self.0[3]
    }
}
