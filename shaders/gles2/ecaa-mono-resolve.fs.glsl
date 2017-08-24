// pathfinder/shaders/gles2/ecaa-mono-resolve.fs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision mediump float;

uniform vec4 uBGColor;
uniform vec4 uFGColor;
uniform sampler2D uAAAlpha;

varying vec2 vTexCoord;

void main() {
    float alpha = clamp(texture2D(uAAAlpha, vTexCoord).r, 0.0, 1.0);
    gl_FragColor = mix(uBGColor, uFGColor, alpha);
}
