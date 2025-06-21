#version 430

// pathfinder/shaders/fill.cs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#extension GL_GOOGLE_include_directive : enable

precision highp float;

#ifdef GL_ES
precision highp sampler2D;
#endif

#include "fill_area.inc.glsl"

layout(local_size_x = 16, local_size_y = 4) in;

#define TILE_FIELD_NEXT_TILE_ID             0
#define TILE_FIELD_FIRST_FILL_ID            1
#define TILE_FIELD_BACKDROP_ALPHA_TILE_ID   2
#define TILE_FIELD_CONTROL                  3

#define TILE_CTRL_MASK_MASK                     0x3
#define TILE_CTRL_MASK_WINDING                  0x1
#define TILE_CTRL_MASK_EVEN_ODD                 0x2

#define TILE_CTRL_MASK_0_SHIFT                  0

layout(rgba8) uniform image2D uDest;
uniform sampler2D uAreaLUT;
uniform ivec2 uAlphaTileRange;

restrict readonly layout(std430, binding = 0) buffer bFills {
    uint iFills[];
};

restrict layout(std430, binding = 1) buffer bTiles {
    // [0]: path ID
    // [1]: next tile ID
    // [2]: first fill ID
    // [3]: backdrop delta upper 8 bits, alpha tile ID lower 24 bits
    // [4]: color/ctrl/backdrop word
    uint iTiles[];
};

restrict readonly layout(std430, binding = 2) buffer bAlphaTiles {
    // [0]: alpha tile index
    // [1]: clip tile index
    uint iAlphaTiles[];
};

#include "fill_compute.inc.glsl"

ivec2 computeTileCoord(uint alphaTileIndex) {
    uint x = alphaTileIndex & 0xff;
    uint y = (alphaTileIndex >> 8u) & 0xff + (((alphaTileIndex >> 16u) & 0xff) << 8u);
    return ivec2(16, 4) * ivec2(x, y) + ivec2(gl_LocalInvocationID.xy);
}

void main() {
    ivec2 tileSubCoord = ivec2(gl_LocalInvocationID.xy) * ivec2(1, 4);

    // This is a workaround for the 64K workgroup dispatch limit in OpenGL.
    uint batchAlphaTileIndex = (gl_WorkGroupID.x | (gl_WorkGroupID.y << 15));
    uint alphaTileIndex = batchAlphaTileIndex + uint(uAlphaTileRange.x);
    if (alphaTileIndex >= uint(uAlphaTileRange.y))
        return;

    uint tileIndex = iAlphaTiles[batchAlphaTileIndex * 2 + 0];
    if ((int(iTiles[tileIndex * 4 + TILE_FIELD_BACKDROP_ALPHA_TILE_ID] << 8) >> 8) < 0)
        return;

    int fillIndex = int(iTiles[tileIndex * 4 + TILE_FIELD_FIRST_FILL_ID]);
    int backdrop = int(iTiles[tileIndex * 4 + TILE_FIELD_CONTROL]) >> 24;

    vec4 coverages = vec4(backdrop);
    coverages += accumulateCoverageForFillList(fillIndex, tileSubCoord);

    uint tileControlWord = iTiles[tileIndex * 4 + TILE_FIELD_CONTROL];
    int tileCtrl = int((tileControlWord >> 16) & 0xffu);
    int maskCtrl = (tileCtrl >> TILE_CTRL_MASK_0_SHIFT) & TILE_CTRL_MASK_MASK;
    if ((maskCtrl & TILE_CTRL_MASK_WINDING) != 0) {
        coverages = clamp(abs(coverages), 0.0, 1.0);
    } else {
        coverages = clamp(1.0 - abs(1.0 - mod(coverages, 2.0)), 0.0, 1.0);
    }

    // Handle clip if necessary.
    int clipTileIndex = int(iAlphaTiles[batchAlphaTileIndex * 2 + 1]);
    if (clipTileIndex >= 0)
        coverages = min(coverages, imageLoad(uDest, computeTileCoord(clipTileIndex)));

    imageStore(uDest, computeTileCoord(alphaTileIndex), coverages);
}
