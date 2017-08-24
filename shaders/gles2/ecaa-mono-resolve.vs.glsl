// pathfinder/shaders/gles2/ecaa-mono-resolve.vs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

uniform vec4 uTransformST;
uniform vec2 uTexScale;

attribute vec2 aPosition;
attribute vec2 aTexCoord;

varying vec2 vTexCoord;

void main() {
    gl_Position = vec4(transformVertexPositionST(aPosition, uTransformST), -1.0, 1.0);
    vTexCoord = aTexCoord * uTexScale;
}
