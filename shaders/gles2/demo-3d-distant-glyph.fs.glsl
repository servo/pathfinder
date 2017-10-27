// pathfinder/shaders/gles2/demo-3d-distant-glyph.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform vec4 uColor;
uniform sampler2D uAtlas;

varying vec2 vTexCoord;

void main() {
    gl_FragColor = uColor * texture2D(uAtlas, vTexCoord).rrrr;
}
