// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A high-performance GPU rasterizer for OpenType fonts.
//!
//! Pathfinder rasterizes glyphs from a `.ttf`, `.ttc`, or `.otf` file loaded in memory to an atlas
//! texture.

#![cfg_attr(test, feature(test))]

#[macro_use]
extern crate bitflags;
extern crate byteorder;
extern crate compute_shader;
extern crate euclid;
extern crate gl;
#[cfg(test)]
extern crate memmap;
#[cfg(test)]
#[macro_use]
extern crate quickcheck;
#[cfg(test)]
extern crate test;

pub mod atlas;
pub mod charmap;
pub mod coverage;
pub mod error;
pub mod otf;
pub mod outline;
pub mod rasterizer;
pub mod shaper;

mod rect_packer;
mod util;

#[cfg(test)]
mod tests;

