// pathfinder/resources/shaders/tile_alpha_vertex.inc.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

uniform vec2 uFramebufferSize;
uniform vec2 uTileSize;
uniform vec2 uStencilTextureSize;
uniform vec2 uViewBoxOrigin;

in vec2 aTessCoord;
in vec2 aTileOrigin;
in int aBackdrop;
in uint aObject;

out vec2 vTexCoord;
out float vBackdrop;
out vec4 vColor;

vec4 getFillColor(uint object);

vec2 computeTileOffset(uint tileIndex, float stencilTextureWidth) {
    uint tilesPerRow = uint(stencilTextureWidth / uTileSize.x);
    uvec2 tileOffset = uvec2(tileIndex % tilesPerRow, tileIndex / tilesPerRow);
    return vec2(tileOffset) * uTileSize;
}

void computeVaryings() {
    uint tileIndex = uint(gl_InstanceID);
    vec2 pixelPosition = (aTileOrigin + aTessCoord) * uTileSize + uViewBoxOrigin;
    vec2 position = (pixelPosition / uFramebufferSize * 2.0 - 1.0) * vec2(1.0, -1.0);
    vec2 texCoord = computeTileOffset(tileIndex, uStencilTextureSize.x) + aTessCoord * uTileSize;

    vTexCoord = texCoord / uStencilTextureSize;
    vBackdrop = float(aBackdrop);
    vColor = getFillColor(aObject);
    gl_Position = vec4(position, 0.0, 1.0);
}

