#version 330

// pathfinder/shaders/blit.vs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

in vec2 aPosition;

out vec2 vTexCoord;

void main() {
    vTexCoord = aPosition;
    gl_Position = vec4(mix(aPosition, vec2(-1.0), vec2(1.0)), 0.0, 1.0);
}
