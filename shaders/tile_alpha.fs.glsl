#version 330

// pathfinder/shaders/tile_alpha.fs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform sampler2D uStencilTexture;
uniform sampler2D uPaintTexture;
uniform vec2 uPaintTextureSize;

in vec2 vColorTexCoord;
in vec2 vMaskTexCoord;
in vec4 vColor;

out vec4 oFragColor;

void main() {
    float coverage = texture(uStencilTexture, vMaskTexCoord).r;
    vec4 color = texture(uPaintTexture, vColorTexCoord);
    color.a *= coverage;
    color.rgb *= color.a;
    oFragColor = color;
}
