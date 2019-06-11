// pathfinder/shaders/tile_solid_vertex.inc.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

uniform vec2 uFramebufferSize;
uniform vec2 uTileSize;
uniform vec2 uViewBoxOrigin;

in ivec2 aTessCoord;
in ivec2 aTileOrigin;

out vec4 vColor;

vec4 getColor();

void computeVaryings() {
    vec2 pixelPosition = vec2(aTileOrigin + aTessCoord) * uTileSize + uViewBoxOrigin;
    vec2 position = (pixelPosition / uFramebufferSize * 2.0 - 1.0) * vec2(1.0, -1.0);

    vColor = getColor();
    gl_Position = vec4(position, 0.0, 1.0);
}
