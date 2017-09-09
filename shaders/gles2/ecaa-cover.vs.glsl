// pathfinder/shaders/gles2/ecaa-cover.vs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

uniform ivec2 uFramebufferSize;
uniform float uScaleX;
uniform ivec2 uBVertexPositionDimensions;
uniform ivec2 uBVertexPathIDDimensions;
uniform ivec2 uPathTransformDimensions;
uniform ivec2 uPathHintsDimensions;
uniform sampler2D uBVertexPosition;
uniform sampler2D uBVertexPathID;
uniform sampler2D uPathTransform;
uniform sampler2D uPathHints;

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

    vec4 hints = fetchFloat4Data(uPathHints, pathID, uPathHintsDimensions);
    vec4 transform = fetchFloat4Data(uPathTransform, pathID, uPathTransformDimensions);
    transform.xz *= uScaleX;

    upperLeftPosition = hintPosition(upperLeftPosition, hints);
    upperRightPosition = hintPosition(upperRightPosition, hints);
    lowerLeftPosition = hintPosition(lowerLeftPosition, hints);
    lowerRightPosition = hintPosition(lowerRightPosition, hints);

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

    vec2 position = mix(roundedExtents.xy, roundedExtents.zw, aQuadPosition);
    position = convertScreenToClipSpace(position, uFramebufferSize);
    float depth = convertPathIndexToViewportDepthValue(pathID);
    gl_Position = vec4(position, depth, 1.0);

    vHorizontalExtents = extents.xz;
}
