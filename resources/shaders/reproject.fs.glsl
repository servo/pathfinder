#version {{version}}

// pathfinder/resources/shaders/reproject.fs.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform mat4 uTexTransform;
uniform sampler2D uTexture;

in vec2 vTexCoord;

out vec4 oFragColor;

void main() {
    vec4 normTexCoord = uTexTransform * vec4(vTexCoord, 0.0, 1.0);
    oFragColor = texture(uTexture, (normTexCoord.xy / normTexCoord.w + 1.0) * 0.5);
}
