// pathfinder/simd/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A minimal SIMD abstraction, usable outside of Pathfinder.

#[cfg(any(feature = "pf-no-simd", all(not(target_arch = "x86"), not(target_arch = "x86_64"))))]
pub use crate::scalar as default;
#[cfg(all(not(feature = "pf-no-simd"), any(target_arch = "x86", target_arch = "x86_64")))]
pub use crate::x86 as default;

pub mod scalar;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod x86;
mod extras;

#[cfg(test)]
mod test;
