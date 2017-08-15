// pathfinder/shaders/gles2/blit.vs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision lowp float;

attribute vec2 aPosition;
attribute vec2 aTexCoord;

varying vec2 vTexCoord;

void main() {
    gl_Position = vec4(aPosition, 0.0, 1.0);
    vTexCoord = aTexCoord;
}
