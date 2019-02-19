#version 330

// pathfinder/demo/resources/shaders/solid_tile.vs.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform vec2 uFramebufferSize;
uniform vec2 uTileSize;
uniform sampler2D uFillColorsTexture;
uniform vec2 uFillColorsTextureSize;
uniform vec2 uViewBoxOrigin;

in vec2 aTessCoord;
in vec2 aTileOrigin;
in uint aObject;

out vec4 vColor;

vec2 computeFillColorTexCoord(uint object, vec2 textureSize) {
    uint width = uint(textureSize.x);
    return (vec2(float(object % width), float(object / width)) + vec2(0.5)) / textureSize;
}

void main() {
    vec2 pixelPosition = (aTileOrigin + aTessCoord) * uTileSize + uViewBoxOrigin;
    vec2 position = (pixelPosition / uFramebufferSize * 2.0 - 1.0) * vec2(1.0, -1.0);
    vec2 colorTexCoord = computeFillColorTexCoord(aObject, uFillColorsTextureSize);

    vColor = texture(uFillColorsTexture, colorTexCoord);
    gl_Position = vec4(position, 0.0, 1.0);
}
