// pathfinder/shaders/gles2/ecaa-resolve.fs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

uniform ivec2 uFramebufferSize;
uniform sampler2D uBGColor;
uniform sampler2D uFGColor;
uniform sampler2D uAAAlpha;

varying vec2 vTexCoord;

void main() {
    vec4 bgColor = texture2D(uBGColor, vTexCoord);
    vec4 fgColor = texture2D(uFGColor, vTexCoord);
    float alpha = texture2D(uAAAlpha, vTexCoord).a;
    gl_FragColor = mix(bgColor, alpha == 1.0 ? fgColor : vec4(1.0, 0.0, 0.0, 1.0), alpha);
}
