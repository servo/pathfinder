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

void plot(__global int *gPixels,
          uint2 point,
          uint widthInTiles,
          uint imageHeight,
          float coverage) {
    __global int *pixel = getPixel(gPixels,
                                   (uint2)(point.x, imageHeight - point.y - 1),
                                   widthInTiles);
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
                   __global int *gPixels,
                   uint atlasWidth) {
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

    // Convert units to pixels.
    float2 pP0 = convert_float2(p0) * pixelsPerUnit;
    float2 pP1 = convert_float2(p1) * pixelsPerUnit;
    float2 pP2 = convert_float2(p2) * pixelsPerUnit;

    // Determine the direction we're going.
    float2 direction = copysign((float2)(1.0f, 1.0f), pP0 - pP2);

    // Set up plotting.
    uint widthInTiles = atlasWidth / TILE_SIZE;
    short4 glyphRect = glyph->rect;
    uint imageHeight = convert_uint(glyphRect.w - glyphRect.y);

    // Loop over each line segment.
    float t = t0;
    while (t < t1) {
        // Compute endpoints.
        float2 lP0, lP1;
        if (direction.x >= 0.0f) {
            lP0 = pP0;
            lP1 = pP2;
        } else {
            lP0 = pP2;
            lP1 = pP0;
        }

        // Compute the slope.
        float dXdY = fabs(lP1.x - lP0.x / lP1.y - lP0.y);

        // Initialize the current point. Determine how long the segment extends across the first
        // pixel column.
        int2 p = (int2)((int)p0.x, 0);
        float dX = min(convert_float(p.x) + 1.0f, lP1.x) - lP0.x;

        // Initialize `yLeft` and `yRight`, the intercepts of Y with the current pixel.
        float yLeft = lP0.y;
        float yRight = yLeft + direction.y * dX / dXdY;

        // Iterate over columns.
        while (p.x < (int)ceil(lP1.x)) {
            // Flip `yLeft` and `yRight` around if necessary so that the slope is positive.
            float y0, y1;
            if (yLeft <= yRight) {
                y0 = yLeft;
                y1 = yRight;
            } else {
                y0 = yRight;
                y1 = yLeft;
            }

            // Split `y0` into fractional and whole parts, and split `y1` into remaining fractional
            // and whole parts.
            float y0R, y1R;
            float y0F = fract(y0, &y0R), y1F = fract(y1, &y1R);
            int y0I = convert_int(y0R), y1I = convert_int(y1R);
            if (y1F != 0.0f)
                y1I++;

            // Compute area coverage for the first pixel.
            float coverage;
            if (y1I <= y0I + 1) {
                // The line is less than one pixel. This is a trapezoid.
                coverage = 1.0f - mix(y0F, y1F, 0.5f);
            } else {
                // Usual case: This is a triangle.
                coverage = 0.5f * dXdY * (1.0f - y0F) * (1.0f - y0F);
            }

            // Plot the first pixel of this column.
            plot(gPixels, as_uint2(p), widthInTiles, imageHeight, dX * direction.x * coverage);

            // Since the coverage of this row must sum to 1, we keep track of the total coverage.
            float coverageLeft = coverage;

            // Plot the pixels between the first and the last.
            if (p.y + 1 < y1I) {
                // Compute coverage for and plot the second pixel in the column.
                p.y++;
                if (p.y + 1 == y1I)
                    coverage = 1.0f - (0.5f * dXdY * y1F * y1F) - coverage;
                else
                    coverage = dXdY * (1.5f - y0F) - coverage;
                coverageLeft += coverage;
                plot(gPixels, as_uint2(p), widthInTiles, imageHeight, dX * direction.x * coverage);

                // Iterate over any remaining pixels.
                p.y++;
                coverage = dXdY;
                while (p.y < y1I) {
                    coverageLeft += coverage;
                    plot(gPixels,
                         as_uint2(p),
                         widthInTiles,
                         imageHeight,
                         dX * direction.x * coverage);
                    p.y++;
                }
            }

            // Plot the remaining coverage.
            coverage = 1.0f - coverageLeft;
            plot(gPixels, as_uint2(p), widthInTiles, imageHeight, dX * direction.x * coverage);

            // Move to the next column.
            p.x++;

            // Compute Y intercepts for the next column.
            yLeft = yRight;
            float yRight = yLeft + direction.y * dX / dXdY;

            // Determine how long the segment extends across the next pixel column.
            dX = min(convert_float(p.x) + 1.0f, lP1.x) - convert_float(p.x);
        }
    }
}

