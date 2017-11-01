// pathfinder/shaders/gles2/composite-alpha-mask.vs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision mediump float;

/// A 3D transform to apply to the scene.
uniform mat4 uTransform;

/// The 2D vertex position.
attribute vec2 aPosition;
/// The texture coordinate.
attribute vec2 aTexCoord;

varying vec2 vTexCoord;

void main() {
    gl_Position = uTransform * vec4(aPosition, 0.0, 1.0);
    vTexCoord = aTexCoord;
}
