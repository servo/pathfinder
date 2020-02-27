#version 330

// pathfinder/shaders/filter_blur.fs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// TODO(pcwalton): This could be significantly optimized by operating on a
// sparse per-tile basis.

// The technique here is "Incremental Computation of the Gaussian", GPU Gems 3, chapter 40:
// https://developer.nvidia.com/gpugems/gpugems3/part-vi-gpu-computing/chapter-40-incremental-computation-gaussian
//
// It's the same technique WebRender uses.

#extension GL_GOOGLE_include_directive : enable

precision highp float;

#define SQRT_PI_2_INV   2.5066282746310002

uniform sampler2D uSrc;
uniform vec2 uSrcOffsetScale;
uniform vec3 uInitialGaussCoeff;
uniform int uSupport;

in vec2 vTexCoord;

out vec4 oFragColor;

void main() {
    // Set up our incremental calculation.
    vec3 gaussCoeff = uInitialGaussCoeff;
    float gaussSum = gaussCoeff.x;
    vec4 color = texture(uSrc, vTexCoord) * gaussCoeff.x;
    gaussCoeff.xy *= gaussCoeff.yz;

    // This is a common trick that lets us use the texture filtering hardware to evaluate two
    // texels at a time. The basic principle is that, if c0 and c1 are colors of adjacent texels
    // and k0 and k1 are arbitrary factors, the formula `k0 * c0 + k1 * c1` is equivalent to
    // `(k0 + k1) * lerp(c0, c1, k1 / (k0 + k1))`. Linear interpolation, as performed by the
    // texturing hardware when sampling adjacent pixels in one direction, evaluates
    // `lerp(c0, c1, t)` where t is the offset from the texel with color `c0`. To evaluate the
    // formula `k0 * c0 + k1 * c1`, therefore, we can use the texture hardware to perform linear
    // interpolation with `t = k1 / (k0 + k1)`.
    for (int i = 1; i <= uSupport; i += 2) {
        float gaussPartialSum = gaussCoeff.x;
        gaussCoeff.xy *= gaussCoeff.yz;
        gaussPartialSum += gaussCoeff.x;

        vec2 srcOffset = uSrcOffsetScale * (float(i) + gaussCoeff.x / gaussPartialSum);
        color += (texture(uSrc, vTexCoord - srcOffset) + texture(uSrc, vTexCoord + srcOffset)) *
            gaussPartialSum;

        gaussSum += 2.0 * gaussPartialSum;
        gaussCoeff.xy *= gaussCoeff.yz;
    }

    // Finish.
    color /= gaussSum;
    color.rgb *= color.a;
    oFragColor = color;
}

