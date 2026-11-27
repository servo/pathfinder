#version 430

// pathfinder/shaders/dice.cs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Chops lines and curves into microlines.

#extension GL_GOOGLE_include_directive : enable

#define BIN_WORKGROUP_SIZE  64

#define MAX_CURVE_STACK_SIZE    32

#define FLAGS_PATH_INDEX_CURVE_IS_QUADRATIC   0x80000000u
#define FLAGS_PATH_INDEX_CURVE_IS_CUBIC       0x40000000u

#define BIN_INDIRECT_DRAW_PARAMS_MICROLINE_COUNT_INDEX  3

#define TOLERANCE           0.25
#define MICROLINE_LENGTH    16.0

precision highp float;

#ifdef GL_ES
precision highp sampler2D;
#endif

layout(local_size_x = 64) in;

uniform mat2 uTransform;
uniform vec2 uTranslation;
uniform int uPathCount;
uniform int uLastBatchSegmentIndex;
uniform int uMaxMicrolineCount;

restrict layout(std430, binding = 0) buffer bComputeIndirectParams {
    // [0]: number of x workgroups
    // [1]: number of y workgroups (always 1)
    // [2]: number of z workgroups (always 1)
    // [3]: number of output microlines
    uint iComputeIndirectParams[];
};

// Indexed by batch path index.
restrict readonly layout(std430, binding = 1) buffer bDiceMetadata {
    // x: global path ID
    // y: first global segment index
    // z: first batch segment index
    // w: unused
    uvec4 iDiceMetadata[];
};

restrict readonly layout(std430, binding = 2) buffer bPoints {
    vec2 iPoints[];
};

restrict readonly layout(std430, binding = 3) buffer bInputIndices {
    uvec2 iInputIndices[];
};

restrict layout(std430, binding = 4) buffer bMicrolines {
    // x: from (X, Y) whole pixels, packed signed 16-bit
    // y: to (X, Y) whole pixels, packed signed 16-bit
    // z: (from X, from Y, to X, to Y) fractional pixels, packed unsigned 8-bit (0.8 fixed point)
    // w: path ID
    uvec4 iMicrolines[];
};

void emitMicroline(vec4 microlineSegment, uint pathIndex, uint outputMicrolineIndex) {
    if (outputMicrolineIndex >= uMaxMicrolineCount)
        return;

    ivec4 microlineSubpixels = ivec4(round(clamp(microlineSegment, -32768.0, 32767.0) * 256.0));
    ivec4 microlinePixels = ivec4(floor(vec4(microlineSubpixels) / 256.0));
    ivec4 microlineFractPixels = microlineSubpixels - microlinePixels * 256;

    iMicrolines[outputMicrolineIndex] =
        uvec4((uint(microlinePixels.x) & 0xffff) | (uint(microlinePixels.y) << 16),
            (uint(microlinePixels.z) & 0xffff) | (uint(microlinePixels.w) << 16),
            uint(microlineFractPixels.x)        | (uint(microlineFractPixels.y) << 8) |
            (uint(microlineFractPixels.z) << 16) | (uint(microlineFractPixels.w) << 24),
            pathIndex);
}

// See Kaspar Fischer, "Piecewise Linear Approximation of Bézier Curves", 2000.
bool curveIsFlat(vec4 baseline, vec4 ctrl) {
    vec4 uv = vec4(3.0) * ctrl - vec4(2.0) * baseline - baseline.zwxy;
    uv *= uv;
    uv = max(uv, uv.zwxy);
    return uv.x + uv.y <= 16.0 * TOLERANCE * TOLERANCE;
}

void subdivideCurve(vec4 baseline,
                    vec4 ctrl,
                    float t,
                    out vec4 prevBaseline,
                    out vec4 prevCtrl,
                    out vec4 nextBaseline,
                    out vec4 nextCtrl) {
    vec2 p0 = baseline.xy, p1 = ctrl.xy, p2 = ctrl.zw, p3 = baseline.zw;
    vec2 p0p1 = mix(p0, p1, t), p1p2 = mix(p1, p2, t), p2p3 = mix(p2, p3, t);
    vec2 p0p1p2 = mix(p0p1, p1p2, t), p1p2p3 = mix(p1p2, p2p3, t);
    vec2 p0p1p2p3 = mix(p0p1p2, p1p2p3, t);
    prevBaseline = vec4(p0, p0p1p2p3);
    prevCtrl = vec4(p0p1, p0p1p2);
    nextBaseline = vec4(p0p1p2p3, p3);
    nextCtrl = vec4(p1p2p3, p2p3);
}

