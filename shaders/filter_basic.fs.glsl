#version 330

// pathfinder/shaders/filter_basic.fs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// TODO(pcwalton): This could be significantly optimized by operating on a
// sparse per-tile basis.

#extension GL_GOOGLE_include_directive : enable

precision highp float;

uniform sampler2D uSource;
uniform vec2 uSourceSize;

in vec2 vTexCoord;

out vec4 oFragColor;

void main() {
    oFragColor = texture(uSource, vTexCoord);
}
