#version 330

// pathfinder/shaders/tile.vs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#extension GL_GOOGLE_include_directive : enable

precision highp float;

#ifdef GL_ES
precision highp sampler2D;
#endif

#include "tile_vertex.inc.glsl"

uniform mat4 uTransform;
uniform vec2 uTileSize;
uniform sampler2D uTextureMetadata;
uniform ivec2 uTextureMetadataSize;
uniform sampler2D uZBuffer;
uniform ivec2 uZBufferSize;

in ivec2 aTileOffset;
in ivec2 aTileOrigin;
in uvec4 aMaskTexCoord0;
in ivec2 aCtrlBackdrop;
in int aPathIndex;
in int aColor;

out vec3 vMaskTexCoord0;
out vec2 vColorTexCoord0;
out vec4 vBaseColor;
out float vTileCtrl;
out vec4 vFilterParams0;
out vec4 vFilterParams1;
out vec4 vFilterParams2;
out vec4 vFilterParams3;
out vec4 vFilterParams4;
out float vCtrl;

void main() {
    vec2 tileOrigin = vec2(aTileOrigin), tileOffset = vec2(aTileOffset);
    vec2 position = (tileOrigin + tileOffset) * uTileSize;

    ivec4 zValue = ivec4(texture(uZBuffer, (tileOrigin + vec2(0.5)) / vec2(uZBufferSize)) * 255.0);
    if (aPathIndex < (zValue.x | (zValue.y << 8) | (zValue.z << 16) | (zValue.w << 24))) {
        gl_Position = vec4(0.0);
        return;
    }

    uvec2 maskTileCoord = uvec2(aMaskTexCoord0.x, aMaskTexCoord0.y + 256u * aMaskTexCoord0.z);
    vec2 maskTexCoord0 = (vec2(maskTileCoord) + tileOffset) * uTileSize;
    if (aCtrlBackdrop.y == 0 && aMaskTexCoord0.w != 0u) {
        gl_Position = vec4(0.0);
        return;
    }

    int ctrl;
    computeTileVaryings(position,
                        aColor,
                        uTextureMetadata,
                        uTextureMetadataSize,
                        vColorTexCoord0,
                        vBaseColor,
                        vFilterParams0,
                        vFilterParams1,
                        vFilterParams2,
                        vFilterParams3,
                        vFilterParams4,
                        ctrl);

    vTileCtrl = float(aCtrlBackdrop.x);
    vCtrl = float(ctrl);
    vMaskTexCoord0 = vec3(maskTexCoord0, float(aCtrlBackdrop.y));
    gl_Position = uTransform * vec4(position, 0.0, 1.0);
}
