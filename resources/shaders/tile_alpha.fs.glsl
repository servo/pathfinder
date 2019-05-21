#version {{version}}

// pathfinder/demo/resources/shaders/mask_tile.fs.glsl
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
uniform sampler2D uPaintTexture;

in vec2 vTexCoord;
in vec2 vPaintTexCoord;
in float vBackdrop;

out vec4 oFragColor;

void main() {
    float coverage = abs(texture(uStencilTexture, vTexCoord).r + vBackdrop);
    vec4 color = texture(uPaintTexture, vPaintTexCoord);
    oFragColor = vec4(color.rgb, color.a * coverage);
}
