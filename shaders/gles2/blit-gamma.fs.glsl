// pathfinder/shaders/gles2/blit-gamma.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Blits a texture, applying gamma correction.

precision mediump float;

/// The source texture to blit.
uniform sampler2D uSource;

/// The approximate background color, in linear RGB.
uniform vec3 uBGColor;

/// The gamma LUT.
uniform sampler2D uGammaLUT;

/// The incoming texture coordinate.
varying vec2 vTexCoord;

void main() {
    vec4 source = texture2D(uSource, vTexCoord);
    gl_FragColor = vec4(gammaCorrect(source.rgb, uBGColor, uGammaLUT), source.a);
}
