#version {{version}}

// pathfinder/resources/shaders/reproject.vs.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform mat2 uTransform;
uniform vec2 uTranslation;

in vec2 aPosition;

out vec2 vTexCoord;

void main() {
    vec2 normPosition = aPosition * 2.0 - 1.0;
    vec2 position = uTransform * normPosition + uTranslation;
    vTexCoord = normPosition;
    gl_Position = vec4(position, 0.0, 1.0);
}
