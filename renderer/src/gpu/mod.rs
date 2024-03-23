// pathfinder/renderer/src/gpu/mod.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The GPU renderer for Pathfinder 3.

#[cfg(feature="d3d9")]
pub mod d3d9;
#[cfg(feature="d3d11")]
pub mod d3d11;
#[cfg(feature="debug")]
pub mod debug;
pub mod options;
pub mod perf;
pub mod renderer;

pub(crate) mod blend;
pub(crate) mod shaders;
