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

// FIXME(pcwalton): This should be higher. Dynamically query its maximum possible size, perhaps?
#define MAX_GLYPHS  256

// Information about the metrics of each glyph.
layout(std140) struct GlyphDescriptor {
    // The left/top/right/bottom offsets of the glyph from point (0, 0) in glyph space.
    ivec4 extents;
    // The number of units per em in this glyph.
    uint unitsPerEm;
    // The index of the first point in the VBO.
    uint startPoint;
    // The index of the first element in the IBO.
    uint startIndex;
};

// Information about the position of each glyph in the atlas.
layout(std140) struct ImageDescriptor {
    // The left/top/right/bottom positions of the glyph in the atlas.
    uvec4 atlasRect;
    // The font size in pixels.
    float pointSize;
    // The index of the glyph.
    uint glyphIndex;
};

// The size of the atlas in pixels.
uniform uvec2 uAtlasSize;

layout(std140) uniform ubGlyphDescriptors {
    GlyphDescriptor uGlyphs[MAX_GLYPHS];
};

layout(std140) uniform ubImageDescriptors {
    ImageDescriptor uImages[MAX_GLYPHS];
};

// The position of each vertex in glyph space.
in ivec2 aPosition;

// Which glyph the vertex belongs to.
//
// TODO(pcwalton): See if this is faster as a binary search on the vertex ID.
in uint aGlyphIndex;

// The vertex ID, passed along onto the TCS.
flat out uint vVertexID;

void main() {
    vVertexID = gl_VertexID;

    ImageDescriptor image = uImages[aGlyphIndex];
    GlyphDescriptor glyph = uGlyphs[aGlyphIndex];

    float emsPerUnit = 1.0f / float(glyph.unitsPerEm);

    vec2 glyphPos = vec2(aPosition.x - glyph.extents.x, glyph.extents.w - aPosition.y);
    vec2 atlasPos = glyphPos * emsPerUnit * image.pointSize + vec2(image.atlasRect.xy);

    gl_Position = vec4(atlasPos, 0.0f, 1.0f);
}

