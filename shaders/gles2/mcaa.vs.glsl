// pathfinder/shaders/gles2/mcaa.vs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Renders paths when performing *mesh coverage antialiasing* (MCAA). This
//! one shader handles both lines and curves.
//!
//! This shader expects to render to a standard RGB color buffer in the
//! multicolor case or a single-channel floating-point color buffer in the
//! monochrome case.
//!
//! Set state as follows depending on whether multiple overlapping multicolor
//! paths are present:
//!
//! * When paths of multiple colors are present, use
//!   `glBlendFuncSeparate(GL_ONE, GL_ONE_MINUS_SRC_ALPHA, GL_ONE, GL_ONE)` and
//!   set `uMulticolor` to 1.
//!
//! * Otherwise, if only one color of path is present, use
//!   `glBlendFunc(GL_ONE, GL_ONE)` and set `uMulticolor` to 0.
//!
//! Use this shader only when your transform is only a scale and/or
//! translation, not a perspective, rotation, or skew. (Otherwise, consider
//! repartitioning the path to generate a new mesh, or, alternatively, use the
//! direct Loop-Blinn shaders.)

#define MAX_SLOPE   10.0

precision highp float;

/// A dilation (scale and translation) to be applied to the object.
uniform vec4 uTransformST;
/// Vertical snapping positions.
uniform vec4 uHints;
/// The framebuffer size in pixels.
uniform ivec2 uFramebufferSize;
/// The size of the path transform buffer texture in texels.
uniform ivec2 uPathTransformSTDimensions;
/// The path transform buffer texture, one dilation per path ID.
uniform sampler2D uPathTransformST;
/// The size of the path colors buffer texture in texels.
uniform ivec2 uPathColorsDimensions;
/// The path colors buffer texture, one color per path ID.
uniform sampler2D uPathColors;
/// True if multiple colors are being rendered; false otherwise.
///
/// If this is true, then points will be snapped to the nearest pixel.
uniform bool uMulticolor;

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
    int pathID = int(floor(aPathID));

    vec4 transformST = fetchFloat4Data(uPathTransformST, pathID, uPathTransformSTDimensions);
    if (abs(transformST.x) > 0.001 && abs(transformST.y) > 0.001) {
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
                                                topSlope,
                                                uMulticolor);
        trPosition = computeMCAASnappedPosition(trPosition,
                                                uHints,
                                                transformST,
                                                uTransformST,
                                                uFramebufferSize,
                                                topSlope,
                                                uMulticolor);
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
                                                bottomSlope,
                                                uMulticolor);
        brPosition = computeMCAASnappedPosition(brPosition,
                                                uHints,
                                                transformST,
                                                uTransformST,
                                                uFramebufferSize,
                                                bottomSlope,
                                                uMulticolor);
        bcPosition = computeMCAAPosition(bcPosition,
                                        uHints,
                                        transformST,
                                        uTransformST,
                                        uFramebufferSize);

        float depth = convertPathIndexToViewportDepthValue(pathID);

        // Use the same side--in this case, the top--or else floating point error during partitioning
        // can occasionally cause inconsistent rounding, resulting in cracks.
        vec2 position;

        if (uMulticolor)
            position.x = quadPosition.x < 0.5 ? tlPosition.x : trPosition.x;
        else
            position.x = quadPosition.x < 0.5 ? floor(tlPosition.x) : ceil(trPosition.x);

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
    } else {
        gl_Position = vec4(0.0);
    }
}
