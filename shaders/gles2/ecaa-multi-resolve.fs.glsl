// pathfinder/shaders/gles2/ecaa-multi-resolve.fs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

uniform sampler2D uBGColor;
uniform sampler2D uFGColor;
uniform sampler2D uAAAlpha;

varying vec2 vTexCoord;

void main() {
    vec4 bgColor = texture2D(uBGColor, vTexCoord);
    vec4 fgColor = texture2D(uFGColor, vTexCoord);
    float alpha = clamp(texture2D(uAAAlpha, vTexCoord).r, 0.0, 1.0);
    gl_FragColor = mix(bgColor, fgColor, alpha);
}
