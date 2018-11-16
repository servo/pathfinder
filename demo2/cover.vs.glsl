#version 300 es

// pathfinder/demo2/cover.vs.glsl
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform vec2 uFramebufferSize;
uniform vec2 uTileSize;
uniform vec2 uStencilTextureSize;

in vec2 aTessCoord;
in vec2 aTileOrigin;
in uint aTileIndex;
in vec4 aColor;

out vec2 vTexCoord;
out vec4 vColor;

vec2 computeTileOffset(uint tileIndex, float stencilTextureWidth) {
    uint tilesPerRow = uint(stencilTextureWidth / uTileSize.x);
    uvec2 tileOffset = uvec2(aTileIndex % tilesPerRow, aTileIndex / tilesPerRow);
    return vec2(tileOffset) * uTileSize;
}

void main() {
    vec2 position = aTileOrigin + uTileSize * aTessCoord;
    vec2 texCoord = computeTileOffset(aTileIndex, uStencilTextureSize.x) + aTessCoord * uTileSize;
    vTexCoord = texCoord / uStencilTextureSize;
    vColor = aColor;
    gl_Position = vec4((position / uFramebufferSize * 2.0 - 1.0) * vec2(1.0, -1.0), 0.0, 1.0);
}
