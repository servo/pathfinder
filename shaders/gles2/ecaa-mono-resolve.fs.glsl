// pathfinder/shaders/gles2/ecaa-mono-resolve.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision mediump float;

uniform vec4 uBGColor;
uniform vec4 uFGColor;
uniform sampler2D uAAAlpha;

varying vec2 vTexCoord;

void main() {
    float alpha = clamp(texture2D(uAAAlpha, vTexCoord).r, 0.0, 1.0);
    gl_FragColor = vec4(uFGColor.rgb, uFGColor.a * alpha);
}