vec2 sampleCurve(vec4 baseline, vec4 ctrl, float t) {
    vec2 p0 = baseline.xy, p1 = ctrl.xy, p2 = ctrl.zw, p3 = baseline.zw;
    vec2 p0p1 = mix(p0, p1, t), p1p2 = mix(p1, p2, t), p2p3 = mix(p2, p3, t);
    vec2 p0p1p2 = mix(p0p1, p1p2, t), p1p2p3 = mix(p1p2, p2p3, t);
    return mix(p0p1p2, p1p2p3, t);
}

vec2 sampleLine(vec4 line, float t) {
    return mix(line.xy, line.zw, t);
}

vec2 getPoint(uint pointIndex) {
    return uTransform * iPoints[pointIndex] + uTranslation;
}

void main() {
    uint batchSegmentIndex = gl_GlobalInvocationID.x;
    if (batchSegmentIndex >= uLastBatchSegmentIndex)
        return;

    // Find the path index.
    uint lowPathIndex = 0, highPathIndex = uint(uPathCount);
    int iteration = 0;
    while (iteration < 1024 && lowPathIndex + 1 < highPathIndex) {
        uint midPathIndex = lowPathIndex + (highPathIndex - lowPathIndex) / 2;
        uint midBatchSegmentIndex = iDiceMetadata[midPathIndex].z;
        if (batchSegmentIndex < midBatchSegmentIndex) {
            highPathIndex = midPathIndex;
        } else {
            lowPathIndex = midPathIndex;
            if (batchSegmentIndex == midBatchSegmentIndex)
                break;
        }
        iteration++;
    }

    uint batchPathIndex = lowPathIndex;
    uvec4 diceMetadata = iDiceMetadata[batchPathIndex];
    uint firstGlobalSegmentIndexInPath = diceMetadata.y;
    uint firstBatchSegmentIndexInPath = diceMetadata.z;
    uint globalSegmentIndex = batchSegmentIndex - firstBatchSegmentIndexInPath +
        firstGlobalSegmentIndexInPath;

    uvec2 inputIndices = iInputIndices[globalSegmentIndex];
    uint fromPointIndex = inputIndices.x, flagsPathIndex = inputIndices.y;

    uint toPointIndex = fromPointIndex;
    if ((flagsPathIndex & FLAGS_PATH_INDEX_CURVE_IS_CUBIC) != 0u)
        toPointIndex += 3;
    else if ((flagsPathIndex & FLAGS_PATH_INDEX_CURVE_IS_QUADRATIC) != 0u)
        toPointIndex += 2;
    else
        toPointIndex += 1;

    vec4 baseline = vec4(getPoint(fromPointIndex), getPoint(toPointIndex));

    // Read control points if applicable, and calculate number of segments.
    //
    // The technique is from Thomas Sederberg, "Computer-Aided Geometric Design" notes, section
    // 10.6 "Error Bounds".
    vec4 ctrl = vec4(0.0);
    float segmentCountF;
    bool isCurve = (flagsPathIndex & (FLAGS_PATH_INDEX_CURVE_IS_CUBIC |
                                      FLAGS_PATH_INDEX_CURVE_IS_QUADRATIC)) != 0;
    if (isCurve) {
        vec2 ctrl0 = getPoint(fromPointIndex + 1);
        if ((flagsPathIndex & FLAGS_PATH_INDEX_CURVE_IS_QUADRATIC) != 0) {
            vec2 ctrl0_2 = ctrl0 * vec2(2.0);
            ctrl = (baseline + (ctrl0 * vec2(2.0)).xyxy) * vec4(1.0 / 3.0);
        } else {
            ctrl = vec4(ctrl0, getPoint(fromPointIndex + 2));
        }
        vec2 bound = vec2(6.0) * max(abs(ctrl.zw - 2.0 * ctrl.xy + baseline.xy),
                                     abs(baseline.zw - 2.0 * ctrl.zw + ctrl.xy));
        segmentCountF = sqrt(length(bound) / (8.0 * TOLERANCE));
    } else {
        segmentCountF = length(baseline.zw - baseline.xy) / MICROLINE_LENGTH;
    }

    // Allocate space.
    int segmentCount = max(int(ceil(segmentCountF)), 1);
    uint firstOutputMicrolineIndex =
        atomicAdd(iComputeIndirectParams[BIN_INDIRECT_DRAW_PARAMS_MICROLINE_COUNT_INDEX],
                  segmentCount);

    float prevT = 0.0;
    vec2 prevPoint = baseline.xy;
    for (int segmentIndex = 0; segmentIndex < segmentCount; segmentIndex++) {
        float nextT = float(segmentIndex + 1) / float(segmentCount);
        vec2 nextPoint;
        if (isCurve)
            nextPoint = sampleCurve(baseline, ctrl, nextT);
        else
            nextPoint = sampleLine(baseline, nextT);
        emitMicroline(vec4(prevPoint, nextPoint),
                      batchPathIndex,
                      firstOutputMicrolineIndex + segmentIndex);
        prevT = nextT;
        prevPoint = nextPoint;
    }
}
