/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#[macro_use]
extern crate log;

mod gamma_lut;

use gamma_lut::GammaLut;

const CONTRAST: f32 = 0.0;
const GAMMA: f32 = 0.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorU {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ColorU {
    #[inline]
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> ColorU {
        ColorU {
            r: r,
            g: g,
            b: b,
            a: a,
        }
    }
}

pub fn main() {
    let gamma_lut = GammaLut::new(CONTRAST, GAMMA, GAMMA);
    // TODO(pcwalton)
}
