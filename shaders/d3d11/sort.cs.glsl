#version 430

// pathfinder/shaders/sort.cs.glsl
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

#define TILE_FIELD_NEXT_TILE_ID             0
#define TILE_FIELD_FIRST_FILL_ID            1
#define TILE_FIELD_BACKDROP_ALPHA_TILE_ID   2
#define TILE_FIELD_CONTROL                  3

#define FILL_INDIRECT_DRAW_PARAMS_SIZE      8

uniform int uTileCount;

restrict layout(std430, binding = 0) buffer bTiles {
    // [0]: next tile ID
    // [1]: first fill ID
    // [2]: backdrop delta upper 8 bits, alpha tile ID lower 24
    // [3]: color/ctrl/backdrop word
    uint iTiles[];
};

restrict layout(std430, binding = 1) buffer bFirstTileMap {
    int iFirstTileMap[];
};

restrict readonly layout(std430, binding = 2) buffer bZBuffer {
    int iZBuffer[];
};

layout(local_size_x = 64) in;

int getFirst(uint globalTileIndex) {
    return iFirstTileMap[globalTileIndex];
}

int getNextTile(int tileIndex) {
    return int(iTiles[tileIndex * 4 + TILE_FIELD_NEXT_TILE_ID]);
}

void setNextTile(int tileIndex, int newNextTileIndex) {
    iTiles[tileIndex * 4 + TILE_FIELD_NEXT_TILE_ID] = uint(newNextTileIndex);
}

void main() {
    uint globalTileIndex = gl_GlobalInvocationID.x;
    if (globalTileIndex >= uint(uTileCount))
        return;

    int zValue = iZBuffer[FILL_INDIRECT_DRAW_PARAMS_SIZE + globalTileIndex];

    int unsortedFirstTileIndex = getFirst(globalTileIndex);
    int sortedFirstTileIndex = -1;

    while (unsortedFirstTileIndex >= 0) {
        int currentTileIndex = unsortedFirstTileIndex;
        unsortedFirstTileIndex = getNextTile(currentTileIndex);

        if (currentTileIndex >= zValue) {
            int prevTrialTileIndex = -1;
            int trialTileIndex = sortedFirstTileIndex;
            while (true) {
                if (trialTileIndex < 0 || currentTileIndex < trialTileIndex) {
                    if (prevTrialTileIndex < 0) {
                        setNextTile(currentTileIndex, sortedFirstTileIndex);
                        sortedFirstTileIndex = currentTileIndex;
                    } else {
                        setNextTile(currentTileIndex, trialTileIndex);
                        setNextTile(prevTrialTileIndex, currentTileIndex);
                    }
                    break;
                }
                prevTrialTileIndex = trialTileIndex;
                trialTileIndex = getNextTile(trialTileIndex);
            }
        }
    }

    iFirstTileMap[globalTileIndex] = sortedFirstTileIndex;
}
