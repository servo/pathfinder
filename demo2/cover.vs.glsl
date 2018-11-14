#version 300 es

// pathfinder/demo2/cover.vs.glsl
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform vec2 uFramebufferSize;
uniform vec2 uTileSize;

in vec2 aTessCoord;
in vec2 aTileOrigin;

void main() {
    vec2 position = aTileOrigin + uTileSize * aTessCoord;
    gl_Position = vec4(position / uFramebufferSize * 2.0 - 1.0, 0.0, 1.0);
}
