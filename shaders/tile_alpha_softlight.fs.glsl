#version 330

// pathfinder/shaders/tile_alpha_softlight.fs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// The soft light blend mode.

#extension GL_GOOGLE_include_directive : enable

precision highp float;

out vec4 oFragColor;

#include "tile_alpha_sample.inc.glsl"

void main() {
    vec4 srcRGBA = sampleSrcColor();
    vec4 destRGBA = sampleDestColor();

    // B(Cb, Cs) = Cb*(1 + (2*Cs - 1)*X)
    //   where X = if Cs <= 0.5  then 1 - Cb              else D - 1
    //     and D = if Cb <= 0.25 then (16*Cb - 12)*Cb + 4 else 1/sqrt(Cb)
    vec3 dest = destRGBA.rgb, src = srcRGBA.rgb;
    bvec3 destDark = lessThanEqual(dest, vec3(0.25)), srcDark = lessThanEqual(src, vec3(0.5));
    vec3 d = select3(destDark, (dest * 16.0 - 12.0) * dest + 4.0, inversesqrt(dest));
    vec3 x = select3(srcDark, vec3(1.0) - dest, d - 1.0);
    vec3 blended = dest * ((src * 2.0 - 1.0) * x + 1.0);

    oFragColor = blendColors(destRGBA, srcRGBA, blended);
}
