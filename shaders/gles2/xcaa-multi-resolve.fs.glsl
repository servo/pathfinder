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
uniform sampler2D uBGFGPathID;
uniform sampler2D uAAAlpha;
uniform sampler2D uPathColors;

varying vec2 vTexCoord;

void main() {
    vec4 packedPathIDsBGFG = texture2D(uBGFGPathID, vTexCoord);
    int pathIDBG = unpackPathID(packedPathIDsBGFG.xy);
    int pathIDFG = unpackPathID(packedPathIDsBGFG.zw);
    vec4 bgColor = fetchFloat4Data(uPathColors, pathIDBG, uPathColorsDimensions);
    vec4 fgColor = fetchFloat4Data(uPathColors, pathIDFG, uPathColorsDimensions);
    float alpha = clamp(texture2D(uAAAlpha, vTexCoord).r, 0.0, 1.0);
    gl_FragColor = mix(bgColor, fgColor, alpha);
    //gl_FragColor = vec4(vec3(alpha), 1.0);
    //gl_FragColor = bgColor != fgColor ? vec4(1.0, 0.0, 0.0, 1.0) : vec4(1.0);
}
