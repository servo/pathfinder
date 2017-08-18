// pathfinder/shaders/gles2/ecaa-line.vs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

uniform mat4 uTransform;
uniform ivec2 uFramebufferSize;
uniform ivec2 uBVertexPositionDimensions;
uniform ivec2 uBVertexPathIDDimensions;
uniform sampler2D uBVertexPosition;
uniform sampler2D uBVertexPathID;
uniform bool uLowerPart;

attribute vec2 aQuadPosition;
attribute vec4 aLineIndices;

varying vec4 vEndpoints;

void main() {
    // Fetch B-vertex positions.
    ivec2 pointIndices = ivec2(unpackUInt32Attribute(aLineIndices.xy),
                               unpackUInt32Attribute(aLineIndices.zw));
    vec2 leftPosition = fetchFloat2Data(uBVertexPosition,
                                        pointIndices.x,
                                        uBVertexPositionDimensions);
    vec2 rightPosition = fetchFloat2Data(uBVertexPosition,
                                         pointIndices.y,
                                         uBVertexPositionDimensions);

    vec2 position;
    if (abs(leftPosition.x - rightPosition.x) > EPSILON) {
        leftPosition = transformVertexPosition(leftPosition, uTransform);
        rightPosition = transformVertexPosition(rightPosition, uTransform);

        vec2 verticalExtents = vec2(min(leftPosition.y, rightPosition.y),
                                    max(leftPosition.y, rightPosition.y));

        vec4 roundedExtents = vec4(floor(vec2(leftPosition.x, verticalExtents.x)),
                                   ceil(vec2(rightPosition.x, verticalExtents.y)));

        // FIXME(pcwalton): Use a separate VBO for this.
        vec2 quadPosition = (aQuadPosition + 1.0) * 0.5;

        position = mix(roundedExtents.xy, roundedExtents.zw,  quadPosition);
        position = convertScreenToClipSpace(position, uFramebufferSize);
    } else {
        position = vec2(0.0);
    }

    int pathID = fetchUInt16Data(uBVertexPathID, pointIndices.x, uBVertexPathIDDimensions);
    float depth = convertPathIndexToDepthValue(pathID);

    gl_Position = vec4(position, depth, 1.0);
    vEndpoints = vec4(leftPosition, rightPosition);
}
