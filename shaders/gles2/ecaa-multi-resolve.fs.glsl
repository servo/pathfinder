// pathfinder/shaders/gles2/ecaa-multi-resolve.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform sampler2D uBGColor;
uniform sampler2D uFGColor;
uniform sampler2D uAAAlpha;

varying vec2 vTexCoord;

void main() {
    vec4 bgColor = texture2D(uBGColor, vTexCoord);
    vec4 fgColor = texture2D(uFGColor, vTexCoord);
    float alpha = clamp(texture2D(uAAAlpha, vTexCoord).r, 0.0, 1.0);
    gl_FragColor = mix(bgColor, fgColor, alpha);
}
