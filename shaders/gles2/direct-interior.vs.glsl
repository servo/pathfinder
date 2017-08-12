// pathfinder/shaders/gles2/direct-interior.vs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

#version 100

precision highp float;

uniform mat4 uTransform;
uniform ivec2 uFramebufferSize;
uniform int uMaxTextureSize;
uniform ivec2 uPathColorsDimensions;
uniform sampler2D uPathColors;

attribute vec2 aPosition;
attribute int aPathIndex;

varying vec4 vColor;

void main() {
    vec2 position = transformVertexPosition(aPosition, uTransform);
    position = convertScreenToClipSpace(position, uFramebufferSize);
    float depth = convertPathIndexToDepthValue(aPathIndex);
    gl_Position = vec4(position, depth, 1.0);

    vColor = fetchFloat4Data(uPathColors, aPathIndex, uPathColorsDimensions);
}
