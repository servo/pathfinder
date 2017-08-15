// pathfinder/shaders/gles2/blit.fs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision lowp float;

uniform sampler2D uSource;

varying vec2 vTexCoord;

void main() {
    gl_FragColor = texture2D(uSource, vTexCoord);
}
