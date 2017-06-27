// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Computes total coverage and writes into the output atlas for grayscale antialiasing.
//
// This proceeds top to bottom for better data locality. For details on the algorithm, see [1].
//
// [1]: https://medium.com/@raphlinus/inside-the-fastest-font-renderer-in-the-world-75ae5270c445

const sampler_t SAMPLER = CLK_NORMALIZED_COORDS_FALSE | CLK_ADDRESS_NONE | CLK_FILTER_NEAREST;

__kernel void accum_gray(__write_only image2d_t gImage,
                         __read_only image2d_t gCoverage,
                         uint4 kAtlasRect,
                         uint kAtlasShelfHeight) {
    // Determine the boundaries of the column we'll be traversing.
    uint column = 0, firstRow = 0, lastRow = 0;
    get_location(&column, &firstRow, &lastRow, kAtlasRect, kAtlasShelfHeight);

    // Sweep down the column, accumulating coverage as we go.
    float coverage = 0.0f;
    for (uint row = firstRow; row < lastRow; row++) {
        int2 coord = (int2)((int)column, (int)row);
        coverage += read_imagef(gCoverage, SAMPLER, coord).r;

        float gray = fabs(coverage);
        write_imagef(gImage, coord + (int2)kAtlasRect.xy, (float4)(gray, gray, gray, 1.0f));
    }
}

