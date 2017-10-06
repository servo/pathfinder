// pathfinder/shaders/gles2/ecaa-cover.vs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform vec4 uTransformST;
uniform vec4 uHints;
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

    upperLeftPosition.y = min(upperLeftPosition.y, upperRightPosition.y);
    lowerRightPosition.y = max(lowerLeftPosition.y, lowerRightPosition.y);

    int pathID = fetchUInt16Data(uBVertexPathID, pointIndices.x, uBVertexPathIDDimensions);

    vec4 transform = fetchFloat4Data(uPathTransform, pathID, uPathTransformDimensions);

    upperLeftPosition = hintPosition(upperLeftPosition, uHints);
    lowerRightPosition = hintPosition(lowerRightPosition, uHints);

    upperLeftPosition = transformVertexPositionST(upperLeftPosition, transform);
    lowerRightPosition = transformVertexPositionST(lowerRightPosition, transform);

    upperLeftPosition = transformVertexPositionST(upperLeftPosition, uTransformST);
    lowerRightPosition = transformVertexPositionST(lowerRightPosition, uTransformST);

    upperLeftPosition = convertClipToScreenSpace(upperLeftPosition, uFramebufferSize);
    lowerRightPosition = convertClipToScreenSpace(lowerRightPosition, uFramebufferSize);

    vec4 roundedExtents = vec4(floor(upperLeftPosition), ceil(lowerRightPosition));

    vec2 position = mix(roundedExtents.xy, roundedExtents.zw, aQuadPosition);
    position = convertScreenToClipSpace(position, uFramebufferSize);
    float depth = convertPathIndexToViewportDepthValue(pathID);
    gl_Position = vec4(position, depth, 1.0);

    vHorizontalExtents = vec2(upperLeftPosition.x, lowerRightPosition.x);
}
