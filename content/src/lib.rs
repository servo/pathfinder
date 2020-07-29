// pathfinder/content/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Components of a vector scene, and various path utilities.

#![warn(missing_docs)]

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate log;

pub mod clip;
pub mod dash;
pub mod effects;
pub mod fill;
pub mod gradient;
pub mod orientation;
pub mod outline;
pub mod pattern;
pub mod render_target;
pub mod segment;
pub mod stroke;
pub mod transform;

mod dilation;
mod util;
