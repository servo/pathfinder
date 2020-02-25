#version 330

// pathfinder/shaders/tile_alpha_overlay.fs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Multiply, screen, overlay, and hard light filters.

#extension GL_GOOGLE_include_directive : enable

#define OVERLAY_BLEND_MODE_MULTIPLY     0
#define OVERLAY_BLEND_MODE_SCREEN       1
#define OVERLAY_BLEND_MODE_HARD_LIGHT   2
#define OVERLAY_BLEND_MODE_OVERLAY      3

precision highp float;

uniform int uBlendMode;

out vec4 oFragColor;

#include "tile_alpha_sample.inc.glsl"

void main() {
    vec4 srcRGBA = sampleSrcColor();
    vec4 destRGBA = sampleDestColor();

    bool reversed = uBlendMode == OVERLAY_BLEND_MODE_OVERLAY;
    vec3 src  = reversed ? srcRGBA.rgb  : destRGBA.rgb;
    vec3 dest = reversed ? destRGBA.rgb : srcRGBA.rgb;

    vec3 multiply = src * dest;
    vec3 blended;
    if (uBlendMode == OVERLAY_BLEND_MODE_MULTIPLY) {
        blended = multiply;
    } else {
        vec3 screen = dest + src - multiply;
        if (uBlendMode == OVERLAY_BLEND_MODE_SCREEN)
            blended = screen;
        else
            blended = select3(lessThanEqual(src, vec3(0.5)), multiply, screen * 2.0 - 1.0);
    }

    oFragColor = blendColors(destRGBA, srcRGBA, blended);
}
