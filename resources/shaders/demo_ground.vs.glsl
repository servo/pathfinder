#version {{version}}

// pathfinder/demo/resources/shaders/demo_ground.vs.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform mat4 uTransform;
uniform int uGridlineCount;

in vec2 aPosition;

out vec2 vTexCoord;

void main() {
    vTexCoord = aPosition * float(uGridlineCount);
    gl_Position = uTransform * vec4(aPosition.x, 0.0, aPosition.y, 1.0);
}
