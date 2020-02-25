#version 330

// pathfinder/shaders/tile_alpha_exclusion.fs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// The exclusion blend mode.

#extension GL_GOOGLE_include_directive : enable

precision highp float;

out vec4 oFragColor;

#include "tile_alpha_sample.inc.glsl"

void main() {
    vec4 srcRGBA = sampleSrcColor();
    vec4 destRGBA = sampleDestColor();

    vec3 dest = destRGBA.rgb, src = srcRGBA.rgb;
    vec3 blended = dest + src - dest * src * 2.0;

    oFragColor = blendColors(destRGBA, srcRGBA, blended);
}
