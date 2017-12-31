// pathfinder/shaders/gles2/xcaa-curve.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Performs analytic *coverage antialiasing* (XCAA) in order to render curves.
//!
//! Given endpoints P0 and P2 and control point P1, this shader expects that
//! P0.x <= P1.x <= P2.x.

precision highp float;

varying vec4 vEndpoints;
varying vec2 vControlPoint;
varying float vWinding;

void main() {
    // Unpack.
    vec2 pixelCenter = gl_FragCoord.xy;
    vec2 p0 = vEndpoints.xy, p1 = vEndpoints.zw;
    vec2 cp = vControlPoint;

    // Compute pixel extents.
    vec2 pixelColumnBounds = pixelCenter.xx + vec2(-0.5, 0.5);

    // Clip the curve to the left and right edges to create a line.
    vec2 t = solveCurveT(p0.x, cp.x, p1.x, pixelColumnBounds);

    // Handle endpoints properly. These tests are negated to handle NaNs.
    if (!(p0.x < pixelColumnBounds.x))
        t.x = 0.0;
    if (!(p1.x > pixelColumnBounds.y))
        t.y = 1.0;

    vec2 clippedP0 = mix(mix(p0, cp, t.x), mix(cp, p1, t.x), t.x);
    vec2 clippedDP = mix(mix(p0, cp, t.y), mix(cp, p1, t.y), t.y) - clippedP0;

    // Compute area.
    gl_FragColor = vec4(computeCoverage(clippedP0, clippedDP, pixelCenter.y, vWinding));
}
