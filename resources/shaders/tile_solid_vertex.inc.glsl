// pathfinder/resources/shaders/tile_solid_vertex.inc.glsl
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
uniform mat4 uRasterTransform;

in vec2 aTessCoord;
in vec2 aTileOrigin;
in uint aObject;

out vec4 vColor;

vec4 getFillColor(uint object);

void computeVaryings() {
    vec2 pixelPosition = (aTileOrigin + aTessCoord) * uTileSize + uViewBoxOrigin;
    vec2 position2D = (pixelPosition / uFramebufferSize * 2.0 - 1.0) * vec2(1.0, -1.0);
    vec4 position = uRasterTransform * vec4(position2D, 0.0, 1.0);

    vColor = getFillColor(aObject);
    gl_Position = position;
}
