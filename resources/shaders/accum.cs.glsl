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
// [1]: https://medium.com/@raphlinus/inside-the-fastest-font-renderer-in-the-world-75ae5270c445

#version 330
#extension GL_ARB_compute_shader : require
#extension GL_ARB_explicit_uniform_location : require
#extension GL_ARB_shader_image_load_store : require
#extension GL_ARB_shader_storage_buffer_object : require
#extension GL_ARB_shading_language_420pack : require

layout(local_size_x = 1024) in;

layout(r8, binding = 0) uniform restrict writeonly image2DRect uImage;
layout(r32f, binding = 1) uniform restrict readonly image2DRect uCoverage;
layout(location = 2) uniform uvec4 uAtlasRect;
layout(location = 3) uniform uint uAtlasShelfHeight;

void main() {
    // Determine the boundaries of the column we'll be traversing.
    uint atlasWidth = uAtlasRect.z - uAtlasRect.x;
    uint column = gl_GlobalInvocationID.x % atlasWidth;
    uint shelfIndex = gl_GlobalInvocationID.x / atlasWidth;
    uint firstRow = shelfIndex * uAtlasShelfHeight;
    uint lastRow = (shelfIndex + 1u) * uAtlasShelfHeight;

    uint atlasHeight = uAtlasRect.w - uAtlasRect.y;
    if (firstRow >= atlasHeight)
        return;

    // Sweep down the column, accumulating coverage as we go.
    float coverage = 0.0f;
    for (uint row = firstRow; row < lastRow; row++) {
        ivec2 coord = ivec2(column, row);
        coverage += imageLoad(uCoverage, coord).r;
        imageStore(uImage, coord + ivec2(uAtlasRect.xy), vec4(coverage, coverage, coverage, 1.0));
    }
}
