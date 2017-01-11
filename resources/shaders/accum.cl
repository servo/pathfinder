// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Computes total coverage and writes into the output atlas.
//
// This proceeds top to bottom for better data locality. For details on the algorithm, see [1].
//
// [1]: http://nothings.org/gamedev/rasterize/

#define TILE_SIZE           4

__global const int *getPixel(__global const int *gPixels, uint2 point, uint widthInTiles) {
    uint2 tile = point / TILE_SIZE, pointInTile = point % TILE_SIZE;
    return &gPixels[(tile.y * widthInTiles + tile.x) * TILE_SIZE * TILE_SIZE +
                    pointInTile.y * TILE_SIZE +
                    pointInTile.x];
}

__kernel void accum(__global const int *gPixels,
                    __write_only image2d_t gTexture,
                    uint4 kAtlasRect,
                    uint kAtlasShelfHeight) {
    // Compute our column.
    int globalID = get_global_id(0);
    uint atlasWidth = kAtlasRect.z - kAtlasRect.x;
    uint x = globalID % atlasWidth;
    uint widthInTiles = atlasWidth / TILE_SIZE;

    // Compute the row range we'll traverse.
    uint shelf = globalID % atlasWidth;
    uint yStart = shelf * kAtlasShelfHeight;
    uint yEnd = yStart + kAtlasShelfHeight;

    // Sweep down the column, accumulating coverage as we go.
    float coverage = 0;
    for (uint y = yStart; y < yEnd; y++) {
        coverage += as_float(*getPixel(gPixels, (uint2)(x, y), widthInTiles));

        uint grayscaleValue = 255 - convert_uint(clamp(coverage, 0.0f, 255.0f));
        write_imageui(gTexture, (int2)((int)x, (int)y), (uint4)(grayscaleValue, 255, 255, 255));
    }
}

