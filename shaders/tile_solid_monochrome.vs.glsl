#version 330

// pathfinder/shaders/tile_solid_monochrome.vs.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#extension GL_GOOGLE_include_directive : enable

precision highp float;

#include "tile_solid_vertex.inc.glsl"
#include "tile_monochrome.inc.glsl"

void main() {
    computeVaryings();
}
