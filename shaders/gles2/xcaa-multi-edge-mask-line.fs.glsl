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
    vec2 pixelCenter = gl_FragCoord.xy;
    vec2 p0 = vEndpoints.xy, p1 = vEndpoints.zw;

    // Clip to left and right pixel boundaries.
    vec2 dP = p1 - p0;
    vec4 p0DPX = clipLineToPixelColumn(p0, dP, pixelCenter.x);

    // Discard if not edge.
    if (!isPartiallyCovered(p0DPX.xy, p0DPX.zw, pixelCenter.y))
        discard;
    gl_FragColor = vec4(1.0);
}
