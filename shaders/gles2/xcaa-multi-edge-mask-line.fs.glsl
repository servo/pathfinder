// pathfinder/shaders/gles2/xcaa-multi-edge-mask-line.vs.glsl
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
varying float vWinding;

void main() {
    // Unpack.
    vec2 center = gl_FragCoord.xy;
    vec2 p0 = vEndpoints.xy, p1 = vEndpoints.zw;

    // Set up Liang-Barsky clipping.
    vec4 pixelExtents = center.xxyy + vec4(-0.5, 0.5, -0.5, 0.5);
    vec2 dp = p1 - p0;
    vec4 q = pixelExtents - p0.xxyy;

    // Use Liang-Barsky to clip to the left and right sides of this pixel.
    vec2 t = clamp(q.xy / dp.xx, 0.0, 1.0);
    vec2 spanP0 = p0 + dp * t.x, spanP1 = p0 + dp * t.y;

    // Discard if not edge.
    if (!isPartiallyCovered(p0, dp, center, vWinding))
        discard;
    gl_FragColor = vec4(1.0);
}
