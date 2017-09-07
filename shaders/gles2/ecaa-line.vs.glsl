// pathfinder/shaders/gles2/ecaa-line.vs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

uniform mat4 uTransform;
uniform float uScaleX;
uniform ivec2 uFramebufferSize;
uniform ivec2 uBVertexPositionDimensions;
uniform ivec2 uBVertexPathIDDimensions;
uniform ivec2 uPathTransformDimensions;
uniform sampler2D uBVertexPosition;
uniform sampler2D uBVertexPathID;
uniform sampler2D uPathTransform;
uniform bool uLowerPart;

attribute vec2 aQuadPosition;
attribute vec4 aLineIndices;

varying vec4 vEndpoints;

void main() {
    // Fetch B-vertex positions.
    ivec2 pointIndices = ivec2(unpackUInt32Attribute(aLineIndices.xy),
                               unpackUInt32Attribute(aLineIndices.zw));
    vec2 leftPosition = fetchFloat2Data(uBVertexPosition,
                                        pointIndices.x,
                                        uBVertexPositionDimensions);
    vec2 rightPosition = fetchFloat2Data(uBVertexPosition,
                                         pointIndices.y,
                                         uBVertexPositionDimensions);

    int pathID = fetchUInt16Data(uBVertexPathID, pointIndices.x, uBVertexPathIDDimensions);

    vec4 transform = fetchFloat4Data(uPathTransform, pathID, uPathTransformDimensions);
    transform.xz *= uScaleX;

    // Transform the points, and compute the position of this vertex.
    vec2 position;
    computeQuadPosition(position,
                        leftPosition,
                        rightPosition,
                        aQuadPosition,
                        uFramebufferSize,
                        transform);

    float depth = convertPathIndexToViewportDepthValue(pathID);

    gl_Position = vec4(position, depth, 1.0);
    vEndpoints = vec4(leftPosition, rightPosition);
}
