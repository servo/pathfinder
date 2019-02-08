// pathfinder/renderer/src/post.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Functionality related to postprocessing effects.
//!
//! Since these effects run on GPU as fragment shaders, this contains no
//! implementations, just shared declarations.

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DefringingKernel(pub [f32; 4]);

/// This intentionally does not precisely match what Core Graphics does (a
/// Lanczos function), because we don't want any ringing artefacts.
pub static DEFRINGING_KERNEL_CORE_GRAPHICS: DefringingKernel = DefringingKernel([
    0.033165660, 0.102074051, 0.221434336, 0.286651906
]);
pub static DEFRINGING_KERNEL_FREETYPE: DefringingKernel = DefringingKernel([
    0.0, 0.031372549, 0.301960784, 0.337254902
]);

