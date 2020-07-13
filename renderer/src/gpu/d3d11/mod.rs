// pathfinder/renderer/src/gpu/d3d11/mod.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A GPU compute-based renderer that uses functionality available in Direct3D 11.
//! 
//! This renderer supports OpenGL at least 4.3, OpenGL ES at least 3.1, and Metal of any version.

pub mod renderer;
pub mod shaders;
