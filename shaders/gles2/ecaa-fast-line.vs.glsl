// pathfinder/shaders/gles2/ecaa-fast-line.vs.glsl
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
uniform ivec2 uPathTransformDimensions;
uniform sampler2D uPathTransform;
uniform bool uLowerPart;

attribute vec2 aQuadPosition;
attribute vec2 aLeftPosition;
attribute vec2 aRightPosition;
attribute float aPathID;

varying vec4 vEndpoints;

void main() {
    vec2 leftPosition = aLeftPosition;
    vec2 rightPosition = aRightPosition;
    int pathID = int(aPathID);

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
