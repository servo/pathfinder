// pathfinder/shaders/gles2/ecaa-fast-cover.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

varying vec2 vHorizontalExtents;

void main() {
    vec2 sides = gl_FragCoord.xx + vec2(-0.5, 0.5);
    vec2 clampedSides = clamp(vHorizontalExtents, sides.x, sides.y);
    gl_FragColor = vec4(vec3(clampedSides.y - clampedSides.x), 1.0);
}
