#version 330

// pathfinder/shaders/tile_solid.vs.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform mat4 uTransform;
uniform vec2 uTileSize;

in uvec2 aTessCoord;
in ivec2 aTileOrigin;
in vec4 aColorTexMatrix;
in vec2 aColorTexOffset;

out vec2 vColorTexCoord;

void main() {
    vec2 tileOffset = vec2(aTessCoord) * uTileSize;
    vec2 position = aTileOrigin * uTileSize + tileOffset;
    vColorTexCoord = mat2(aColorTexMatrix) * tileOffset + aColorTexOffset;
    gl_Position = uTransform * vec4(position, 0.0, 1.0);
}
