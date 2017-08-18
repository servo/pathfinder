// pathfinder/shaders/gles2/common.inc.glsl
//
// Copyright (c) 2017 Mozilla Foundation

#version 100

#extension GL_EXT_draw_buffers : require
#extension GL_EXT_frag_depth : require

#define MAX_PATHS   65536

#define EPSILON     0.001

precision highp float;

// https://stackoverflow.com/a/36078859
int imod(int ia, int ib) {
    float a = float(ia), b = float(ib);
    float m = a - floor((a + 0.5) / b) * b;
    return int(floor(m + 0.5));
}

vec2 transformVertexPosition(vec2 position, mat4 transform) {
    return (transform * vec4(position, 0.0, 1.0)).xy;
}

vec2 convertScreenToClipSpace(vec2 position, ivec2 framebufferSize) {
    return position / vec2(framebufferSize) * 2.0 - 1.0;
}

float convertPathIndexToDepthValue(int pathIndex) {
    return mix(-1.0, 1.0, float(pathIndex) / float(MAX_PATHS));
}

int unpackUInt16(vec2 packedValue) {
    ivec2 valueBytes = ivec2(floor(packedValue * 255.0));
    return valueBytes.y * 256 + valueBytes.x;
}

int unpackUInt32Attribute(vec2 packedValue) {
    ivec2 valueWords = ivec2(packedValue);
    return valueWords.y * 65536 + valueWords.x;
}

vec4 fetchFloat4Data(sampler2D dataTexture, int index, ivec2 dimensions) {
    ivec2 pixelCoord = ivec2(imod(index, dimensions.x), index / dimensions.x);
    return texture2D(dataTexture, (vec2(pixelCoord) + 0.5) / vec2(dimensions));
}

vec4 fetchFloat4NormIndexedData(sampler2D dataTexture, float normIndex, ivec2 dimensions) {
    return fetchFloat4Data(dataTexture, int(normIndex * float(dimensions.x)), dimensions);
}

vec2 fetchFloat2Data(sampler2D dataTexture, int index, ivec2 dimensions) {
    vec4 float4Data = fetchFloat4Data(dataTexture, index / 2, dimensions);
    return index / 2 * 2 == index ? float4Data.xy : float4Data.zw;
}

int fetchUInt16Data(sampler2D dataTexture, int index, ivec2 dimensions) {
    return unpackUInt16(fetchFloat2Data(dataTexture, index, dimensions));
}

vec2 packPathID(int pathID) {
    return vec2(imod(pathID, 256), pathID / 256) / 255.0;
}

int unpackPathID(vec2 packedPathID) {
    return unpackUInt16(packedPathID);
}

bool xor(bool a, bool b) {
    return (a && !b) || (!a && b);
}

float det2(vec2 a, vec2 b) {
    return a.x * b.y - b.x * a.y;
}
