#version 300 es

// pathfinder/demo2/stencil.fs.glsl
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

out vec4 oFragColor;

void main() {
    float coverage = gl_FrontFacing ? 1.0 : -1.0;
    oFragColor = vec4(coverage);
}
