#version 330

// pathfinder/shaders/reproject.vs.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform mat4 uNewTransform;

in ivec2 aPosition;

out vec2 vTexCoord;

void main() {
#ifdef PF_ORIGIN_UPPER_LEFT
    vTexCoord = vec2(aPosition.x, 1.0 - aPosition.y);
#else
    vTexCoord = vec2(aPosition);
#endif

    gl_Position = uNewTransform * vec4(ivec4(aPosition, 0, 1));
}
