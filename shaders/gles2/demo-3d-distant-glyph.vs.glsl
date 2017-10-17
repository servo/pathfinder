// pathfinder/shaders/gles2/demo-3d-distant-glyph.vs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform mat4 uTransform;
uniform vec4 uGlyphTexCoords;
uniform vec2 uGlyphSize;

attribute vec2 aQuadPosition;
attribute vec2 aPosition;

varying vec2 vTexCoord;

void main() {
    vec2 positionBL = aPosition, positionTR = aPosition + uGlyphSize;
    gl_Position = uTransform * vec4(mix(positionBL, positionTR, aQuadPosition), 0.0, 1.0);
    vTexCoord = mix(uGlyphTexCoords.xy, uGlyphTexCoords.zw, aQuadPosition);
}
