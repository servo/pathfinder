#version 330

// pathfinder/demo2/cover.fs.glsl
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform sampler2D uStencilTexture;

in vec2 vTexCoord;
in float vBackdrop;
in vec4 vColor;

out vec4 oFragColor;

void main() {
    float coverage = abs(texture(uStencilTexture, vTexCoord).r + vBackdrop);
    vec4 color = vec4(1.0, 0.0, 0.0, 1.0);
    //oFragColor = vec4(vColor.rgb, vColor.a * coverage);
    oFragColor = vec4(color.rgb, color.a * coverage);
}
