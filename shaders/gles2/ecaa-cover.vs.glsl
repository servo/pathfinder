// pathfinder/shaders/gles2/ecaa-cover.vs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

uniform ivec2 uFramebufferSize;
uniform ivec2 uBVertexPositionDimensions;
uniform ivec2 uBVertexPathIDDimensions;
uniform ivec2 uPathTransformDimensions;
uniform sampler2D uBVertexPosition;
uniform sampler2D uBVertexPathID;
uniform sampler2D uPathTransform;

attribute vec2 aQuadPosition;
attribute vec4 aUpperPointIndices;
attribute vec4 aLowerPointIndices;

varying vec2 vHorizontalExtents;

void main() {
    // Fetch B-vertex positions.
    // FIXME(pcwalton): This could be slightly optimized to fetch fewer positions.
    ivec4 pointIndices = ivec4(unpackUInt32Attribute(aUpperPointIndices.xy),
                               unpackUInt32Attribute(aUpperPointIndices.zw),
                               unpackUInt32Attribute(aLowerPointIndices.xy),
                               unpackUInt32Attribute(aLowerPointIndices.zw));
    vec2 upperLeftPosition = fetchFloat2Data(uBVertexPosition,
                                             pointIndices.x,
                                             uBVertexPositionDimensions);
    vec2 upperRightPosition = fetchFloat2Data(uBVertexPosition,
                                              pointIndices.y,
                                              uBVertexPositionDimensions);
    vec2 lowerLeftPosition = fetchFloat2Data(uBVertexPosition,
                                             pointIndices.z,
                                             uBVertexPositionDimensions);
    vec2 lowerRightPosition = fetchFloat2Data(uBVertexPosition,
                                              pointIndices.w,
                                              uBVertexPositionDimensions);

    int pathID = fetchUInt16Data(uBVertexPathID, pointIndices.x, uBVertexPathIDDimensions);

    vec4 transform = fetchFloat4Data(uPathTransform, pathID, uPathTransformDimensions);

    upperLeftPosition = transformVertexPositionST(upperLeftPosition, transform);
    upperRightPosition = transformVertexPositionST(upperRightPosition, transform);
    lowerLeftPosition = transformVertexPositionST(lowerLeftPosition, transform);
    lowerRightPosition = transformVertexPositionST(lowerRightPosition, transform);

    vec4 extents = vec4(min(upperLeftPosition.x, lowerLeftPosition.x),
                        min(min(upperLeftPosition.y, upperRightPosition.y),
                            min(lowerLeftPosition.y, lowerRightPosition.y)),
                        max(upperRightPosition.x, lowerRightPosition.x),
                        max(max(upperLeftPosition.y, upperRightPosition.y),
                            max(lowerLeftPosition.y, lowerRightPosition.y)));

    vec4 roundedExtents = vec4(floor(extents.xy), ceil(extents.zw));

    // FIXME(pcwalton): Use a separate VBO for this.
    vec2 quadPosition = (aQuadPosition + 1.0) * 0.5;

    vec2 position = mix(roundedExtents.xy, roundedExtents.zw, quadPosition);
    position = convertScreenToClipSpace(position, uFramebufferSize);
    float depth = convertPathIndexToDepthValue(pathID);
    gl_Position = vec4(position, depth, 1.0);

    vHorizontalExtents = extents.xz;
}
