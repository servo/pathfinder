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
patch in vec2 vpP0;
// The control point, if this is a curve. If this is a line, this value must be ignored.
patch in vec2 vpP1;
// The endpoint of this segment.
patch in vec2 vpP2;
// x: 1.0 if this segment runs left to right; -1.0 otherwise.
// y: The tessellation level.
//
// This is packed together into a single vec2 to work around an Apple Intel driver bug whereby
// patch outputs beyond the first 4 are forced to 0.
//
// And in case you're wondering why the tessellation level is passed along in a patch out instead
// of having the TES read it directly, that's another Apple bug workaround, this time in the Radeon
// driver.
patch in vec2 vpDirectionTessLevel;

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
    // Work out how many lines made up this segment, which line we're working on, and which
    // endpoint of that line we're looking at.
    uint tessPointCount = uint(vpDirectionTessLevel.y + 1.0f);
    uint tessIndex = uint(round(gl_TessCoord.x * float(tessPointCount - 1)));
    uint lineCount = tessPointCount / 2, lineIndex = tessIndex / 2, endpoint = tessIndex % 2;

    // Compute our endpoints (trivial if this is part of a line, less trivial if this is part of a
    // curve).
    if (lineCount == 1) {
        vP0 = vpP0;
        vP1 = vpP2;
    } else {
        float t0 = float(lineIndex + 0) / float(lineCount);
        float t1 = float(lineIndex + 1) / float(lineCount);
        vP0 = mix(mix(vpP0, vpP1, t0), mix(vpP1, vpP2, t0), t0);
        vP1 = mix(mix(vpP0, vpP1, t1), mix(vpP1, vpP2, t1), t1);
    }

    // Compute Y extents and slope.
    vYMinMax = vP0.y <= vP1.y ? vec2(vP0.y, vP1.y) : vec2(vP1.y, vP0.y);
    vSlope = (vP1.y - vP0.y) / (vP1.x - vP0.x);

    // Forward direction onto the fragment shader.
    vDirection = vpDirectionTessLevel.x;

    // Compute our final position in atlas space, rounded out to the next pixel.
    float x = endpoint == 0 ? floor(vP0.x) : ceil(vP1.x);
    float y = gl_TessCoord.y == 0.0f ? floor(vYMinMax.x) : ceil(vYMinMax.y) + 1.0f;

    // Convert atlas space to device space.
    gl_Position = vec4(vec2(x, y) / vec2(uAtlasSize) * 2.0f - 1.0f, 0.0f, 1.0f);
}

