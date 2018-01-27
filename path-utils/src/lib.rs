// pathfinder/path-utils/src/lib.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Various utilities for manipulating Bézier curves.
//! 
//! Most of these should go upstream to Lyon at some point.

extern crate arrayvec;
extern crate euclid;
extern crate lyon_geom;
extern crate lyon_path;

pub mod cubic_to_quadratic;
pub mod normals;
pub mod segments;
pub mod stroke;
