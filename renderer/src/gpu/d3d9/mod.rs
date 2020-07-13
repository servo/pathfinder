// pathfinder/renderer/src/gpu/d3d9/mod.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A hybrid CPU-GPU renderer that only relies on functionality available in Direct3D 9.
//! 
//! This renderer supports OpenGL at least 3.0, OpenGL ES at least 3.0, Metal of any version, and
//! WebGL at least 2.0.

pub mod renderer;
pub mod shaders;
