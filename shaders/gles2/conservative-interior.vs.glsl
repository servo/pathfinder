// pathfinder/shaders/gles2/conservative-interior.vs.glsl
//
// Copyright (c) 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Renders polygonal portions of a mesh, only filling pixels that are fully
//! covered.
//!
//! Typically, you will run this shader before running XCAA.
//! Remember to enable the depth test with a `GREATER` depth function for optimal
//! performance.

precision highp float;

/// An affine transform to be applied to all points.
uniform vec4 uTransformST;
uniform vec2 uTransformExt;
/// Vertical snapping positions.
uniform vec4 uHints;
/// The framebuffer size in pixels.
uniform ivec2 uFramebufferSize;
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

/// The 2D position of this point.
attribute vec2 aPosition;
/// The path ID, starting from 1.
attribute float aPathID;
/// The vertex ID. In OpenGL 3.0+, this can be omitted in favor of `gl_VertexID`.
attribute float aVertexID;

/// The color of this path.
varying vec4 vColor;

void main() {
    int pathID = int(aPathID);
    int vertexID = int(aVertexID);

    vec4 transformST = fetchFloat4Data(uPathTransformST, pathID, uPathTransformSTDimensions);

    mat2 globalTransformLinear = mat2(uTransformST.x, uTransformExt, uTransformST.y);
    mat2 localTransformLinear = mat2(transformST.x, 0.0, 0.0, transformST.y);
    mat2 transformLinear = globalTransformLinear * localTransformLinear;

    vec2 translation = uTransformST.zw + globalTransformLinear * transformST.zw;

    float onePixel = 2.0 / float(uFramebufferSize.y);
    float dilation = length(invMat2(transformLinear) * vec2(0.0, onePixel));

    vec2 position = aPosition + vec2(0.0, imod(vertexID, 6) < 3 ? dilation : -dilation);
    position = transformLinear * position + translation;
    float depth = convertPathIndexToViewportDepthValue(pathID);

    gl_Position = vec4(position, depth, 1.0);
    vColor = fetchFloat4Data(uPathColors, pathID, uPathColorsDimensions);
}
