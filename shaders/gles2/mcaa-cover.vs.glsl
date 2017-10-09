// pathfinder/shaders/gles2/mcaa-cover.vs.glsl
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

attribute vec2 aQuadPosition;
attribute vec2 aUpperLeftPosition;
attribute vec2 aLowerRightPosition;
attribute float aPathID;

varying vec2 vHorizontalExtents;

void main() {
    int pathID = int(aPathID);

    vec4 transform = fetchFloat4Data(uPathTransform, pathID, uPathTransformDimensions);

    vec2 upperLeftPosition = hintPosition(aUpperLeftPosition, uHints);
    vec2 lowerRightPosition = hintPosition(aLowerRightPosition, uHints);

    upperLeftPosition = transformVertexPositionST(upperLeftPosition, transform);
    lowerRightPosition = transformVertexPositionST(lowerRightPosition, transform);

    upperLeftPosition = transformVertexPositionST(upperLeftPosition, uTransformST);
    lowerRightPosition = transformVertexPositionST(lowerRightPosition, uTransformST);

    upperLeftPosition = convertClipToScreenSpace(upperLeftPosition, uFramebufferSize);
    lowerRightPosition = convertClipToScreenSpace(lowerRightPosition, uFramebufferSize);

    vec4 roundedExtents = vec4(floor(upperLeftPosition.x), ceil(upperLeftPosition.y),
                               ceil(lowerRightPosition));

    vec2 position = mix(roundedExtents.xy, roundedExtents.zw, aQuadPosition);
    position = convertScreenToClipSpace(position, uFramebufferSize);
    float depth = convertPathIndexToViewportDepthValue(pathID);
    gl_Position = vec4(position, depth, 1.0);

    vHorizontalExtents = vec2(upperLeftPosition.x, lowerRightPosition.x);
}
