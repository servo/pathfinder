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

    // Discard if not edge.
    if (!isPartiallyCovered(p0, p1 - p0, center, vWinding))
        discard;
    gl_FragColor = vec4(1.0);
}
