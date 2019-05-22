// pathfinder/resources/shaders/tile_multicolor.inc.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

uniform sampler2D uPaintMetadataTexture;

vec2 getPaintTexCoord(vec2 tessCoord, uint objectIndex) {
    vec4 texRect = texelFetch(uPaintMetadataTexture, ivec2(0, int(objectIndex)), 0);
    return mix(texRect.xy, texRect.zw, tessCoord);
}
