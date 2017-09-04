// pathfinder/shaders/gles2/demo-3d-monument.vs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision mediump float;

uniform mat4 uTransform;

attribute vec3 aPosition;

void main() {
    gl_Position = uTransform * vec4(aPosition, 1.0);
}
