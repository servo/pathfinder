// pathfinder/shaders/gles2/ecaa-cover.fs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

varying vec2 vHorizontalExtents;

void main() {
    vec2 sides = gl_FragCoord.xx + vec2(-0.5, 0.5);
    vec2 clampedSides = clamp(vHorizontalExtents, sides.x, sides.y);
    gl_FragColor = vec4(vec3(clampedSides.y - clampedSides.x), 1.0);
}
