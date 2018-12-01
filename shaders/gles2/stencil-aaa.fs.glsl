// pathfinder/shaders/gles2/stencil-aaa.fs.glsl
//
// Copyright (c) 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision mediump float;

uniform sampler2D uAreaLUT;

varying vec2 vFrom;
varying vec2 vCtrl;
varying vec2 vTo;

void main() {
    // Unpack.
    vec2 from = vFrom, ctrl = vCtrl, to = vTo;

    // Determine winding, and sort into a consistent order so we only need to find one root below.
    vec2 v0 = ctrl - from, v1 = to - ctrl;

    // Shoot a vertical ray toward the curve.
    vec2 window = clamp(vec2(from.x, to.x), -0.5, 0.5);
    //float offset = mix(window.x, window.y, 0.5) - left.x;
    //float t = offset / (v0.x + sqrt(v1.x * offset - v0.x * (offset - v0.x)));
    float t = 0.5;
    float x = mix(mix(from.x, ctrl.x, t), mix(ctrl.x, to.x, t), t);
    float dX = 2.0 * mix(v0.x, v1.x, t);
    t -= x / dX;
    x = mix(mix(from.x, ctrl.x, t), mix(ctrl.x, to.x, t), t);
    dX = 2.0 * mix(v0.x, v1.x, t);
    t -= x / dX;

    // Compute position and derivative to form a line approximation.
    float y = mix(mix(from.y, ctrl.y, t), mix(ctrl.y, to.y, t), t);
    //float dYDX = mix(v0.y, v1.y, t) / mix(v0.x, v1.x, t);
    float dYDX = dFdx(y);

    // Look up area under that line, and scale horizontally to the window size.
    dX = (gl_FrontFacing ? 1.0 : -1.0) * (window.x - window.y);
    gl_FragColor = vec4(texture2D(uAreaLUT, vec2(y + 8.0, abs(dYDX * dX)) / 16.0).r * dX);
}
