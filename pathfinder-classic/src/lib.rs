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
//! Typically, the steps to use Pathfinder are:
//!
//! 1. Create a `Rasterizer` object. This holds the OpenGL state.
//!
//! 2. Open the font from disk (or elsewhere), and call `Font::new()` (or
//!    `Font::from_collection_index` in the case of a `.ttc` or `.dfont` collection) to load it.
//!
//! 3. If the text to be rendered is not already shaped, call
//!    `Font::glyph_mapping_for_codepoint_ranges()` to determine the glyphs needed to render the
//!    text, and call `shaper::shape_text()` to convert the text to glyph IDs.
//!
//! 4. Create an `OutlineBuilder` and call `OutlineBuilder::add_glyph()` on each glyph to parse
//!    each outline from the font. Then upload the outlines to the GPU with
//!    `OutlineBuilder::create_buffers()`.
//!
//! 5. Create an `AtlasBuilder` with a suitable width (1024 or 2048 is usually fine) and call
//!    `AtlasBuilder::pack_glyph()` on each glyph you need to render. Then call
//!    `AtlasBuilder::create_atlas()` to upload the atlas buffer to the GPU.
//!
//! 6. Make a `CoverageBuffer` of an appropriate size (1024 or 2048 pixels on each side is
//!    typically reasonable).
//!
//! 7. Create an image to render the atlas to with `Rasterizer::device().create_image()`. The
//!    format should be `R8` and the buffer should be created read-write.
//!
//! 8. Draw the glyphs with `Rasterizer::draw_atlas()`.
//!
//! Don't forget to flush the queue (`Rasterizer::queue().flush()`) and/or perform appropriate
//! synchronization (`glMemoryBarrier()`) as necessary.
//!
//! ## Hardware requirements
//!
//! Pathfinder requires at least OpenGL 3.3 and either OpenGL 4.3 compute shader or OpenCL 1.2.
//! Intel GPUs in Sandy Bridge processors or later should be OK.

#![cfg_attr(test, feature(test))]

#[macro_use]
extern crate bitflags;
extern crate byteorder;
extern crate compute_shader;
extern crate euclid;
extern crate flate2;
extern crate gl;
#[cfg(test)]
extern crate memmap;
extern crate num_traits;
#[cfg(test)]
#[macro_use]
extern crate quickcheck;
#[cfg(test)]
extern crate test;

pub mod atlas;
pub mod charmap;
pub mod coverage;
pub mod error;
pub mod font;
pub mod hinting;
pub mod outline;
pub mod rasterizer;
pub mod shaper;
pub mod typesetter;

mod containers;
mod rect_packer;
mod tables;
mod util;

#[cfg(test)]
mod tests;

