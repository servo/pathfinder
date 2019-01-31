// pathfinder/gl/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! An OpenGL backend for Pathfinder.
//!
//! It's not necessary to use this crate to render vector graphics with
//! Pathfinder; you can use the `pathfinder_renderer` crate and do the GPU
//! rendering yourself using the API or engine of your choice. This crate can
//! be useful for simple use cases, however.

#[macro_use]
extern crate serde_derive;

pub mod debug;
pub mod device;
pub mod renderer;
