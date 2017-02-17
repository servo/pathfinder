// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#version 410

layout(quads) in;

// The size of the atlas in pixels.
uniform uvec2 uAtlasSize;

// The starting point of the segment.
in vec2 vpP0[];
// The first control point, if this is a curve. If this is a line, this value must be ignored.
in vec2 vpP1[];
// The second control point, if this is a cubic curve. If this is a quadratic curve or a line, this
// is equal to `vpP1`.
in vec2 vpP2[];
// The endpoint of this segment.
in vec2 vpP3[];
// The tessellation level.
//
// This is passed along explicitly instead of having the TES read it from `gl_TessLevelInner` in
// order to work around an Apple bug in the Radeon driver.
in float vpTessLevel[];

// The starting point of the segment.
flat out vec2 vP0;
// The endpoint of this segment.
flat out vec2 vP1;
// 1.0 if this segment runs left to right; -1.0 otherwise.
flat out float vDirection;
// The slope of this line.
flat out float vSlope;
// Minimum and maximum vertical extents, unrounded.
flat out vec2 vYMinMax;

void main() {
    // Read in curve points.
    vec2 cP0 = vpP0[0], cP1 = vpP1[0], cP2 = vpP2[0], cP3 = vpP3[0];

    // Work out how many lines made up this segment, which line we're working on, and which
    // endpoint of that line we're looking at.
    uint tessPointCount = uint(vpTessLevel[0] + 1.0f);
    uint tessIndex = uint(round(gl_TessCoord.x * float(tessPointCount - 1)));
    uint lineCount = tessPointCount / 2, lineIndex = tessIndex / 2, endpoint = tessIndex % 2;

    // Compute our endpoints (trivial if this is part of a line, less trivial if this is part of a
    // curve).
    vec2 p0, p1;
    if (lineCount == 1) {
        p0 = cP0;
        p1 = cP3;
    } else {
        float t0 = float(lineIndex + 0) / float(lineCount);
        float t1 = float(lineIndex + 1) / float(lineCount);

        // These lerps are needed both for quadratic and cubic Béziers.
        vec2 pP0P1T0 = mix(cP0, cP1, t0), pP0P1T1 = mix(cP0, cP1, t1);
        vec2 pP2P3T0 = mix(cP2, cP3, t0), pP2P3T1 = mix(cP2, cP3, t1);

        if (cP1 == cP2) {
            // Quadratic Bézier.
            p0 = mix(pP0P1T0, pP2P3T0, t0);
            p1 = mix(pP0P1T1, pP2P3T1, t1);
        } else {
            // Cubic Bézier.
            vec2 pP1P2T0 = mix(cP1, cP2, t0), pP1P2T1 = mix(cP1, cP2, t1);
            p0 = mix(mix(pP0P1T0, pP1P2T0, t0), mix(pP1P2T0, pP2P3T0, t0), t0);
            p1 = mix(mix(pP0P1T1, pP1P2T1, t1), mix(pP1P2T1, pP2P3T1, t1), t1);
        }
    }

    // Compute direction. Flip the two points around so that p0 is on the left and p1 is on the
    // right if necessary.
    float direction;
    if (p0.x < p1.x) {
        direction = 1.0f;
    } else {
        direction = -1.0f;
        vec2 tmp = p0;
        p0 = p1;
        p1 = tmp;
    }

    // Forward points and direction onto the fragment shader.
    vP0 = p0;
    vP1 = p1;
    vDirection = direction;

    // Compute Y extents and slope.
    vSlope = (p1.y - p0.y) / (p1.x - p0.x);
    vYMinMax = p0.y <= p1.y ? vec2(p0.y, p1.y) : vec2(p1.y, p0.y);

    // Compute our final position in atlas space, rounded out to the next pixel.
    float x = endpoint == 0 ? floor(p0.x) : ceil(p1.x);
    float y = gl_TessCoord.y == 0.0f ? floor(vYMinMax.x) : ceil(vYMinMax.y) + 1.0f;

    // Convert atlas space to device space.
    gl_Position = vec4(vec2(x, y) / vec2(uAtlasSize) * 2.0f - 1.0f, 0.0f, 1.0f);
}

