// pathfinder/shaders/gles2/xcaa-multi-direct-curve.vs.glsl
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
uniform vec2 uEmboldenAmount;
uniform ivec2 uPathColorsDimensions;
uniform sampler2D uPathColors;
uniform ivec2 uPathTransformDimensions;
uniform sampler2D uPathTransform;

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

    vec4 pathTransform = fetchFloat4Data(uPathTransform, pathID, uPathTransformDimensions);

    vec2 position = dilatePosition(aPosition, aNormalAngle, uEmboldenAmount);
    position = hintPosition(position, uHints);
    position = transformVertexPositionST(position, pathTransform);
    position = transformVertexPosition(position, uTransform);

    float depth = convertPathIndexToViewportDepthValue(pathID);
    gl_Position = vec4(position, depth, 1.0);

    vColor = fetchFloat4Data(uPathColors, pathID, uPathColorsDimensions);
    vTexCoord = vec2(aTexCoord) / 2.0;
    vSign = aSign;
}
