// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#version 410

// Information about the position of each glyph in the atlas.
layout(std140) struct ImageInfo {
    // The left/top/right/bottom positions of the glyph in the atlas.
    uvec4 atlasRect;
    // The left/top/right/bottom offsets of the glyph from point (0, 0) in glyph space.
    ivec4 extents;
    // The font size in pixels.
    float pointSize;
};

// The size of the atlas in pixels.
uniform uvec2 uAtlasSize;

// The number of ems per unit (reciprocal of units per em).
uniform float uEmsPerUnit;

layout(std140) uniform ubImageInfo {
    ImageInfo uImageInfo[256];
};

// The position of each vertex in glyph space.
in ivec2 aPosition;

// Which image the vertex belongs to.
//
// TODO(pcwalton): See if this is faster as a binary search on the vertex ID.
in uint aImageIndex;

// The vertex ID, passed along onto the TCS.
flat out uint vVertexID;

void main() {
    vVertexID = gl_VertexID;

    ImageInfo imageInfo = uImageInfo[aImageIndex];
    vec2 glyphPos = vec2(aPosition.x - imageInfo.extents.x, imageInfo.extents.w - aPosition.y);
    vec2 atlasPos = glyphPos * uEmsPerUnit * imageInfo.pointSize + vec2(imageInfo.atlasRect.xy);
    gl_Position = vec4(atlasPos, 0.0f, 1.0f);
}

