// pathfinder/content/src/render_target.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Raster images that vector graphics can be rendered to and later used as a pattern.

/// Identifies a drawing surface for vector graphics that can be later used as a pattern.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RenderTargetId {
    /// The ID of the scene that this render target ID belongs to.
    pub scene: u32,
    /// The ID of the render target within this scene.
    pub render_target: u32,
}
