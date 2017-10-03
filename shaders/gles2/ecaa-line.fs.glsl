// pathfinder/shaders/gles2/ecaa-line.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform bool uLowerPart;

varying vec4 vEndpoints;

void main() {
    // Unpack.
    vec2 center = gl_FragCoord.xy;
    vec2 p0 = vEndpoints.xy, p1 = vEndpoints.zw;

    // Set up Liang-Barsky clipping.
    vec4 pixelExtents = center.xxyy + vec4(-0.5, 0.5, -0.5, 0.5);
    vec4 p = (p1 - p0).xxyy, q = pixelExtents - p0.xxyy;

    // Use Liang-Barsky to clip to the left and right sides of this pixel.
    vec2 t = clamp(q.xy / p.xy, 0.0, 1.0);
    vec2 spanP0 = p0 + p.yw * t.x, spanP1 = p0 + p.yw * t.y;

    // Compute area.
    gl_FragColor = vec4(computeCoverage(p0, p1, spanP0, spanP1, t, pixelExtents, p, q, uLowerPart));
}
