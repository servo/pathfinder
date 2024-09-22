#version 430

// pathfinder/shaders/bin.cs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Assigns microlines to tiles.

#extension GL_GOOGLE_include_directive : enable

#define MAX_ITERATIONS          1024u

#define STEP_DIRECTION_NONE     0
#define STEP_DIRECTION_X        1
#define STEP_DIRECTION_Y        2

#define TILE_FIELD_NEXT_TILE_ID             0
#define TILE_FIELD_FIRST_FILL_ID            1
#define TILE_FIELD_BACKDROP_ALPHA_TILE_ID   2
#define TILE_FIELD_CONTROL                  3

precision highp float;

#ifdef GL_ES
precision highp sampler2D;
#endif

layout(local_size_x = 64) in;

uniform int uMicrolineCount;
// How many slots we have allocated for fills.
uniform int uMaxFillCount;

restrict readonly layout(std430, binding = 0) buffer bMicrolines {
    uvec4 iMicrolines[];
};

restrict readonly layout(std430, binding = 1) buffer bMetadata {
    // [0]: tile rect
    // [1].x: tile offset
    // [1].y: path ID
    // [1].z: z write flag
    // [1].w: clip path ID
    // [2].x: backdrop offset
    ivec4 iMetadata[];
};

// [0]: vertexCount (6)
// [1]: instanceCount (of fills)
// [2]: vertexStart (0)
// [3]: baseInstance (0)
// [4]: alpha tile count
restrict layout(std430, binding = 2) buffer bIndirectDrawParams {
    uint iIndirectDrawParams[];
};

restrict writeonly layout(std430, binding = 3) buffer bFills {
    uint iFills[];
};

restrict layout(std430, binding = 4) buffer bTiles {
    // [0]: next tile ID (initialized to -1)
    // [1]: first fill ID (initialized to -1)
    // [2]: backdrop delta upper 8 bits, alpha tile ID lower 24 (initialized to 0, -1 respectively)
    // [3]: color/ctrl/backdrop word
    uint iTiles[];
};

restrict layout(std430, binding = 5) buffer bBackdrops {
    // [0]: backdrop
    // [1]: tile X offset
    // [2]: path ID
    uint iBackdrops[];
};

uint computeTileIndexNoCheck(ivec2 tileCoords, ivec4 pathTileRect, uint pathTileOffset) {
    ivec2 offsetCoords = tileCoords - pathTileRect.xy;
    return pathTileOffset + offsetCoords.x + offsetCoords.y * (pathTileRect.z - pathTileRect.x);
}

bvec4 computeTileOutcodes(ivec2 tileCoords, ivec4 pathTileRect) {
    return bvec4(lessThan(tileCoords, pathTileRect.xy),
                 greaterThanEqual(tileCoords, pathTileRect.zw));
}

bool computeTileIndex(ivec2 tileCoords,
                      ivec4 pathTileRect,
                      uint pathTileOffset,
                      out uint outTileIndex) {
    outTileIndex = computeTileIndexNoCheck(tileCoords, pathTileRect, pathTileOffset);
    return !any(computeTileOutcodes(tileCoords, pathTileRect));
}

void addFill(vec4 lineSegment, ivec2 tileCoords, ivec4 pathTileRect, uint pathTileOffset) {
    // Compute tile offset. If out of bounds, cull.
    uint tileIndex;
    if (!computeTileIndex(tileCoords, pathTileRect, pathTileOffset, tileIndex)) {
        return;
    }

    // Clip line. If too narrow, cull.
    uvec4 scaledLocalLine = uvec4((lineSegment - vec4(tileCoords.xyxy * ivec4(16))) * vec4(256.0));
    if (scaledLocalLine.x == scaledLocalLine.z)
        return;

    // Bump instance count.
    uint fillIndex = atomicAdd(iIndirectDrawParams[1], 1);

    // Fill out the link field, inserting into the linked list.
    uint fillLink = atomicExchange(iTiles[tileIndex * 4 + TILE_FIELD_FIRST_FILL_ID],
                                   int(fillIndex));

    // Write fill.
    if (fillIndex < uMaxFillCount) {
        iFills[fillIndex * 3 + 0] = scaledLocalLine.x | (scaledLocalLine.y << 16);
        iFills[fillIndex * 3 + 1] = scaledLocalLine.z | (scaledLocalLine.w << 16);
        iFills[fillIndex * 3 + 2] = fillLink;
    }
}

void adjustBackdrop(int backdropDelta,
                    ivec2 tileCoords,
                    ivec4 pathTileRect,
                    uint pathTileOffset,
                    uint pathBackdropOffset) {
    bvec4 outcodes = computeTileOutcodes(tileCoords, pathTileRect);
    if (any(outcodes)) {
        if (!outcodes.x && outcodes.y && !outcodes.z) {
            uint backdropIndex = pathBackdropOffset + uint(tileCoords.x - pathTileRect.x);
            atomicAdd(iBackdrops[backdropIndex * 3], backdropDelta);
        }
    } else {
        uint tileIndex = computeTileIndexNoCheck(tileCoords, pathTileRect, pathTileOffset);
        atomicAdd(iTiles[tileIndex * 4 + TILE_FIELD_BACKDROP_ALPHA_TILE_ID],
                  uint(backdropDelta) << 24);
    }
}

