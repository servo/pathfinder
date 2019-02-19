#version 330

// pathfinder/demo/resources/shaders/solid_tile.vs.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform vec2 uFramebufferSize;
uniform vec2 uTileSize;
uniform sampler2D uFillColorsTexture;
uniform vec2 uFillColorsTextureSize;
uniform vec2 uViewBoxOrigin;
uniform vec3 uQuadP0;
uniform vec3 uQuadP1;
uniform vec3 uQuadP2;
uniform vec3 uQuadP3;

in vec2 aTessCoord;
in vec2 aTileOrigin;
in uint aObject;

out vec4 vColor;

float wedge(vec2 a, vec2 b) {
    return a.x * b.y - a.y * b.x;
}

// From "A Quadrilateral Rendering Primitive", Hormann and Tarini 2004.
vec4 barycentricQuad(vec2 p) {
    vec2 s0 = uQuadP0.xy - p, s1 = uQuadP1.xy - p, s2 = uQuadP2.xy - p, s3 = uQuadP3.xy - p;
    vec4 a = vec4(wedge(s0, s1), wedge(s1, s2), wedge(s2, s3), wedge(s3, s0));
    vec4 d = vec4(dot(s0, s1),   dot(s1, s2),   dot(s2, s3),   dot(s3, s0));
    vec4 r = vec4(length(s0),    length(s1),    length(s2),    length(s3));
    vec4 t = (r * r.yzwx - d) / a;
    vec4 u = (t.wxyz + t) / r;
    return u / dot(u, vec4(1.0));
}

vec2 computeFillColorTexCoord(uint object, vec2 textureSize) {
    uint width = uint(textureSize.x);
    return (vec2(float(object % width), float(object / width)) + vec2(0.5)) / textureSize;
}

void main() {
    vec2 pixelPosition = (aTileOrigin + aTessCoord) * uTileSize + uViewBoxOrigin;
    vec2 position = (pixelPosition / uFramebufferSize * 2.0 - 1.0) * vec2(1.0, -1.0);

    //vec4 depths = vec4(uQuadP0.z, uQuadP1.z, uQuadP2.z, uQuadP3.z);
    //float depth = dot(barycentricQuad(position), depths);

    vec4 lambda = barycentricQuad(position);
    float red = lambda.x;
    float green = lambda.y;
    float blue = lambda.z;
    float depth = 0.0;

    vec2 colorTexCoord = computeFillColorTexCoord(aObject, uFillColorsTextureSize);

    //vColor = texture(uFillColorsTexture, colorTexCoord);
    vColor = vec4(red, green, blue, 1.0);
    gl_Position = vec4(position, depth, 1.0);
}
