// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#version 330

#define MAX_GLYPHS  2048

// Accessors to work around Apple driver bugs.
#define GLYPH_DESCRIPTOR_UNITS_PER_EM(d)    (d).misc.x
#define IMAGE_DESCRIPTOR_ATLAS_POS(d)       (d).xy
#define IMAGE_DESCRIPTOR_POINT_SIZE(d)      (d).z

// Information about the metrics of each glyph.
struct GlyphDescriptor {
    // The left/bottom/right/top offsets of the glyph from point (0, 0) in glyph space.
    ivec4 extents;
    // x: Units per em.
    uvec4 misc;
};

// The size of the atlas in pixels.
uniform uvec2 uAtlasSize;

// Whether subpixel antialiasing is in use.
uniform bool uSubpixelAA;

layout(std140) uniform ubGlyphDescriptors {
    GlyphDescriptor uGlyphs[MAX_GLYPHS];
};

layout(std140) uniform ubImageDescriptors {
    vec4 uImages[MAX_GLYPHS];
};

// The position of each vertex in glyph space.
in ivec2 aPosition;

// Which glyph the vertex belongs to.
in uint aGlyphIndex;

// The vertex ID, passed along onto the TCS.
flat out int vVertexID;

void main() {
    vVertexID = gl_VertexID;

    vec4 image = uImages[aGlyphIndex];
    GlyphDescriptor glyph = uGlyphs[aGlyphIndex];

    vec2 glyphPos = vec2(aPosition.x - glyph.extents.x, glyph.extents.w - aPosition.y);
    float pointSize = IMAGE_DESCRIPTOR_POINT_SIZE(image);
    vec2 glyphPxPos = glyphPos * pointSize / GLYPH_DESCRIPTOR_UNITS_PER_EM(glyph);
    vec2 atlasPos = glyphPxPos + IMAGE_DESCRIPTOR_ATLAS_POS(image);
    if (uSubpixelAA)
        atlasPos.x *= 3.0f;

    gl_Position = vec4(atlasPos, 0.0f, 1.0f);
}

