// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// A geometry shader fallback when tessellation is not available. This is *not* linked into the
// program by default.
//
// This will probably not perform well, but it's useful for testing, since llvmpipe does not
// support tessellation as of Jan. 2017.
//
// To use this shader, set `RasterizerOptions::force_geometry_shader` to true or set the
// environment variable `PATHFINDER_FORCE_GEOMETRY_SHADER` to 1.

#version 410

#define CURVE_THRESHOLD         0.333f
#define CURVE_TOLERANCE         3.0f

#define PIXELS_TO_DEVICE(x, y)  (vec2((x), (y)) / vec2(uAtlasSize) * 2.0f - 1.0f)

#define SET_VARYINGS(primID) \
    vP0 = lP0; \
    vP1 = lP1; \
    vDirection = direction; \
    vSlope = slope; \
    vYMinMax = yMinMax; \
    gl_PrimitiveID = gl_PrimitiveIDIn + (primID)

layout(triangles) in;
layout(triangle_strip, max_vertices = 256) out;

// The size of the atlas in pixels.
uniform uvec2 uAtlasSize;

// The vertex ID, passed into this shader.
flat in uint vVertexID[];

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
    vec2 p0 = gl_in[0].gl_Position.xy;
    vec2 p1 = gl_in[1].gl_Position.xy;
    vec2 p2 = gl_in[2].gl_Position.xy;

    // Compute direction. Flip around if necessary so that p0 is to the left of p2.
    float direction;
    if (p0.x < p2.x) {
        direction = 1.0f;
    } else {
        direction = -1.0f;
        vec2 tmp = p0;
        p0 = p2;
        p2 = tmp;
    }

    // Determine how many lines to divide into.
    uint lineCount = 1;
    if (vVertexID[1] > 0) {
        // Quadratic curve.
        vec2 dev = p0 - 2.0f * p1 + p2;
        float devSq = dot(dev, dev);
        if (devSq >= CURVE_THRESHOLD) {
            // Inverse square root is likely no slower and may be faster than regular square root
            // (e.g. on x86).
            lineCount += uint(floor(inversesqrt(inversesqrt(CURVE_TOLERANCE * devSq))));
        }
    }

    // Divide into lines.
    for (uint lineIndex = 0; lineIndex < lineCount; lineIndex++) {
        // Compute our endpoints (trivial if this is part of a line, less trivial if this is part
        // of a curve).
        vec2 lP0, lP1;
        if (lineCount == 1) {
            lP0 = p0;
            lP1 = p2;
        } else {
            float t0 = float(lineIndex + 0) / float(lineCount);
            float t1 = float(lineIndex + 1) / float(lineCount);
            lP0 = mix(mix(p0, p1, t0), mix(p1, p2, t0), t0);
            lP1 = mix(mix(p0, p1, t1), mix(p1, p2, t1), t1);
        }

        // Compute Y extents and slope.
        vec2 yMinMax = lP0.y <= lP1.y ? vec2(lP0.y, lP1.y) : vec2(lP1.y, lP0.y);
        float slope = (lP1.y - lP0.y) / (lP1.x - lP0.x);

        // Convert atlas space to device space.
        vec2 pTL = PIXELS_TO_DEVICE(floor(lP0.x), floor(yMinMax.x));
        vec2 pBR = PIXELS_TO_DEVICE(ceil(lP1.x), ceil(yMinMax.y) + 1.0f);
        vec2 pTR = vec2(pBR.x, pTL.y);
        vec2 pBL = vec2(pTL.x, pBR.y);

        // Assign primitive IDs.
        int primID0 = int(lineIndex) * 2, primID1 = int(lineIndex) * 2 + 1;

        // Emit vertices.
        SET_VARYINGS(primID0);
        gl_Position = vec4(pTL, 0.0f, 1.0f);
        EmitVertex();
        SET_VARYINGS(primID0);
        gl_Position = vec4(pTR, 0.0f, 1.0f);
        EmitVertex();
        SET_VARYINGS(primID0);
        gl_Position = vec4(pBL, 0.0f, 1.0f);
        EmitVertex();
        EndPrimitive();
        SET_VARYINGS(primID1);
        gl_Position = vec4(pBR, 0.0f, 1.0f);
        EmitVertex();
        SET_VARYINGS(primID1);
        gl_Position = vec4(pBL, 0.0f, 1.0f);
        EmitVertex();
        SET_VARYINGS(primID1);
        gl_Position = vec4(pTR, 0.0f, 1.0f);
        EmitVertex();
        EndPrimitive();
    }
}

