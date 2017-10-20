// pathfinder/shaders/gles2/xcaa-edge-detect.fs.glsl
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
uniform ivec2 uPathColorsDimensions;
uniform sampler2D uPathID;
uniform sampler2D uPathColors;

varying vec2 vTexCoord;

void checkFG(out vec2 fgPosition, out int fgPathID, vec2 queryPosition, int queryPathID) {
    if (queryPathID > fgPathID) {
        fgPosition = queryPosition;
        fgPathID = queryPathID;
    }
}

void updateMinMaxInt(inout ivec2 minMax, int value) {
    if (value < minMax.x)
        minMax.x = value;
    if (value > minMax.y)
        minMax.y = value;
}

ivec2 minMaxIVec4(ivec4 values) {
    ivec2 minMax = ivec2(values.x);
    updateMinMaxInt(minMax, values.y);
    updateMinMaxInt(minMax, values.z);
    updateMinMaxInt(minMax, values.w);
    return minMax;
}

void main() {
    // Unpack.
    vec2 position = vTexCoord;

    // Compute positions.
    vec2 onePixel = 1.0 / vec2(uFramebufferSize);
    vec2 positionL = position + vec2(-onePixel.x, 0.0);
    vec2 positionR = position + vec2( onePixel.x, 0.0);
    vec2 positionB = position + vec2(0.0, -onePixel.y);
    vec2 positionT = position + vec2(0.0,  onePixel.y);

    // Determine the topmost and bottommost paths.
    int centerPathID = unpackPathID(texture2D(uPathID, position).rg);
    ivec4 neighborPathIDs = ivec4(unpackPathID(texture2D(uPathID, positionL).rg),
                                  unpackPathID(texture2D(uPathID, positionR).rg),
                                  unpackPathID(texture2D(uPathID, positionB).rg),
                                  unpackPathID(texture2D(uPathID, positionT).rg));
    ivec2 pathIDsBGFG = minMaxIVec4(neighborPathIDs);

    // Determine the depth.
    //
    // If all colors are the same, avoid touching this pixel in any further passes.
    float outDepth;
    if (pathIDsBGFG.x == pathIDsBGFG.y)
        outDepth = 1.0;
    else
        outDepth = convertPathIndexToWindowDepthValue(pathIDsBGFG.y);

    // FIXME(pcwalton): Fetch the background color.
    // FIXME(pcwalton): Output path ID for debugging. Switch to BG color.
    //vec2 color = pathIDsBGFG.x == pathIDsBGFG.y ? vec2(1.0) : packPathID(pathIDsBGFG.y);
    //vec4 color = fetchFloat4Data(uPathColors, pathIDsBGFG.x, uPathColorsDimensions);

    // Output results.
    //gl_FragColor = vec4(packPathID(pathIDsBGFG.x), 0.0, 1.0);
    gl_FragColor = vec4(packPathID(pathIDsBGFG.x), packPathID(pathIDsBGFG.y));
    gl_FragDepthEXT = outDepth;
}
