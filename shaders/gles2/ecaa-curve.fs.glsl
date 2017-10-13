// pathfinder/shaders/gles2/ecaa-curve.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

varying vec4 vEndpoints;
varying vec2 vControlPoint;
varying float vWinding;

// Solve the equation:
//
//    x = p0x + t^2 * (p0x - 2*p1x + p2x) + t*(2*p1x - 2*p0x)
//
// We use the Citardauq Formula to avoid floating point precision issues.
float solveCurveT(float p0x, float p1x, float p2x, float x) {
    float a = p0x - 2.0 * p1x + p2x;
    float b = 2.0 * p1x - 2.0 * p0x;
    float c = p0x - x;
    return 2.0 * c / (-b - sqrt(b * b - 4.0 * a * c));
}

void main() {
    // Unpack.
    vec2 center = gl_FragCoord.xy;
    vec2 p0 = vEndpoints.xy, p1 = vEndpoints.zw;
    vec2 cp = vControlPoint;

    // Compute pixel extents.
    vec4 pixelExtents = center.xxyy + vec4(-0.5, 0.5, -0.5, 0.5);

    // Clip the curve to the left and right edges to create a line.
    //
    // TODO(pcwalton): Consider clipping to the bottom and top edges properly too. (I kind of doubt
    // it's worth it to do this, though, given that the maximum error doing it this way will always
    // be less than a pixel, and it saves a lot of time.)
    //
    // FIXME(pcwalton): Factor out shared terms to avoid computing them multiple times.
    vec2 t = vec2(p0.x < pixelExtents.x ? solveCurveT(p0.x, cp.x, p1.x, pixelExtents.x) : 0.0,
                  p1.x > pixelExtents.y ? solveCurveT(p0.x, cp.x, p1.x, pixelExtents.y) : 1.0);

    vec2 spanP0 = mix(mix(p0, cp, t.x), mix(cp, p1, t.x), t.x);
    vec2 spanP1 = mix(mix(p0, cp, t.y), mix(cp, p1, t.y), t.y);
    p0 = spanP0;
    p1 = spanP1;

    // Set up Liang-Barsky clipping.
    vec2 dp = p1 - p0;
    vec4 q = pixelExtents - p0.xxyy;
    t = clamp(q.xy / dp.xx, 0.0, 1.0);
    spanP0 = p0 + dp * t.x;
    spanP1 = p0 + dp * t.y;

    // Compute area.
    gl_FragColor = vec4(computeCoverage(p0,
                                        spanP0, spanP1,
                                        t,
                                        pixelExtents,
                                        dp, q,
                                        vWinding > 0.0));
}
