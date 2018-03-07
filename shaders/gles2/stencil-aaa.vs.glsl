// pathfinder/shaders/gles2/stencil-aaa.vs.glsl
//
// Copyright (c) 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

uniform vec4 uTransformST;
uniform vec2 uTransformExt;
uniform ivec2 uFramebufferSize;
/// Vertical snapping positions.
uniform vec4 uHints;
uniform vec2 uEmboldenAmount;
uniform ivec2 uPathBoundsDimensions;
uniform sampler2D uPathBounds;
uniform ivec2 uPathTransformSTDimensions;
uniform sampler2D uPathTransformST;
uniform ivec2 uPathTransformExtDimensions;
uniform sampler2D uPathTransformExt;

attribute vec2 aTessCoord;
attribute vec2 aFromPosition;
attribute vec2 aCtrlPosition;
attribute vec2 aToPosition;
attribute vec2 aFromNormal;
attribute vec2 aCtrlNormal;
attribute vec2 aToNormal;
attribute float aPathID;

varying vec3 vUV;
varying vec3 vXDist;

void main() {
    // Unpack.
    vec2 emboldenAmount = uEmboldenAmount * 0.5;
    int pathID = int(aPathID);

    // Hint positions.
    vec2 fromPosition = hintPosition(aFromPosition, uHints);
    vec2 ctrlPosition = hintPosition(aCtrlPosition, uHints);
    vec2 toPosition = hintPosition(aToPosition, uHints);

    // Embolden as necessary.
    fromPosition -= aFromNormal * emboldenAmount;
    ctrlPosition -= aCtrlNormal * emboldenAmount;
    toPosition -= aToNormal * emboldenAmount;

    // Fetch transform.
    vec2 transformExt;
    vec4 transformST = fetchPathAffineTransform(transformExt,
                                                uPathTransformST,
                                                uPathTransformSTDimensions,
                                                uPathTransformExt,
                                                uPathTransformExtDimensions,
                                                pathID);

    // Concatenate transforms.
    mat2 globalTransformLinear = mat2(uTransformST.x, uTransformExt, uTransformST.y);
    mat2 localTransformLinear = mat2(transformST.x, -transformExt, transformST.y);
    mat2 transformLinear = globalTransformLinear * localTransformLinear;

    // Perform the linear component of the transform (everything but translation).
    fromPosition = quantize(transformLinear * fromPosition);
    ctrlPosition = quantize(transformLinear * ctrlPosition);
    toPosition = quantize(transformLinear * toPosition);

    // Choose correct quadrant for rotation.
    vec4 bounds = fetchFloat4Data(uPathBounds, pathID, uPathBoundsDimensions);
    vec2 fillVector = transformLinear * vec2(0.0, 1.0);
    vec2 corner = transformLinear * vec2(fillVector.x < 0.0 ? bounds.z : bounds.x,
                                         fillVector.y < 0.0 ? bounds.y : bounds.w);

    // Compute edge vectors.
    vec2 v02 = toPosition - fromPosition;
    vec2 v01 = ctrlPosition - fromPosition, v21 = ctrlPosition - toPosition;

    // Compute area of convex hull (w). Change from curve to line if appropriate.
    float w = det2(mat2(v01, v02));
    float sqLen01 = dot(v01, v01), sqLen02 = dot(v02, v02), sqLen21 = dot(v21, v21);
    float hullHeight = abs(w * inversesqrt(sqLen02));
    float minCtrlSqLen = sqLen02 * 0.01;
    if (sqLen01 < minCtrlSqLen || sqLen21 < minCtrlSqLen || hullHeight < 0.0001) {
        w = 0.0;
        v01 = vec2(0.5, abs(v02.y) >= 0.01 ? 0.0 : 0.5) * v02.xx;
    }

    // Compute position and dilate. If too thin, discard to avoid artefacts.
    vec2 dilation = vec2(0.0), position;
    if (aTessCoord.x < 0.5) {
        position.x = min(min(fromPosition.x, toPosition.x), ctrlPosition.x);
        dilation.x = -1.0;
    } else {
        position.x = max(max(fromPosition.x, toPosition.x), ctrlPosition.x);
        dilation.x = 1.0;
    }
    if (aTessCoord.y < 0.5) {
        position.y = min(min(fromPosition.y, toPosition.y), ctrlPosition.y);
        dilation.y = -1.0;
    } else {
        position.y = corner.y;
    }
    position += 2.0 * dilation / vec2(uFramebufferSize);

    // Compute UV using Cramer's rule.
    // https://gamedev.stackexchange.com/a/63203
    vec2 v03 = position - fromPosition;
    vec3 uv = vec3(0.0, det2(mat2(v01, v03)), sign(w));
    uv.x = uv.y + 0.5 * det2(mat2(v03, v02));
    uv.xy /= det2(mat2(v01, v02));

    // Compute X distances.
    vec3 xDist = position.x - vec3(fromPosition.x, ctrlPosition.x, toPosition.x);

    // Compute final position and depth.
    position += uTransformST.zw + globalTransformLinear * transformST.zw;
    float depth = convertPathIndexToViewportDepthValue(pathID);

    // Finish up.
    gl_Position = vec4(position, depth, 1.0);
    vUV = uv;
    vXDist = xDist;
}
