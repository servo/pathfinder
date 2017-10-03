// pathfinder/shaders/gles2/direct-curve.vs.glsl
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
uniform ivec2 uPathColorsDimensions;
uniform ivec2 uPathTransformDimensions;
uniform ivec2 uPathHintsDimensions;
uniform sampler2D uPathColors;
uniform sampler2D uPathTransform;
uniform sampler2D uPathHints;

attribute vec2 aPosition;
attribute vec2 aTexCoord;
attribute float aPathID;
attribute float aSign;

varying vec4 vColor;
varying vec2 vPathID;
varying vec2 vTexCoord;
varying float vSign;

void main() {
    int pathID = int(aPathID);

    vec4 pathHints = fetchFloat4Data(uPathHints, pathID, uPathHintsDimensions);
    vec4 pathTransform = fetchFloat4Data(uPathTransform, pathID, uPathTransformDimensions);

    vec2 position = hintPosition(aPosition, pathHints);
    position = transformVertexPositionST(position, pathTransform);
    position = transformVertexPosition(position, uTransform);

    float depth = convertPathIndexToViewportDepthValue(pathID);
    gl_Position = vec4(position, depth, 1.0);

    vColor = fetchFloat4Data(uPathColors, pathID, uPathColorsDimensions);
    vPathID = packPathID(pathID);
    vTexCoord = vec2(aTexCoord) / 2.0;
    vSign = aSign;
}
