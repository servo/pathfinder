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
pub static DEFRINGING_KERNEL_CORE_GRAPHICS: DefringingKernel =
    DefringingKernel([0.033165660, 0.102074051, 0.221434336, 0.286651906]);
pub static DEFRINGING_KERNEL_FREETYPE: DefringingKernel =
    DefringingKernel([0.0, 0.031372549, 0.301960784, 0.337254902]);

/// Should match macOS 10.13 High Sierra.
pub static STEM_DARKENING_FACTORS: [f32; 2] = [0.0121, 0.0121 * 1.25];

/// Should match macOS 10.13 High Sierra.
pub const MAX_STEM_DARKENING_AMOUNT: [f32; 2] = [0.3, 0.3];

/// This value is a subjective cutoff. Above this ppem value, no stem darkening is performed.
pub const MAX_STEM_DARKENING_PIXELS_PER_EM: f32 = 72.0;
