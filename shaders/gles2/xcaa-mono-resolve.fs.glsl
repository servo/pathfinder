// pathfinder/shaders/gles2/xcaa-mono-resolve.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Renders a single-channel alpha coverage buffer to an RGB framebuffer.

precision mediump float;

/// The background color of the monochrome path.
uniform vec4 uBGColor;
/// The foreground color of the monochrome path.
uniform vec4 uFGColor;
/// The alpha coverage texture.
uniform sampler2D uAAAlpha;

varying vec2 vTexCoord;

void main() {
    float alpha = abs(texture2D(uAAAlpha, vTexCoord).r);
    gl_FragColor = mix(uBGColor, uFGColor, alpha);
}
