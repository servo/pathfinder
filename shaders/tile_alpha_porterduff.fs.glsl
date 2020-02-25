#version 330

// pathfinder/shaders/tile_alpha_porterduff.fs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Porter-Duff blend modes not supported by the standard GPU blender.

#extension GL_GOOGLE_include_directive : enable

#define PORTER_DUFF_FACTOR_ZERO                 0
#define PORTER_DUFF_FACTOR_DEST_ALPHA           1
#define PORTER_DUFF_FACTOR_SRC_ALPHA            2
#define PORTER_DUFF_FACTOR_ONE_MINUS_DEST_ALPHA 3

precision highp float;

uniform int uDestFactor;
uniform int uSrcFactor;

out vec4 oFragColor;

#include "tile_alpha_sample.inc.glsl"

vec4 getFactor(int factor, vec4 destRGBA, vec4 srcRGBA) {
    if (factor == PORTER_DUFF_FACTOR_ZERO)
        return vec4(0.0);
    if (factor == PORTER_DUFF_FACTOR_DEST_ALPHA)
        return vec4(destRGBA.a);
    if (factor == PORTER_DUFF_FACTOR_SRC_ALPHA)
        return vec4(srcRGBA.a);
    return vec4(1.0 - destRGBA.a);
}

void main() {
    vec4 srcRGBA = sampleSrcColor();
    vec4 destRGBA = sampleDestColor();

    vec4 destFactor = getFactor(uDestFactor, destRGBA, srcRGBA);
    vec4 srcFactor = getFactor(uSrcFactor, destRGBA, srcRGBA);

    vec4 blended = destFactor * destRGBA * vec4(destRGBA.aaa, 1.0) +
        srcFactor * srcRGBA * vec4(srcRGBA.aaa, 1.0);
    oFragColor = blended;
}
