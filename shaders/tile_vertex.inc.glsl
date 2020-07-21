// pathfinder/shaders/tile_vertex.inc.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

vec4 fetchUnscaled(sampler2D srcTexture, vec2 scale, vec2 originCoord, int entry) {
    return texture(srcTexture, (originCoord + vec2(0.5) + vec2(entry, 0)) * scale);
}

void computeTileVaryings(vec2 position,
                         int colorEntry,
                         sampler2D textureMetadata,
                         ivec2 textureMetadataSize,
                         out vec2 outColorTexCoord0,
                         out vec4 outBaseColor,
                         out vec4 outFilterParams0,
                         out vec4 outFilterParams1,
                         out vec4 outFilterParams2,
                         out vec4 outFilterParams3,
                         out vec4 outFilterParams4,
                         out int outCtrl) {
    vec2 metadataScale = vec2(1.0) / vec2(textureMetadataSize);
    vec2 metadataEntryCoord = vec2(colorEntry % 128 * 10, colorEntry / 128);
    vec4 colorTexMatrix0 = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 0);
    vec4 colorTexOffsets = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 1);
    vec4 baseColor       = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 2);
    vec4 filterParams0   = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 3);
    vec4 filterParams1   = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 4);
    vec4 filterParams2   = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 5);
    vec4 filterParams3   = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 6);
    vec4 filterParams4   = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 7);
    vec4 extra           = fetchUnscaled(textureMetadata, metadataScale, metadataEntryCoord, 8);
    outColorTexCoord0 = mat2(colorTexMatrix0) * position + colorTexOffsets.xy;
    outBaseColor = baseColor;
    outFilterParams0 = filterParams0;
    outFilterParams1 = filterParams1;
    outFilterParams2 = filterParams2;
    outFilterParams3 = filterParams3;
    outFilterParams4 = filterParams4;
    outCtrl = int(extra.x);
}
