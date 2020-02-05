// pathfinder/geometry/src/gradient.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::sorted_vector::SortedVector;
use pathfinder_color::ColorU;
use std::cmp::PartialOrd;

#[derive(Clone, Debug)]
pub struct Gradient {
    stops: SortedVector<ColorStop>,
}

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct ColorStop {
    pub offset: f32,
    pub color: ColorU,
}

impl Gradient {
    #[inline]
    pub fn add_color_stop(&mut self, stop: ColorStop) {
        self.stops.push(stop);
    }

    #[inline]
    pub fn stops(&self) -> &[ColorStop] {
        &self.stops.array
    }
}
