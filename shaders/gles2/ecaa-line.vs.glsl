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

/// A 3D transform to be applied to the object.
uniform mat4 uTransform;
/// Vertical snapping positions.
uniform vec4 uHints;
/// The framebuffer size in pixels.
uniform ivec2 uFramebufferSize;
/// The size of the path bounds texture in texels.
uniform ivec2 uPathBoundsDimensions;
/// The path bounds texture, one rect per path ID.
uniform sampler2D uPathBounds;
/// The size of the path transform buffer texture in texels.
uniform ivec2 uPathTransformSTDimensions;
/// The path transform buffer texture, one path dilation per texel.
uniform sampler2D uPathTransformST;
/// The size of the extra path transform factors buffer texture in texels.
uniform ivec2 uPathTransformExtDimensions;
/// The extra path transform factors buffer texture, packed two path transforms per texel.
uniform sampler2D uPathTransformExt;
/// The amount of faux-bold to apply, in local path units.
uniform vec2 uEmboldenAmount;

/// The abstract quad position: (0.0, 0.0) to (1.0, 1.0).
attribute vec2 aTessCoord;
/// The position of the left endpoint.
attribute vec2 aLeftPosition;
/// The position of the right endpoint.
attribute vec2 aRightPosition;
/// The path ID (starting from 1).
attribute float aPathID;
/// The normal angles of the left endpoint and right endpoint, respectively.
attribute vec2 aNormalAngles;

varying vec4 vEndpoints;
varying float vWinding;

void main() {
    vec2 leftPosition = aLeftPosition;
    vec2 rightPosition = aRightPosition;
    int pathID = int(aPathID);
    vec2 leftRightNormalAngles = aNormalAngles;

    vec2 pathTransformExt;
    vec4 pathTransformST = fetchPathAffineTransform(pathTransformExt,
                                                    uPathTransformST,
                                                    uPathTransformSTDimensions,
                                                    uPathTransformExt,
                                                    uPathTransformExtDimensions,
                                                    pathID);
    vec4 bounds = fetchFloat4Data(uPathBounds, pathID, uPathBoundsDimensions);

    // Transform the points, and compute the position of this vertex.
    leftPosition = computeECAAPosition(leftPosition,
                                       aNormalAngles.x,
                                       uEmboldenAmount,
                                       uHints,
                                       pathTransformST,
                                       pathTransformExt,
                                       uTransform,
                                       uFramebufferSize);
    rightPosition = computeECAAPosition(rightPosition,
                                        aNormalAngles.y,
                                        uEmboldenAmount,
                                        uHints,
                                        pathTransformST,
                                        pathTransformExt,
                                        uTransform,
                                        uFramebufferSize);
    float winding = computeECAAWinding(leftPosition, rightPosition);
    if (winding == 0.0) {
        gl_Position = vec4(0.0);
        return;
    }

    vec2 edgeBL = bounds.xy, edgeTL = bounds.xw, edgeTR = bounds.zw, edgeBR = bounds.zy;
    edgeBL = transformECAAPosition(edgeBL, pathTransformST, pathTransformExt, uTransform);
    edgeBR = transformECAAPosition(edgeBR, pathTransformST, pathTransformExt, uTransform);
    edgeTL = transformECAAPosition(edgeTL, pathTransformST, pathTransformExt, uTransform);
    edgeTR = transformECAAPosition(edgeTR, pathTransformST, pathTransformExt, uTransform);

    // Find the bottom of the path, and convert to clip space.
    //
    // FIXME(pcwalton): Speed this up somehow?
    float pathBottomY = max(max(edgeBL.y, edgeBR.y), max(edgeTL.y, edgeTR.y));
    pathBottomY = (pathBottomY + 1.0) * 0.5 * float(uFramebufferSize.y);

    vec2 position;
    if (aTessCoord.x < 0.5)
        position = vec2(floor(leftPosition.x), leftPosition.y);
    else
        position = vec2(ceil(rightPosition.x), rightPosition.y);

    // FIXME(pcwalton): Only compute path bottom Y if necessary.
    if (aTessCoord.y < 0.5)
        position.y = floor(position.y - 1.0);
    else
        position.y = pathBottomY;

    position = convertScreenToClipSpace(position, uFramebufferSize);
    float depth = convertPathIndexToViewportDepthValue(pathID);

    gl_Position = vec4(position, depth, 1.0);
    vEndpoints = vec4(leftPosition, rightPosition);
    vWinding = winding;
}
