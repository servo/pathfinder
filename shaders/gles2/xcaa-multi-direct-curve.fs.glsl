// pathfinder/shaders/gles2/xcaa-multi-direct-curve.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// This shader implements the quadratic Loop-Blinn formulation.

precision highp float;

varying vec2 vPathID;
varying vec2 vTexCoord;
varying float vSign;

void main() {
    float side = vTexCoord.x * vTexCoord.x - vTexCoord.y;
    float alpha = float(sign(side) == sign(vSign));
    gl_FragColor = vec4(vPathID, 0.0, alpha);
}
