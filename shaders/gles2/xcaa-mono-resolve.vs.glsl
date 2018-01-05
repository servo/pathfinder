// pathfinder/shaders/gles2/xcaa-mono-resolve.vs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Renders a single-channel alpha coverage buffer to an RGB framebuffer.

precision highp float;

/// A dilation (scale and transform) to be applied to the quad.
uniform vec4 uTransformST;
/// A fixed pair of factors to be applied to the texture coordinates.
uniform vec2 uTexScale;

attribute vec2 aPosition;
attribute vec2 aTexCoord;

varying vec2 vTexCoord;

void main() {
    gl_Position = vec4(transformVertexPositionST(aPosition, uTransformST), -1.0, 1.0);
    vTexCoord = aTexCoord * uTexScale;
}
