#version 330

// pathfinder/shaders/reproject.fs.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
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

uniform mat4 uOldTransform;
uniform sampler2D uTexture;

in vec2 vTexCoord;

out vec4 oFragColor;

void main() {
    vec4 normTexCoord = uOldTransform * vec4(vTexCoord, 0.0, 1.0);
    vec2 texCoord = ((normTexCoord.xy / normTexCoord.w) + 1.0) * 0.5;
    oFragColor = texture(uTexture, texCoord);
}
