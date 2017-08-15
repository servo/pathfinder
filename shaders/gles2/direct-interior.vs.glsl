// pathfinder/shaders/gles2/direct-interior.vs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

uniform mat4 uTransform;
uniform ivec2 uFramebufferSize;
uniform ivec2 uPathColorsDimensions;
uniform sampler2D uPathColors;

attribute vec2 aPosition;
attribute float aPathDepth;

varying vec4 vColor;

void main() {
    vec2 position = transformVertexPosition(aPosition, uTransform);
    position = convertScreenToClipSpace(position, uFramebufferSize);
    gl_Position = vec4(position, aPathDepth, 1.0);

    vColor = fetchFloat4NormIndexedData(uPathColors, aPathDepth, uPathColorsDimensions);
}
