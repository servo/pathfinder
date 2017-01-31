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

#define CURVE_THRESHOLD         0.333f
#define CURVE_TOLERANCE         3.0f

layout(vertices = 1) out;

// The vertex ID, passed into this shader.
flat in int vVertexID[];

// The starting point of the segment.
patch out vec2 vpP0;
// The control point, if this is a curve. If this is a line, this value must be ignored.
patch out vec2 vpP1;
// The endpoint of this segment.
patch out vec2 vpP2;
// 1.0 if this segment runs left to right; -1.0 otherwise.
patch out float vpDirection;

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

    // Divide into lines.
    float lineCount = 1.0f;
    if (vVertexID[1] > 0) {
        // Quadratic curve.
        vec2 dev = p0 - 2.0f * p1 + p2;
        float devSq = dot(dev, dev);
        if (devSq >= CURVE_THRESHOLD) {
            // Inverse square root is likely no slower and may be faster than regular square root
            // (e.g. on x86).
            lineCount += floor(inversesqrt(inversesqrt(CURVE_TOLERANCE * devSq)));
        }
    }

    // Tessellate into lines. This is subtle, so a diagram may help.
    //
    // Suppose we decided to divide this curve up into 4 lines. Then our abstract tessellated patch
    // space will look like this:
    //
    //    x₀ x₁ x₂ x₃ x₄ x₅ x₆ x₇
    //    ┌──┬──┬──┬──┬──┬──┬──┐
    //    │▒▒│  │▒▒│  │▒▒│  │▒▒│
    //    │▒▒│  │▒▒│  │▒▒│  │▒▒│
    //    └──┴──┴──┴──┴──┴──┴──┘
    //
    // The shaded areas are the only areas that will actually be drawn. They might look like this:
    //
    //                x₅
    //                x₆      x₇
    //          x₃    ┌───────┐
    //          x₄    │▒▒▒▒▒▒▒│
    //       x₁ ┌─────┼───────┘
    //       x₂ │▒▒▒▒▒│
    //       ┌──┼─────┘
    //       │▒▒│
    //       │▒▒│
    //    x₀ │▒▒│
    //    ┌──┼──┘
    //    │▒▒│
    //    │▒▒│
    //    └──┘
    //
    // In this way, the unshaded areas become zero-size and are discarded by the rasterizer.
    //
    // Note that, in reality, it will often be the case that the quads overlap vertically by one
    // pixel in the horizontal direction. In fact, this will occur whenever a line segment endpoint
    // does not precisely straddle a pixel boundary. However, observe that we can guarantee that
    // x₂ ≤ x₁, x₄ ≤ x₃, and so on, because there is never any horizontal space between endpoints.
    // This means that all triangles inside the unshaded areas are guaranteed to be wound in the
    // opposite direction from those inside the shaded areas. Because the OpenGL spec guarantees
    // that, by default, all tessellated triangles are wound counterclockwise in abstract patch
    // space, the triangles within the unshaded areas must be wound clockwise and are therefore
    // candidates for backface culling. Backface culling is always enabled when running Pathfinder,
    // so we're in the clear: the rasterizer will always discard the unshaded areas and render only
    // the shaded ones.

    float tessLevel = min(p0.x == p2.x ? 0.0f : (lineCount * 2.0f - 1.0f), 31.0f);
    gl_TessLevelInner[0] = tessLevel;
    gl_TessLevelInner[1] = 1.0f;
    gl_TessLevelOuter[0] = 1.0f;
    gl_TessLevelOuter[1] = tessLevel;
    gl_TessLevelOuter[2] = 1.0f;
    gl_TessLevelOuter[3] = tessLevel;

    // NB: These per-patch outputs must be assigned in this order, or Apple's compiler will
    // miscompile us.
    vpP0 = p0;
    vpP1 = p1;
    vpP2 = p2;
    vpDirection = direction;
}

