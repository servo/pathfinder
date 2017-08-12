// pathfinder/shaders/gles2/common.inc.glsl
//
// Copyright (c) 2017 Mozilla Foundation

#define MAX_PATHS   65536

vec2 transformVertexPosition(vec2 position, mat4 transform) {
    return (transform * vec4(position, 0.0, 1.0)).xy;
}

vec2 convertScreenToClipSpace(vec2 position, ivec2 framebufferSize) {
    return position / vec2(framebufferSize) * 2.0 - 1.0;
}

float convertPathIndexToDepthValue(int pathIndex) {
    return float(pathIndex + 1) / float(MAX_PATHS);
}

vec4 fetchFloat4Data(sampler2D dataTexture, int index, ivec2 dimensions) {
    return texture2D(dataTexture, (float(index) + 0.5) / vec2(dimensions));
}
