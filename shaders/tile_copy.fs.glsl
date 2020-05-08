#version 330

// pathfinder/shaders/tile_copy.fs.glsl
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
uniform sampler2D uSrc;

out vec4 oFragColor;

void main() {
    vec2 texCoord = gl_FragCoord.xy / uFramebufferSize;
    oFragColor = texture(uSrc, texCoord);
}
