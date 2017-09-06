// pathfinder/shaders/gles2/ssaa-subpixel-resolve.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision mediump float;

uniform sampler2D uSource;
uniform ivec2 uSourceDimensions;

varying vec2 vTexCoord;

void main() {
    float onePixel = 1.0 / float(uSourceDimensions.x);
    gl_FragColor = vec4(texture2D(uSource, vec2(vTexCoord.s - onePixel, vTexCoord.t)).r,
                        texture2D(uSource, vec2(vTexCoord.s,            vTexCoord.t)).r,
                        texture2D(uSource, vec2(vTexCoord.s + onePixel, vTexCoord.t)).r,
                        1.0);
}
