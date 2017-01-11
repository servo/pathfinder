// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Computes exact area coverage for lines, breaking Bézier curves down into them as necessary.
// Proceeds top to bottom for better data locality during the subsequent accumulation stage. For
// details on the algorithm, see [1].
//
// [1]: http://nothings.org/gamedev/rasterize/

#define POINTS_PER_SEGMENT  32
#define TILE_SIZE           4

#define OPERATION_MOVE      0
#define OPERATION_ON_CURVE  1
#define OPERATION_OFF_CURVE 2

struct GlyphDescriptor {
    short4 rect;
    ushort unitsPerEm;
    ushort pointCount;
    uint startPoint;
};

typedef struct GlyphDescriptor GlyphDescriptor;

struct ImageDescriptor {
    uint2 atlasPosition;
    float pointSize;
    uint glyphIndex;
    uint startPointInBatch;
    uint pointCount;
};

typedef struct ImageDescriptor ImageDescriptor;

__global int *getPixel(__global int *gPixels, uint2 point, uint widthInTiles) {
    uint2 tile = point / TILE_SIZE, pointInTile = point % TILE_SIZE;
    return &gPixels[(tile.y * widthInTiles + tile.x) * TILE_SIZE * TILE_SIZE +
                    pointInTile.y * TILE_SIZE +
                    pointInTile.x];
}

uchar getOperation(uint globalPointIndex, __global const uchar *gOperations) {
    return (gOperations[globalPointIndex / 4] >> (globalPointIndex % 4 * 2)) & 0x3;
}

void plot(__global int *gPixels, uint2 point, uint widthInTiles, float coverage) {
    __global int *pixel = getPixel(gPixels, point, widthInTiles);
    int oldCoverage = as_int(*pixel);
    while (true) {
        int newCoverage = as_int(as_float(oldCoverage) + coverage);
        int existingCoverage = atomic_cmpxchg(pixel, oldCoverage, newCoverage);
        if (existingCoverage == oldCoverage)
            break;
        oldCoverage = existingCoverage;
    }
}

__kernel void draw(__global const ImageDescriptor *gImages,
                   __global const GlyphDescriptor *gGlyphs,
                   __global const short2 *gCoordinates,
                   __global const uchar *gOperations,
                   __global const uint *gIndices,
                   __global int *gPixels) {
    // Find the image.
    int batchID = get_global_id(0);
    uint imageID = gIndices[batchID / POINTS_PER_SEGMENT];
    __global const ImageDescriptor *image = &gImages[imageID];
    while (batchID >= image->startPointInBatch + image->pointCount) {
        imageID++;
        image = &gImages[imageID];
    }

    // Find the glyph.
    uint glyphIndex = image->glyphIndex;
    __global const GlyphDescriptor *glyph = &gGlyphs[glyphIndex];

    // Unpack glyph and image.
    uint2 atlasPosition = image->atlasPosition;
    float pixelsPerUnit = image->pointSize * convert_float(glyph->unitsPerEm);
    uint pointIndexInGlyph = batchID - image->startPointInBatch;
    uint globalPointIndex = glyph->startPoint + pointIndexInGlyph;

    // Stop here if this is a move operation.
    uchar curOperation = getOperation(globalPointIndex, gOperations);
    if (curOperation == OPERATION_MOVE)
        return;

    // Unpack the points that make up this line or curve.
    short2 p0, p1, p2;
    float t0, t1;
    uchar prevOperation = getOperation(globalPointIndex - 1, gOperations);
    short2 prevPoint = gCoordinates[globalPointIndex - 1];
    short2 curPoint = gCoordinates[globalPointIndex];
    if (prevOperation == OPERATION_OFF_CURVE) {
        p0 = gCoordinates[globalPointIndex - 2];
        p1 = prevPoint;
        p2 = curPoint;
        t0 = 0.0f;
        t1 = 0.5f;
    } else if (curOperation == OPERATION_OFF_CURVE) {
        p0 = prevPoint;
        p1 = curPoint;
        p2 = gCoordinates[globalPointIndex + 1];
        t0 = 0.5f;
        t1 = 1.0f;
    } else {
        p0 = prevPoint;
        p2 = curPoint;
    }
}

