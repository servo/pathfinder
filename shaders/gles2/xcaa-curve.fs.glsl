// pathfinder/shaders/gles2/xcaa-curve.fs.glsl
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

void main() {
    // Unpack.
    vec2 center = gl_FragCoord.xy;
    vec2 p0 = vEndpoints.xy, p1 = vEndpoints.zw;
    vec2 cp = vControlPoint;

    // Compute pixel extents.
    vec2 pixelExtents = center.xx + vec2(-0.5, 0.5);

    // Clip the curve to the left and right edges to create a line.
    //
    // TODO(pcwalton): Consider clipping to the bottom and top edges properly too. (I kind of doubt
    // it's worth it to do this, though, given that the maximum error doing it this way will always
    // be less than a pixel, and it saves a lot of time.)
    vec2 t = solveCurveT(p0.x, cp.x, p1.x, pixelExtents);

    // Handle endpoints properly. These tests are negated to handle NaNs.
    if (!(p0.x < pixelExtents.x))
        t.x = 0.0;
    if (!(p1.x > pixelExtents.y))
        t.y = 1.0;

    vec2 clippedP0 = mix(mix(p0, cp, t.x), mix(cp, p1, t.x), t.x);
    vec2 clippedP1 = mix(mix(p0, cp, t.y), mix(cp, p1, t.y), t.y);

    // Compute area.
    gl_FragColor = vec4(computeCoverage(clippedP0, clippedP1 - clippedP0, center, vWinding));
}