vec4 unpackMicroline(uvec4 packedMicroline, out uint outPathIndex) {
    outPathIndex = packedMicroline.w;
    ivec4 signedMicroline = ivec4(packedMicroline);
    return vec4((signedMicroline.x << 16) >> 16, signedMicroline.x >> 16,
                (signedMicroline.y << 16) >> 16, signedMicroline.y >> 16) +
            vec4(signedMicroline.z        & 0xff, (signedMicroline.z >> 8)  & 0xff,
                (signedMicroline.z >> 16) & 0xff, (signedMicroline.z >> 24) & 0xff) / 256.0;
}

void main() {
    uint segmentIndex = gl_GlobalInvocationID.x;
    if (segmentIndex >= uMicrolineCount)
        return;

    uint pathIndex;
    vec4 lineSegment = unpackMicroline(iMicrolines[segmentIndex], pathIndex);

    ivec4 pathTileRect = iMetadata[pathIndex * 3 + 0];
    uint pathTileOffset = uint(iMetadata[pathIndex * 3 + 1].x);
    uint pathBackdropOffset = uint(iMetadata[pathIndex * 3 + 2].x);

    // Following is a straight port of `process_line_segment()`:

    ivec2 tileSize = ivec2(16);

    ivec4 tileLineSegment = ivec4(floor(lineSegment / vec4(tileSize.xyxy)));
    ivec2 fromTileCoords = tileLineSegment.xy, toTileCoords = tileLineSegment.zw;

    vec2 vector = lineSegment.zw - lineSegment.xy;
    vec2 vectorIsNegative = vec2(vector.x < 0.0 ? -1.0 : 0.0, vector.y < 0.0 ? -1.0 : 0.0);
    ivec2 tileStep = ivec2(vector.x < 0.0 ? -1 : 1, vector.y < 0.0 ? -1 : 1);

    vec2 firstTileCrossing = vec2((fromTileCoords + ivec2(vector.x >= 0.0 ? 1 : 0,
                                                          vector.y >= 0.0 ? 1 : 0)) * tileSize);

    vec2 tMax = (firstTileCrossing - lineSegment.xy) / vector;
    vec2 tDelta = abs(tileSize / vector);

    vec2 currentPosition = lineSegment.xy;
    ivec2 tileCoords = fromTileCoords;
    int lastStepDirection = STEP_DIRECTION_NONE;
    uint iteration = 0;

    while (iteration < MAX_ITERATIONS) {
        int nextStepDirection;
        if (tMax.x < tMax.y)
            nextStepDirection = STEP_DIRECTION_X;
        else if (tMax.x > tMax.y)
            nextStepDirection = STEP_DIRECTION_Y;
        else if (tileStep.x > 0.0)
            nextStepDirection = STEP_DIRECTION_X;
        else
            nextStepDirection = STEP_DIRECTION_Y;

        float nextT = min(nextStepDirection == STEP_DIRECTION_X ? tMax.x : tMax.y, 1.0);

        // If we've reached the end tile, don't step at all.
        if (tileCoords == toTileCoords)
            nextStepDirection = STEP_DIRECTION_NONE;

        vec2 nextPosition = mix(lineSegment.xy, lineSegment.zw, nextT);
        vec4 clippedLineSegment = vec4(currentPosition, nextPosition);
        addFill(clippedLineSegment, tileCoords, pathTileRect, pathTileOffset);

        // Add extra fills if necessary.
        vec4 auxiliarySegment;
        bool haveAuxiliarySegment = false;
        if (tileStep.y < 0 && nextStepDirection == STEP_DIRECTION_Y) {
            auxiliarySegment = vec4(clippedLineSegment.zw, vec2(tileCoords * tileSize));
            haveAuxiliarySegment = true;
        } else if (tileStep.y > 0 && lastStepDirection == STEP_DIRECTION_Y) {
            auxiliarySegment = vec4(vec2(tileCoords * tileSize), clippedLineSegment.xy);
            haveAuxiliarySegment = true;
        }
        if (haveAuxiliarySegment)
            addFill(auxiliarySegment, tileCoords, pathTileRect, pathTileOffset);

        // Adjust backdrop if necessary.
        //
        // NB: Do not refactor the calls below. This exact code sequence is needed to avoid a
        // miscompilation on the Radeon Metal compiler.
        if (tileStep.x < 0 && lastStepDirection == STEP_DIRECTION_X) {
            adjustBackdrop(1,
                           tileCoords,
                           pathTileRect,
                           pathTileOffset,
                           pathBackdropOffset);
        } else if (tileStep.x > 0 && nextStepDirection == STEP_DIRECTION_X) {
            adjustBackdrop(-1,
                           tileCoords,
                           pathTileRect,
                           pathTileOffset,
                           pathBackdropOffset);
        }

        // Take a step.
        if (nextStepDirection == STEP_DIRECTION_X) {
            tMax.x += tDelta.x;
            tileCoords.x += tileStep.x;
        } else if (nextStepDirection == STEP_DIRECTION_Y) {
            tMax.y += tDelta.y;
            tileCoords.y += tileStep.y;
        } else if (nextStepDirection == STEP_DIRECTION_NONE) {
            break;
        }

        currentPosition = nextPosition;
        lastStepDirection = nextStepDirection;

        iteration++;
    }
}
