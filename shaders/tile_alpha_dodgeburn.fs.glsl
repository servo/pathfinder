#version 330

// pathfinder/shaders/tile_alpha_dodgeburn.fs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Color dodge and color burn blend modes.

#extension GL_GOOGLE_include_directive : enable

precision highp float;

uniform int uBurn;

out vec4 oFragColor;

#include "tile_alpha_sample.inc.glsl"

void main() {
    vec4 srcRGBA = sampleSrcColor();
    vec4 destRGBA = sampleDestColor();

    vec3 dest = uBurn == 0 ? destRGBA.rgb : vec3(1.0) - destRGBA.rgb;
    vec3 src  = uBurn == 0 ? vec3(1.0) - srcRGBA.rgb : srcRGBA.rgb;

    bvec3 srcNonzero = notEqual(src, vec3(0.0));
    vec3 blended = min(vec3(srcNonzero.x ? dest.x / src.x : 1.0,
                            srcNonzero.y ? dest.y / src.y : 1.0,
                            srcNonzero.z ? dest.z / src.z : 1.0),
                       vec3(1.0));
    if (uBurn != 0)
        blended = vec3(1.0) - blended;

    oFragColor = blendColors(destRGBA, srcRGBA, blended);
}
