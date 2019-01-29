// pathfinder/renderer/src/paint.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! How a path is to be filled.

use std::fmt::{self, Debug, Formatter};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Paint {
    pub color: ColorU,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PaintId(pub u16);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ColorU {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ColorU {
    #[inline]
    pub fn black() -> ColorU {
        ColorU {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }
}

impl Debug for ColorU {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        if self.a == 255 {
            write!(formatter, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            write!(formatter,
                   "rgba({}, {}, {}, {})",
                   self.r,
                   self.g,
                   self.b,
                   self.a as f32 / 255.0)
        }
    }
}


#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderId(pub u16);

#[derive(Clone, Copy, Debug, Default)]
pub struct ObjectShader {
    pub fill_color: ColorU,
}
