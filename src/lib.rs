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
//! ## Introduction
//!
//! Pathfinder is a fast, practical GPU-based rasterizer for OpenType fonts using OpenGL 4.3. It
//! features:
//! 
//! * Very low setup time. Glyph outlines can go from the `.otf` file to the GPU in a form ready
//! for rasterization in less than a microsecond. There is no expensive tessellation or
//! preprocessing step.
//! 
//! * High quality antialiasing. Unlike techniques that rely on multisample antialiasing,
//! Pathfinder computes exact fractional trapezoidal area coverage on a per-pixel basis.
//! 
//! * Fast rendering, even at small pixel sizes. On typical systems, Pathfinder should easily
//! exceed the performance of the best CPU rasterizers.
//! 
//! * Low memory consumption. The only memory overhead over the glyph and outline storage itself is
//! that of a coverage buffer which typically consumes somewhere between 4MB-16MB and can be
//! discarded under memory pressure. Outlines are stored on-GPU in a compressed format and usually
//! take up only a few dozen kilobytes.
//! 
//! * Portability to most GPUs manufactured in the last few years, including integrated GPUs.
//!
//! ## Usage
//!
//! See `examples/generate-atlas.rs` for a simple example.
//!
//! Typically, the steps to use Pathfidner are:
//!
//! 1. Create a `Rasterizer` object. This holds the OpenGL state.
//!
//! 2. Open the font from disk (or elsewhere), and call `Font::new()` (or
//!    `Font::from_collection_index` in the case of a `.ttc` or `.dfont` collection) to load it.
//!     

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

