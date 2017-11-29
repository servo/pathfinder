// pathfinder/shaders/gles2/direct-3d-interior.vs.glsl
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
uniform vec2 uEmboldenAmount;
uniform ivec2 uPathColorsDimensions;
uniform sampler2D uPathColors;
uniform ivec2 uPathTransformSTDimensions;
uniform sampler2D uPathTransformST;
uniform ivec2 uPathTransformExtDimensions;
uniform sampler2D uPathTransformExt;

attribute vec2 aPosition;
attribute float aPathID;
attribute float aNormalAngle;

varying vec4 vColor;
varying vec2 vPathID;

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
    position = transformVertexPositionAffine(position, pathTransformST, pathTransformExt);

    gl_Position = uTransform * vec4(position, 0.0, 1.0);

    vColor = fetchFloat4Data(uPathColors, pathID, uPathColorsDimensions);
    vPathID = packPathID(pathID);
}
