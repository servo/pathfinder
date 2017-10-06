// pathfinder/shaders/gles2/ecaa-line.vs.glsl
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

    // Transform the points, and compute the position of this vertex.
    vec2 position;
    computeQuadPosition(position,
                        leftPosition,
                        rightPosition,
                        aQuadPosition,
                        uFramebufferSize,
                        transform,
                        uTransformST,
                        uHints);

    float depth = convertPathIndexToViewportDepthValue(pathID);

    gl_Position = vec4(position, depth, 1.0);
    vEndpoints = vec4(leftPosition, rightPosition);
}
