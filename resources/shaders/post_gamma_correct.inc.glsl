// pathfinder/resources/shaders/post_gamma_correct.inc.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// The lookup table for gamma correction, in the same format WebRender
// expects.
uniform sampler2D uGammaLUT;

float gammaCorrectChannel(float bgColor, float fgColor) {
    return texture(uGammaLUT, vec2(fgColor, 1.0 - bgColor)).r;
}

// `fgColor` is in linear space.
vec3 gammaCorrect(vec3 bgColor, vec3 fgColor) {
    return vec3(gammaCorrectChannel(bgColor.r, fgColor.r),
                gammaCorrectChannel(bgColor.g, fgColor.g),
                gammaCorrectChannel(bgColor.b, fgColor.b));
}
