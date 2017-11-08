// pathfinder/shaders/gles2/blit-linear.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// A trivial shader that does nothing more than blit a texture.

precision mediump float;

uniform sampler2D uSource;

varying vec2 vTexCoord;

void main() {
    gl_FragColor = texture2D(uSource, vTexCoord);
}
