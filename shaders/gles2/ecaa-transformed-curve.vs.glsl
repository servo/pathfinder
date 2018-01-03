// pathfinder/shaders/gles2/ecaa-transformed-curve.vs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implements *edge coverage antialiasing* (ECAA) for curved path segments,
//! performing splitting as necessary.
//!
//! This shader expects to render to the red channel of a floating point color
//! buffer. Half precision floating point should be sufficient.
//!
//! This is a two-pass shader. It must be run twice, first with `uPassIndex`
//! equal to 0, and then with `uPassIndex` equal to 1.
//!
//! Use this shader only when *all* of the following are true:
//!
//! 1. You are only rendering monochrome paths such as text. (Otherwise,
//!    consider MCAA.)
//!
//! 2. The paths are relatively small, so overdraw is not a concern.
//!    (Otherwise, consider MCAA.)
//!
//! 3. Your transform contains perspective, rotation, or skew. (Otherwise,
//!    consider `ecaa-curve`, which is faster and saves a pass.)

precision highp float;

uniform mat4 uTransform;
uniform vec4 uHints;
uniform ivec2 uFramebufferSize;
uniform ivec2 uPathTransformSTDimensions;
uniform sampler2D uPathTransformST;
uniform ivec2 uPathTransformExtDimensions;
uniform sampler2D uPathTransformExt;
uniform ivec2 uPathBoundsDimensions;
uniform sampler2D uPathBounds;
uniform vec2 uEmboldenAmount;
uniform int uPassIndex;

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

    vec2 pathTransformExt;
    vec4 pathTransformST = fetchPathAffineTransform(pathTransformExt,
                                                    uPathTransformST,
                                                    uPathTransformSTDimensions,
                                                    uPathTransformExt,
                                                    uPathTransformExtDimensions,
                                                    pathID);
    vec4 bounds = fetchFloat4Data(uPathBounds, pathID, uPathBoundsDimensions);

    // Transform the points.
    leftPosition = computeECAAPosition(leftPosition,
                                       aNormalAngles.x,
                                       uEmboldenAmount,
                                       uHints,
                                       pathTransformST,
                                       pathTransformExt,
                                       uTransform,
                                       uFramebufferSize);
    rightPosition = computeECAAPosition(rightPosition,
                                        aNormalAngles.z,
                                        uEmboldenAmount,
                                        uHints,
                                        pathTransformST,
                                        pathTransformExt,
                                        uTransform,
                                        uFramebufferSize);
    controlPointPosition = computeECAAPosition(controlPointPosition,
                                               aNormalAngles.y,
                                               uEmboldenAmount,
                                               uHints,
                                               pathTransformST,
                                               pathTransformExt,
                                               uTransform,
                                               uFramebufferSize);

    float winding;
    vec3 leftTopRightEdges;
    if (!splitCurveAndComputeECAAWinding(winding,
                                         leftTopRightEdges,
                                         leftPosition,
                                         rightPosition,
                                         controlPointPosition,
                                         uPassIndex)) {
        gl_Position = vec4(0.0);
        return;
    }

    vec2 position = computeECAAQuadPositionFromTransformedPositions(leftPosition,
                                                                    rightPosition,
                                                                    aQuadPosition,
                                                                    uFramebufferSize,
                                                                    pathTransformST,
                                                                    pathTransformExt,
                                                                    uTransform,
                                                                    bounds,
                                                                    leftTopRightEdges);

    float depth = convertPathIndexToViewportDepthValue(pathID);

    gl_Position = vec4(position, depth, 1.0);
    vEndpoints = vec4(leftPosition, rightPosition);
    vControlPoint = controlPointPosition;
    vWinding = winding;
}
