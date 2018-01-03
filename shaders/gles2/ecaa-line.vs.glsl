// pathfinder/shaders/gles2/ecaa-line.vs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implements *edge coverage antialiasing* (ECAA) for straight-line path
//! segments.
//!
//! This shader expects to render to the red channel of a floating point color
//! buffer. Half precision floating point should be sufficient.
//!
//! Use this shader only when *both* of the following are true:
//!
//! 1. You are only rendering monochrome paths such as text. (Otherwise,
//!    consider MCAA.)
//!
//! 2. The paths are relatively small, so overdraw is not a concern.
//!    (Otherwise, consider MCAA.)

precision highp float;

uniform mat4 uTransform;
uniform vec4 uHints;
uniform ivec2 uFramebufferSize;
uniform ivec2 uPathBoundsDimensions;
uniform sampler2D uPathBounds;
uniform ivec2 uPathTransformSTDimensions;
uniform sampler2D uPathTransformST;
uniform ivec2 uPathTransformExtDimensions;
uniform sampler2D uPathTransformExt;
uniform vec2 uEmboldenAmount;

attribute vec2 aQuadPosition;
attribute vec2 aLeftPosition;
attribute vec2 aRightPosition;
attribute float aPathID;
attribute float aLeftNormalAngle;
attribute float aRightNormalAngle;

varying vec4 vEndpoints;
varying float vWinding;

void main() {
    vec2 leftPosition = aLeftPosition;
    vec2 rightPosition = aRightPosition;
    int pathID = int(aPathID);
    vec2 leftRightNormalAngles = vec2(aLeftNormalAngle, aRightNormalAngle);

    vec2 pathTransformExt;
    vec4 pathTransformST = fetchPathAffineTransform(pathTransformExt,
                                                    uPathTransformST,
                                                    uPathTransformSTDimensions,
                                                    uPathTransformExt,
                                                    uPathTransformExtDimensions,
                                                    pathID);
    vec4 bounds = fetchFloat4Data(uPathBounds, pathID, uPathBoundsDimensions);

    // Transform the points, and compute the position of this vertex.
    vec2 position;
    float winding;
    computeECAAQuadPosition(position,
                            winding,
                            leftPosition,
                            rightPosition,
                            aQuadPosition,
                            uFramebufferSize,
                            pathTransformST,
                            pathTransformExt,
                            uTransform,
                            uHints,
                            bounds,
                            leftRightNormalAngles,
                            uEmboldenAmount);

    float depth = convertPathIndexToViewportDepthValue(pathID);

    gl_Position = vec4(position, depth, 1.0);
    vEndpoints = vec4(leftPosition, rightPosition);
    vWinding = winding;
}
