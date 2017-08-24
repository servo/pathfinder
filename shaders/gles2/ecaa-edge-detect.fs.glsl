// pathfinder/shaders/gles2/ecaa-edge-detect.fs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

uniform ivec2 uFramebufferSize;
uniform sampler2D uColor;
uniform sampler2D uPathID;

varying vec2 vTexCoord;

void checkFG(out vec2 fgPosition, out int fgPathID, vec2 queryPosition, int queryPathID) {
    if (queryPathID > fgPathID) {
        fgPosition = queryPosition;
        fgPathID = queryPathID;
    }
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

    // Determine the topmost path.
    int centerPathID = unpackPathID(texture2D(uPathID, position).rg);
    ivec4 neighborPathIDs = ivec4(unpackPathID(texture2D(uPathID, positionL).rg),
                                  unpackPathID(texture2D(uPathID, positionR).rg),
                                  unpackPathID(texture2D(uPathID, positionB).rg),
                                  unpackPathID(texture2D(uPathID, positionT).rg));

    // Determine the position of the foreground color.
    vec2 fgPosition = position;
    int fgPathID = centerPathID;
    checkFG(fgPosition, fgPathID, positionL, neighborPathIDs.x);
    checkFG(fgPosition, fgPathID, positionR, neighborPathIDs.y);
    checkFG(fgPosition, fgPathID, positionB, neighborPathIDs.z);
    checkFG(fgPosition, fgPathID, positionT, neighborPathIDs.w);

    // Determine the position of the background color.
    vec2 bgPosition;
    if (fgPathID != centerPathID)
        bgPosition = position;
    else if (fgPathID != neighborPathIDs.x)
        bgPosition = positionL;
    else if (fgPathID != neighborPathIDs.y)
        bgPosition = positionR;
    else if (fgPathID != neighborPathIDs.z)
        bgPosition = positionB;
    else
        bgPosition = positionT;

    // Determine the foreground and background colors.
    vec4 fgColor = texture2D(uColor, fgPosition);
    vec4 bgColor = texture2D(uColor, bgPosition);

    // Determine the depth.
    //
    // If all colors are the same, avoid touching this pixel in any further passes.
    float outDepth = fgColor == bgColor ? -1.0 : convertPathIndexToWindowDepthValue(fgPathID);

    // Output results.
    gl_FragData[0] = bgColor;
    gl_FragData[1] = fgColor;
    gl_FragDepthEXT = outDepth;
}
