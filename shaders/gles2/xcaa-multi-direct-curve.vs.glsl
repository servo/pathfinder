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
uniform ivec2 uPathTransformDimensions;
uniform sampler2D uPathTransform;

attribute vec2 aPosition;
attribute vec2 aTexCoord;
attribute float aPathID;
attribute float aSign;

varying vec2 vPathID;
varying vec2 vTexCoord;
varying float vSign;

void main() {
    int pathID = int(aPathID);

    vec4 pathTransform = fetchFloat4Data(uPathTransform, pathID, uPathTransformDimensions);

    vec2 position = hintPosition(aPosition, uHints);
    position = transformVertexPositionST(position, pathTransform);
    position = transformVertexPosition(position, uTransform);

    float depth = convertPathIndexToViewportDepthValue(pathID);
    gl_Position = vec4(position, depth, 1.0);

    vPathID = packPathID(pathID);
    vTexCoord = vec2(aTexCoord) / 2.0;
    vSign = aSign;
}
