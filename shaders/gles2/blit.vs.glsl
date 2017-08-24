// pathfinder/shaders/gles2/blit.vs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision mediump float;

uniform mat4 uTransform;
uniform vec2 uTexScale;

attribute vec2 aPosition;
attribute vec2 aTexCoord;

varying vec2 vTexCoord;

void main() {
    gl_Position = uTransform * vec4(aPosition, 0.0, 1.0);
    vTexCoord = aTexCoord * uTexScale;
}
