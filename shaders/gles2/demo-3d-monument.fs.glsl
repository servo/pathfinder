// pathfinder/shaders/gles2/demo-3d-monument.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision mediump float;

uniform vec4 uColor;

void main() {
    // TODO(pcwalton): Lighting.
    gl_FragColor = uColor;
}
