// pathfinder/shaders/gles2/ecaa-multi-edge-mask-curve.vs.glsl
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
uniform ivec2 uFramebufferSize;
uniform ivec2 uPathTransformSTDimensions;
uniform sampler2D uPathTransformST;
uniform ivec2 uPathTransformExtDimensions;
uniform sampler2D uPathTransformExt;
uniform int uPassIndex;

attribute vec2 aQuadPosition;
attribute vec2 aLeftPosition;
attribute vec2 aControlPointPosition;
attribute vec2 aRightPosition;
attribute float aPathID;

varying vec4 vEndpoints;
varying vec2 vControlPoint;

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

    // Transform the points.
    leftPosition = transformECAAPositionToScreenSpace(leftPosition,
                                                      pathTransformST,
                                                      pathTransformExt,
                                                      uTransform,
                                                      uFramebufferSize);
    rightPosition = transformECAAPositionToScreenSpace(rightPosition,
                                                       pathTransformST,
                                                       pathTransformExt,
                                                       uTransform,
                                                       uFramebufferSize);
    controlPointPosition = transformECAAPositionToScreenSpace(controlPointPosition,
                                                              pathTransformST,
                                                              pathTransformExt,
                                                              uTransform,
                                                              uFramebufferSize);

    float winding = computeECAAWinding(leftPosition, rightPosition);
    if (winding == 0.0) {
        gl_Position = vec4(0.0);
        return;
    }

    vec4 extents = vec4(leftPosition.x,
                        min(leftPosition.y, rightPosition.y),
                        rightPosition.y,
                        max(leftPosition.y, rightPosition.y));
    vec2 position = computeXCAAClipSpaceQuadPosition(extents, aQuadPosition, uFramebufferSize);

    float depth = convertPathIndexToViewportDepthValue(pathID);

    gl_Position = vec4(position, depth, 1.0);
    vEndpoints = vec4(leftPosition, rightPosition);
    vControlPoint = controlPointPosition;
}
