#version 330

// pathfinder/shaders/tile_clip_copy.vs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

#ifdef GL_ES
precision highp sampler2D;
#endif

uniform vec2 uFramebufferSize;

in ivec2 aTileOffset;
in int aTileIndex;

out vec2 vTexCoord;

void main() {
    vec2 position = vec2(ivec2(aTileIndex % 256, aTileIndex / 256) + aTileOffset);
    position *= vec2(16.0, 4.0) / uFramebufferSize;

    vTexCoord = position;

    if (aTileIndex < 0)
        position = vec2(0.0);

#ifdef PF_ORIGIN_UPPER_LEFT
    position.y = 1.0 - position.y;
#endif
    gl_Position = vec4(mix(vec2(-1.0), vec2(1.0), position), 0.0, 1.0);
}
