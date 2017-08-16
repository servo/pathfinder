// pathfinder/shaders/gles2/direct-interior.vs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

uniform mat4 uTransform;
uniform ivec2 uFramebufferSize;
uniform ivec2 uPathColorsDimensions;
uniform sampler2D uPathColors;

attribute vec2 aPosition;
attribute float aPathID;

varying vec4 vColor;
varying vec2 vPathID;

void main() {
    int pathID = int(aPathID);

    vec2 position = transformVertexPosition(aPosition, uTransform);
    position = convertScreenToClipSpace(position, uFramebufferSize);
    float depth = convertPathIndexToDepthValue(pathID);
    gl_Position = vec4(position, depth, 1.0);

    vColor = fetchFloat4Data(uPathColors, pathID, uPathColorsDimensions);
    vPathID = packPathID(pathID);
}
