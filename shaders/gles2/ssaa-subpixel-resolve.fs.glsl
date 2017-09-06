// pathfinder/shaders/gles2/ssaa-subpixel-resolve.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision mediump float;

uniform sampler2D uSource;
uniform ivec2 uSourceDimensions;

varying vec2 vTexCoord;

#define FILTER_0    (86.0 / 255.0)
#define FILTER_1    (77.0 / 255.0)
#define FILTER_2    (8.0  / 255.0)

float sampleSource(float deltaX) {
    return texture2D(uSource, vec2(vTexCoord.s + deltaX, vTexCoord.y)).r;
}

// https://www.freetype.org/freetype2/docs/reference/ft2-lcd_filtering.html
float lcdFilter(float shadeL2, float shadeL1, float shade0, float shadeR1, float shadeR2) {
    return FILTER_2 * shadeL2 +
        FILTER_1 * shadeL1 +
        FILTER_0 * shade0 +
        FILTER_1 * shadeR1 +
        FILTER_2 * shadeR2;
}

void main() {
    float onePixel = 1.0 / float(uSourceDimensions.x);

    float shade0 = sampleSource(0.0);
    vec3 shadeL = vec3(sampleSource(-1.0 * onePixel),
                       sampleSource(-2.0 * onePixel),
                       sampleSource(-3.0 * onePixel));
    vec3 shadeR = vec3(sampleSource(1.0 * onePixel),
                       sampleSource(2.0 * onePixel),
                       sampleSource(3.0 * onePixel));

    gl_FragColor = vec4(lcdFilter(shadeL.z, shadeL.y, shadeL.x, shade0,   shadeR.x),
                        lcdFilter(shadeL.y, shadeL.x, shade0,   shadeR.x, shadeR.y),
                        lcdFilter(shadeL.x, shade0,   shadeR.x, shadeR.y, shadeR.z),
                        1.0);
}
