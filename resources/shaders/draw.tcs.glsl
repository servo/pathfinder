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

// These outputs really should be patch outs, but that causes problems in Apple drivers.

// The starting point of the segment.
out vec2 vpP0[];
// The first control point, if this is a curve. If this is a line, this value must be ignored.
out vec2 vpP1[];
// The second control point, if this is a curve. If this is a line, this value must be ignored.
// If this curve is quadratic, this will be the same as `vpP1`.
out vec2 vpP2[];
// The endpoint of this segment.
out vec2 vpP3[];
// The tessellation level.
//
// This is passed along explicitly instead of having the TES read it from `gl_TessLevelInner` in
// order to work around an Apple bug in the Radeon driver.
out float vpTessLevel[];

void main() {
    vec2 p0 = gl_in[0].gl_Position.xy;
    vec2 p1 = gl_in[1].gl_Position.xy;
    vec2 p2 = gl_in[2].gl_Position.xy;
    vec2 p3 = gl_in[3].gl_Position.xy;

    // Divide into lines.
    float lineCount = 1.0f;
    if (vVertexID[1] > 0) {
        // A curve.
        //
        // FIXME(pcwalton): Is this formula good for cubic curves?
        vec2 dev = p0 - 2.0f * mix(p1, p2, 0.5) + p3;
        float devSq = dot(dev, dev);
        if (devSq >= CURVE_THRESHOLD) {
            // Inverse square root is likely no slower and may be faster than regular square
            // root
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

    float tessLevel = min(p0.x == p3.x ? 0.0f : (lineCount * 2.0f - 1.0f), 31.0f);
    gl_TessLevelInner[0] = tessLevel;
    gl_TessLevelInner[1] = 1.0f;
    gl_TessLevelOuter[0] = 1.0f;
    gl_TessLevelOuter[1] = tessLevel;
    gl_TessLevelOuter[2] = 1.0f;
    gl_TessLevelOuter[3] = tessLevel;

    // NB: These per-patch outputs must be assigned in this order, or Apple's compiler will
    // miscompile us.
    vpP0[gl_InvocationID] = p0;
    vpP1[gl_InvocationID] = p1;
    vpP2[gl_InvocationID] = p2;
    vpP3[gl_InvocationID] = p3;
    vpTessLevel[gl_InvocationID] = tessLevel;
}

