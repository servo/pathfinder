#version 330

// pathfinder/shaders/mask_winding.vs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

in vec2 aPosition;
in vec2 aMaskTexCoord;
in int aBackdrop;

out vec2 vMaskTexCoord;
out float vBackdrop;

void main() {
    vMaskTexCoord = aMaskTexCoord;
    vBackdrop = float(aBackdrop);
    gl_Position = vec4(mix(vec2(-1.0, -1.0), vec2(1.0, 1.0), aPosition), 0.0, 1.0);
}
