#version 330

// pathfinder/demo2/stencil.fs.glsl
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform sampler2D uAreaLUT;

in vec2 vFrom;
in vec2 vTo;

out vec4 oFragColor;

void main() {
    // Unpack.
    vec2 from = vFrom, to = vTo;

    // Determine winding, and sort into a consistent order so we only need to find one root below.
    bool winding = from.x < to.x;
    vec2 left = winding ? from : to, right = winding ? to : from;

    // Shoot a vertical ray toward the curve.
    vec2 window = clamp(vec2(from.x, to.x), -0.5, 0.5);
    float offset = mix(window.x, window.y, 0.5) - left.x;
    float t = offset / (right.x - left.x);

    // Compute position and derivative to form a line approximation.
    float y = mix(left.y, right.y, t);
    float d = (right.y - left.y) / (right.x - left.x);

    // Look up area under that line, and scale horizontally to the window size.
    float dX = window.x - window.y;
    oFragColor = vec4(texture(uAreaLUT, vec2(y + 8.0, abs(d * dX)) / 16.0).r * dX);
}
