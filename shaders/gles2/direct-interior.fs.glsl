// pathfinder/shaders/gles2/direct-interior.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Renders polygonal portions of a mesh.
//!
//! Typically, you will run this shader before running `direct-curve`.
//! Remember to enable the depth test with a `GREATER` depth function for optimal
//! performance.

precision highp float;

varying vec4 vColor;

void main() {
    gl_FragColor = vColor;
}
