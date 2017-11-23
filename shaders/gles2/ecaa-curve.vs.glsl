// pathfinder/shaders/gles2/ecaa-curve.vs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform mat4 uTransform;
uniform vec4 uHints;
uniform ivec2 uFramebufferSize;
uniform ivec2 uPathTransformDimensions;
uniform ivec2 uPathBoundsDimensions;
uniform sampler2D uPathTransform;
uniform sampler2D uPathBounds;
uniform vec2 uEmboldenAmount;

attribute vec2 aQuadPosition;
attribute vec2 aLeftPosition;
attribute vec2 aControlPointPosition;
attribute vec2 aRightPosition;
attribute float aPathID;
attribute vec3 aNormalAngles;

varying vec4 vEndpoints;
varying vec2 vControlPoint;
varying float vWinding;

void main() {
    vec2 leftPosition = aLeftPosition;
    vec2 controlPointPosition = aControlPointPosition;
    vec2 rightPosition = aRightPosition;
    int pathID = int(aPathID);
    float leftNormalAngle = aNormalAngles.x;
    float controlPointNormalAngle = aNormalAngles.y;
    float rightNormalAngle = aNormalAngles.z;

    vec4 transform = fetchFloat4Data(uPathTransform, pathID, uPathTransformDimensions);
    vec4 bounds = fetchFloat4Data(uPathBounds, pathID, uPathBoundsDimensions);

    // Transform the points, and compute the position of this vertex.
    vec2 position;
    float winding;
    if (computeECAAQuadPosition(position,
                                winding,
                                leftPosition,
                                rightPosition,
                                aQuadPosition,
                                uFramebufferSize,
                                transform,
                                uTransform,
                                uHints,
                                bounds,
                                leftNormalAngle,
                                rightNormalAngle,
                                uEmboldenAmount)) {
        controlPointPosition = dilatePosition(controlPointPosition,
                                              controlPointNormalAngle,
                                              uEmboldenAmount);
        controlPointPosition = hintPosition(controlPointPosition, uHints);
        controlPointPosition = transformVertexPositionST(controlPointPosition, transform);
        controlPointPosition = transformVertexPosition(controlPointPosition, uTransform);
        controlPointPosition = convertClipToScreenSpace(controlPointPosition, uFramebufferSize);
    }

    float depth = convertPathIndexToViewportDepthValue(pathID);

    gl_Position = vec4(position, depth, 1.0);
    vEndpoints = vec4(leftPosition, rightPosition);
    vControlPoint = controlPointPosition;
    vWinding = winding;
}
