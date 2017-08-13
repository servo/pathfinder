// pathfinder/shaders/gles2/direct-curve.vs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

uniform mat4 uTransform;
uniform ivec2 uFramebufferSize;
uniform ivec2 uPathColorsDimensions;
uniform sampler2D uPathColors;

attribute vec2 aPosition;
attribute vec2 aTexCoord;
attribute float aPathDepth;
attribute float aSign;

varying vec4 vColor;
varying vec2 vTexCoord;
varying float vSign;

void main() {
    vec2 position = transformVertexPosition(aPosition, uTransform);
    position = convertScreenToClipSpace(position, uFramebufferSize);
    gl_Position = vec4(position, aPathDepth, 1.0);

    vColor = fetchFloat4NormIndexedData(uPathColors, aPathDepth, uPathColorsDimensions);
    vTexCoord = vec2(aTexCoord) / 2.0;
    vSign = aSign;
}
