#version 300 es

// pathfinder/demo2/stencil.vs.glsl
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform vec2 uFramebufferSize;
uniform vec2 uTileSize;

in vec2 aTessCoord;
in vec2 aFrom;
in vec2 aTo;
in uint aTileIndex;

out vec2 vFrom;
out vec2 vCtrl;
out vec2 vTo;

vec2 computeTileOffset(uint tileIndex, float stencilTextureWidth) {
    uint tilesPerRow = uint(stencilTextureWidth / uTileSize.x);
    uvec2 tileOffset = uvec2(aTileIndex % tilesPerRow, aTileIndex / tilesPerRow);
    return vec2(tileOffset) * uTileSize;
}

void main() {
    vec2 tileOrigin = computeTileOffset(aTileIndex, uFramebufferSize.x);

    vec2 from = aFrom, ctrl = mix(aFrom, aTo, 0.5), to = aTo;

    vec2 dilation, position;
    bool zeroArea = abs(from.x - to.x) < 0.01;
    if (aTessCoord.x < 0.5) {
        position.x = min(min(from.x, to.x), ctrl.x);
        dilation.x = zeroArea ? 0.0 : -1.0;
    } else {
        position.x = max(max(from.x, to.x), ctrl.x);
        dilation.x = zeroArea ? 0.0 : 1.0;
    }
    if (aTessCoord.y < 0.5) {
        position.y = min(min(from.y, to.y), ctrl.y);
        dilation.y = zeroArea ? 0.0 : -1.0;
    } else {
        position.y = uTileSize.y;
        dilation.y = 0.0;
    }
    position += dilation;

    vFrom = aFrom - position;
    vCtrl = mix(aFrom, aTo, 0.5) - position;
    vTo = aTo - position;

    gl_Position = vec4((tileOrigin + position) / uFramebufferSize * 2.0 - 1.0, 0.0, 1.0);
}
