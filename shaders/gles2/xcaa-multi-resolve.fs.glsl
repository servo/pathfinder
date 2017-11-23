// pathfinder/shaders/gles2/xcaa-multi-resolve.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform ivec2 uPathColorsDimensions;
uniform sampler2D uAAAlpha;
uniform sampler2D uAADepth;
uniform sampler2D uPathColors;

varying vec2 vTexCoord;

void main() {
    float edgeDepth = texture2D(uAADepth, vTexCoord).r;
    int edgePathID = convertWindowDepthValueToPathIndex(edgeDepth);
    vec4 edgeColor = fetchFloat4Data(uPathColors, edgePathID, uPathColorsDimensions);
    float edgeAlpha = abs(texture2D(uAAAlpha, vTexCoord).r);
    gl_FragColor = vec4(edgeColor.rgb, edgeColor.a * edgeAlpha);
    gl_FragDepthEXT = edgeDepth;
}
