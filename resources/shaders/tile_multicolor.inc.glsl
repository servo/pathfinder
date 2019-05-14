// pathfinder/resources/shaders/tile_multicolor.inc.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

uniform sampler2D uPaintTexture;
uniform vec2 uPaintTextureSize;

vec2 computePaintTexCoord(uint object, vec2 textureSize) {
    uint width = uint(textureSize.x);
    return (vec2(float(object % width), float(object / width)) + vec2(0.5)) / textureSize;
}

vec4 getColor(uint object) {
    vec2 colorTexCoord = computePaintTexCoord(object, uPaintTextureSize);
    return texture(uPaintTexture, colorTexCoord);
}
