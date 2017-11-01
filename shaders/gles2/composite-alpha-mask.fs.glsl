// pathfinder/shaders/gles2/composite-alpha-mask.fs.glsl
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
uniform sampler2D uMask;

varying vec2 vTexCoord;

void main() {
    vec4 color = texture2D(uSource, vTexCoord);
    float alpha = texture2D(uMask, vTexCoord).a;
    gl_FragColor = vec4(color.rgb, color.a * alpha);
}
