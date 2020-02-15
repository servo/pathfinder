#version 330

// pathfinder/shaders/mask_winding.fs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform sampler2D uMaskTexture;

in vec2 vMaskTexCoord;
in float vBackdrop;

out vec4 oFragColor;

void main() {
    oFragColor = vec4(abs(texture(uMaskTexture, vMaskTexCoord).r + vBackdrop));
}
