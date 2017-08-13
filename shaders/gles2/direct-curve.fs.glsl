// pathfinder/shaders/gles2/direct-curve.fs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

// This shader implements the quadratic Loop-Blinn formulation without writing precise depth.
// It is therefore unsuitable for ECAA, but it's fast (specifically, preserving early Z) and
// compatible with OpenGL ES 2.0.

precision highp float;

varying vec4 vColor;
varying vec2 vTexCoord;
varying float vSign;

void main() {
    float side = vTexCoord.x * vTexCoord.x - vTexCoord.y;
    float alpha = float(sign(side) == sign(vSign));
    gl_FragColor = vec4(vColor.rgb, vColor.a * alpha);
}
