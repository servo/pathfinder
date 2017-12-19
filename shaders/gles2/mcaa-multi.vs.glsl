// pathfinder/shaders/gles2/mcaa-multi.vs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#define MAX_SLOPE   10.0

precision highp float;

uniform vec4 uTransformST;
uniform vec4 uHints;
uniform ivec2 uFramebufferSize;
uniform ivec2 uPathTransformSTDimensions;
uniform sampler2D uPathTransformST;
uniform ivec2 uPathColorsDimensions;
uniform sampler2D uPathColors;

attribute vec2 aQuadPosition;
attribute vec4 aUpperEndpointPositions;
attribute vec4 aLowerEndpointPositions;
attribute vec4 aControlPointPositions;
attribute float aPathID;

varying vec4 vUpperEndpoints;
varying vec4 vLowerEndpoints;
varying vec4 vControlPoints;
varying vec4 vColor;

void main() {
    vec2 tlPosition = aUpperEndpointPositions.xy;
    vec2 tcPosition = aControlPointPositions.xy;
    vec2 trPosition = aUpperEndpointPositions.zw;
    vec2 blPosition = aLowerEndpointPositions.xy;
    vec2 bcPosition = aControlPointPositions.zw;
    vec2 brPosition = aLowerEndpointPositions.zw;
    vec2 quadPosition = aQuadPosition;
    int pathID = int(aPathID);

    vec4 transformST = fetchFloat4Data(uPathTransformST, pathID, uPathTransformSTDimensions);

    vec4 color = fetchFloat4Data(uPathColors, pathID, uPathColorsDimensions);

    vec2 topVector = trPosition - tlPosition, bottomVector = brPosition - blPosition;

    float topSlope = topVector.y / topVector.x;
    float bottomSlope = bottomVector.y / bottomVector.x;
    if (abs(topSlope) > MAX_SLOPE)
        topSlope = sign(topSlope) * MAX_SLOPE;
    if (abs(bottomSlope) > MAX_SLOPE)
        bottomSlope = sign(bottomSlope) * MAX_SLOPE;

    // Transform the points, and compute the position of this vertex.
    tlPosition = computeMCAASnappedPosition(tlPosition,
                                            uHints,
                                            transformST,
                                            uTransformST,
                                            uFramebufferSize,
                                            topSlope);
    trPosition = computeMCAASnappedPosition(trPosition,
                                            uHints,
                                            transformST,
                                            uTransformST,
                                            uFramebufferSize,
                                            topSlope);
    tcPosition = computeMCAAPosition(tcPosition,
                                     uHints,
                                     transformST,
                                     uTransformST,
                                     uFramebufferSize);
    blPosition = computeMCAASnappedPosition(blPosition,
                                            uHints,
                                            transformST,
                                            uTransformST,
                                            uFramebufferSize,
                                            bottomSlope);
    brPosition = computeMCAASnappedPosition(brPosition,
                                            uHints,
                                            transformST,
                                            uTransformST,
                                            uFramebufferSize,
                                            bottomSlope);
    bcPosition = computeMCAAPosition(bcPosition,
                                     uHints,
                                     transformST,
                                     uTransformST,
                                     uFramebufferSize);

    float depth = convertPathIndexToViewportDepthValue(pathID);

    // Use the same side--in this case, the top--or else floating point error during partitioning
    // can occasionally cause inconsistent rounding, resulting in cracks.
    vec2 position;
    position.x = quadPosition.x < 0.5 ? tlPosition.x : trPosition.x;

    if (quadPosition.y < 0.5)
        position.y = floor(min(tlPosition.y, trPosition.y));
    else
        position.y = ceil(max(blPosition.y, brPosition.y));
    position = convertScreenToClipSpace(position, uFramebufferSize);

    gl_Position = vec4(position, depth, 1.0);
    vUpperEndpoints = vec4(tlPosition, trPosition);
    vLowerEndpoints = vec4(blPosition, brPosition);
    vControlPoints = vec4(tcPosition, bcPosition);
    vColor = color;
}
