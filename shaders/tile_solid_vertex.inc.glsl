// pathfinder/shaders/tile_solid_vertex.inc.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

uniform mat4 uTransform;
uniform vec2 uTileSize;

in uvec2 aTessCoord;
in ivec2 aTileOrigin;

out vec4 vColor;

vec4 getColor();

void computeVaryings() {
    vec2 position = vec2(aTileOrigin + ivec2(aTessCoord)) * uTileSize;
    vColor = getColor();
    gl_Position = uTransform * vec4(position, 0.0, 1.0);
}
