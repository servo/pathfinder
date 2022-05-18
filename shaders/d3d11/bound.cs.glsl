#version 430

// pathfinder/shaders/bound.cs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Initializes the tile maps.

#extension GL_GOOGLE_include_directive : enable

precision highp float;

#ifdef GL_ES
precision highp sampler2D;
#endif

#define TILE_FIELD_NEXT_TILE_ID             0
#define TILE_FIELD_FIRST_FILL_ID            1
#define TILE_FIELD_BACKDROP_ALPHA_TILE_ID   2
#define TILE_FIELD_CONTROL                  3

layout(local_size_x = 64) in;

uniform int uPathCount;
uniform int uTileCount;

restrict readonly layout(std430, binding = 0) buffer bTilePathInfo {
    // x: tile upper left, 16-bit packed x/y
    // y: tile lower right, 16-bit packed x/y
    // z: first tile index in this path
    // w: color/ctrl/backdrop word
    uvec4 iTilePathInfo[];
};

restrict layout(std430, binding = 1) buffer bTiles {
    // [0]: next tile ID (initialized to -1)
    // [1]: first fill ID (initialized to -1)
    // [2]: backdrop delta upper 8 bits, alpha tile ID lower 24 (initialized to 0, -1 respectively)
    // [3]: color/ctrl/backdrop word
    uint iTiles[];
};

void main() {
    uint tileIndex = gl_GlobalInvocationID.x;
    if (tileIndex >= uint(uTileCount))
        return;

    uint lowPathIndex = 0, highPathIndex = uint(uPathCount);
    int iteration = 0;
    while (iteration < 1024 && lowPathIndex + 1 < highPathIndex) {
        uint midPathIndex = lowPathIndex + (highPathIndex - lowPathIndex) / 2;
        uint midTileIndex = iTilePathInfo[midPathIndex].z;
        if (tileIndex < midTileIndex) {
            highPathIndex = midPathIndex;
        } else {
            lowPathIndex = midPathIndex;
            if (tileIndex == midTileIndex)
                break;
        }
        iteration++;
    }

    uint pathIndex = lowPathIndex;
    uvec4 pathInfo = iTilePathInfo[pathIndex];

    ivec2 packedTileRect = ivec2(pathInfo.xy);
    ivec4 tileRect = ivec4((packedTileRect.x << 16) >> 16, packedTileRect.x >> 16,
                           (packedTileRect.y << 16) >> 16, packedTileRect.y >> 16);

    uint tileOffset = tileIndex - pathInfo.z;
    uint tileWidth = uint(tileRect.z - tileRect.x);
    ivec2 tileCoords = tileRect.xy + ivec2(tileOffset % tileWidth, tileOffset / tileWidth);

    iTiles[tileIndex * 4 + TILE_FIELD_NEXT_TILE_ID] = ~0u;
    iTiles[tileIndex * 4 + TILE_FIELD_FIRST_FILL_ID] = ~0u;
    iTiles[tileIndex * 4 + TILE_FIELD_BACKDROP_ALPHA_TILE_ID] = 0x00ffffffu;
    iTiles[tileIndex * 4 + TILE_FIELD_CONTROL] = pathInfo.w;
}
