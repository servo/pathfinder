#version {{version}}

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

    vec2 window = clamp(vec2(from.x, to.x), -0.5, 0.5);
    vec2 a = from.y + (window - from.x) * (to.y - from.y) / (to.x - from.x) + 0.5;
    float ymin = min(min(a.x, a.y), 1) - 1e-6;
    float ymax = max(a.x, a.y);
    float b = min(ymax, 1);
    float c = max(b, 0);
    float d = max(ymin, 0);
    float tex = (b - 0.5 * c * c - ymin + 0.5 * d * d) / (ymax - ymin);
    float dX = window.x - window.y;

    oFragColor = vec4(tex * dX);
}
