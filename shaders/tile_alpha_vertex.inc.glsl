// pathfinder/shaders/tile_alpha_vertex.inc.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

uniform mat4 uTransform;
uniform vec2 uTileSize;
uniform vec2 uStencilTextureSize;

in uvec2 aTessCoord;
in uvec3 aTileOrigin;
in int aBackdrop;
in int aTileIndex;

out vec2 vTexCoord;
out float vBackdrop;
out vec4 vColor;

vec4 getColor();

vec2 computeTileOffset(uint tileIndex, float stencilTextureWidth) {
    uint tilesPerRow = uint(stencilTextureWidth / uTileSize.x);
    uvec2 tileOffset = uvec2(tileIndex % tilesPerRow, tileIndex / tilesPerRow);
    return vec2(tileOffset) * uTileSize;
}

void computeVaryings() {
    vec2 origin = vec2(aTileOrigin.xy) + vec2(aTileOrigin.z & 15u, aTileOrigin.z >> 4u) * 256.0;
    vec2 position = (origin + vec2(aTessCoord)) * uTileSize;
    vec2 maskTexCoordOrigin = computeTileOffset(uint(aTileIndex), uStencilTextureSize.x);
    vec2 maskTexCoord = maskTexCoordOrigin + vec2(aTessCoord) * uTileSize;

    vTexCoord = maskTexCoord / uStencilTextureSize;
    vBackdrop = float(aBackdrop);
    vColor = getColor();
    gl_Position = uTransform * vec4(position, 0.0, 1.0);
}

