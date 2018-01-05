// pathfinder/shaders/gles2/direct-interior.vs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Renders polygonal portions of a mesh.
//!
//! Typically, you will run this shader before running `direct-curve`.
//! Remember to enable the depth test with a `GREATER` depth function for optimal
//! performance.

precision highp float;

/// A 3D transform to be applied to all points.
uniform mat4 uTransform;
/// Vertical snapping positions.
uniform vec4 uHints;
/// The size of the path colors texture in texels.
uniform ivec2 uPathColorsDimensions;
/// The fill color for each path.
uniform sampler2D uPathColors;
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

attribute vec2 aPosition;
attribute float aPathID;
attribute float aNormalAngle;

/// The color of this path.
varying vec4 vColor;

void main() {
    int pathID = int(aPathID);

    vec2 pathTransformExt;
    vec4 pathTransformST = fetchPathAffineTransform(pathTransformExt,
                                                    uPathTransformST,
                                                    uPathTransformSTDimensions,
                                                    uPathTransformExt,
                                                    uPathTransformExtDimensions,
                                                    pathID);

    vec2 position = dilatePosition(aPosition, aNormalAngle, uEmboldenAmount);
    position = hintPosition(position, uHints);
    position = transformVertexPositionAffine(position, pathTransformST, pathTransformExt);
    position = transformVertexPosition(position, uTransform);

    float depth = convertPathIndexToViewportDepthValue(pathID);
    gl_Position = vec4(position, depth, 1.0);

    vColor = fetchFloat4Data(uPathColors, pathID, uPathColorsDimensions);
}
