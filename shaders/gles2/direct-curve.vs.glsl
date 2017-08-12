// pathfinder/shaders/gles2/direct-curve.vs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

#version 100

precision highp float;

uniform mat4 uTransform;
uniform ivec2 uFramebufferSize;
uniform ivec2 uPathColorsDimensions;
uniform sampler2D uPathColors;

attribute vec2 aPosition;
attribute ivec2 aTexCoord;
attribute int aKind;

varying vec4 vColor;
varying vec2 vTexCoord;
varying float vSign;

void main() {
    vec2 position = transformVertexPosition(aPosition, uTransform);
    position = convertScreenToClipSpace(position, uFramebufferSize);
    float depth = convertPathIndexToDepthValue(aPathIndex);
    gl_Position = vec4(position, depth, 1.0);

    vColor = fetchFloat4Data(uPathColors, aPathIndex, uPathColorsDimensions);
    vTexCoord = vec2(aTexCoord) / 2.0;

    switch (aKind) {
    case PF_B_VERTEX_KIND_CONVEX_CONTROL_POINT:
        vSign = -1.0;
        break;
    case PF_B_VERTEX_KIND_CONCAVE_CONTROL_POINT:
        vSign = 1.0;
        break;
    default:
        vSign = 0.0;
    }
}
