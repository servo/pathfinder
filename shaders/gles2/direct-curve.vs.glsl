// pathfinder/shaders/gles2/direct-curve.vs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implements the quadratic Loop-Blinn formulation to render curved parts of
//! the mesh.
//!
//! This shader performs no antialiasing; if you want antialiased output from
//! this shader, use MSAA with sample-level shading (GL 4.x) or else perform
//! SSAA by rendering to a higher-resolution framebuffer and downsampling (GL
//! 3.x and below).
//!
//! If you know your mesh has no curves (i.e. it consists solely of polygons),
//! then you don't need to run this shader.

precision highp float;

uniform mat4 uTransform;
uniform vec4 uHints;
uniform vec2 uEmboldenAmount;
uniform ivec2 uPathColorsDimensions;
uniform sampler2D uPathColors;
uniform ivec2 uPathTransformSTDimensions;
uniform sampler2D uPathTransformST;
uniform ivec2 uPathTransformExtDimensions;
uniform sampler2D uPathTransformExt;

attribute vec2 aPosition;
attribute vec2 aTexCoord;
attribute float aPathID;
attribute float aSign;
attribute float aNormalAngle;

varying vec4 vColor;
varying vec2 vTexCoord;
varying float vSign;

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
    vTexCoord = vec2(aTexCoord) / 2.0;
    vSign = aSign;
}
