// pathfinder/shaders/gles2/mcaa-cover.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Performs the conservative coverage step for *mesh coverage antialiasing*
//! (MCAA).
//!
//! This shader expects to render to the red channel of a floating point color
//! buffer. Half precision floating point should be sufficient.
//!
//! Use this shader only when *both* of the following are true:
//!
//! 1. You are only rendering monochrome paths such as text. (Otherwise,
//!    consider `mcaa-multi`.)
//!
//! 2. Your transform is only a scale and/or translation, not a perspective,
//!    rotation, or skew. (Otherwise, consider the ECAA shaders.)

precision highp float;

varying vec2 vHorizontalExtents;

void main() {
    vec2 sides = gl_FragCoord.xx + vec2(-0.5, 0.5);
    vec2 clampedSides = clamp(vHorizontalExtents, sides.x, sides.y);
    gl_FragColor = vec4(vec3(clampedSides.y - clampedSides.x), 1.0);
}
