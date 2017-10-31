// pathfinder/shaders/gles2/xcaa-multi-direct-interior.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform ivec2 uFramebufferSize;
uniform sampler2D uEdgeDepth;

varying vec4 vColor;

void main() {
    vec2 center = floor(gl_FragCoord.xy);
    float depth = gl_FragCoord.z;
    vec2 texCoord = floor(center) / vec2(uFramebufferSize);

    // TODO(pcwalton): Get back early Z somehow?
    if (depth == texture2D(uEdgeDepth, texCoord).r)
        discard;

    gl_FragColor = vColor;
}
