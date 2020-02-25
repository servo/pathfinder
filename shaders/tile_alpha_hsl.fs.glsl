#version 330

// pathfinder/shaders/tile_alpha_hsl.fs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#extension GL_GOOGLE_include_directive : enable

precision highp float;

uniform ivec3 uBlendHSL;

out vec4 oFragColor;

#include "tile_alpha_sample.inc.glsl"

#define PI_2        6.283185307179586
#define DEG_30_INV  1.9098593171027443
#define DEG_60      1.0471975511965976

#define BLEND_TERM_DEST 0
#define BLEND_TERM_SRC  1

// https://en.wikipedia.org/wiki/HSL_and_HSV#HSL_to_RGB_alternative
vec3 convertHSLToRGB(vec3 hsl) {
    float a = hsl.y * min(hsl.z, 1.0 - hsl.z);
    vec3 ks = mod(vec3(0.0, 8.0, 4.0) + vec3(hsl.x * DEG_30_INV), 12.0);
    return hsl.zzz - clamp(min(ks - vec3(3.0), vec3(9.0) - ks), -1.0, 1.0) * a;
}

// https://en.wikipedia.org/wiki/HSL_and_HSV#From_RGB
vec3 convertRGBToHSL(vec3 rgb) {
    float v = max((rgb.x, rgb.y), rgb.z);
    float c = v - min((rgb.x, rgb.y), rgb.z);
    float l = v - 0.5 * c;

    vec3 tmp = vec3(0.0);
    bvec3 is_v = equal(rgb, vec3(v));
    if (is_v.r)
        tmp = vec3(0.0, rgb.gb);
    else if (is_v.g)
        tmp = vec3(2.0, rgb.br);
    else if (is_v.b)
        tmp = vec3(4.0, rgb.rg);
    float h = DEG_60 * (tmp.x + (tmp.y - tmp.z) / c);

    float s = 0.0;
    if (l > 0.0 && l < 1.0)
        s = (v - l) / min(l, 1.0 - l);

    return vec3(h, s, l);
}

void main() {
    vec4 srcRGBA = sampleSrcColor();
    vec4 destRGBA = sampleDestColor();

    vec3 destHSL = convertRGBToHSL(destRGBA.rgb);
    vec3 srcHSL = convertRGBToHSL(srcRGBA.rgb);
    vec3 blendedHSL = select3(equal(uBlendHSL, ivec3(BLEND_TERM_DEST)), destHSL, srcHSL);
    vec3 blendedRGB = convertHSLToRGB(blendedHSL);

    oFragColor = blendColors(destRGBA, srcRGBA, blendedRGB);
}
