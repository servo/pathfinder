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
uniform int uSide;

attribute vec2 aTessCoord;
attribute vec2 aFromPosition;
attribute vec2 aCtrlPosition;
attribute vec2 aToPosition;
attribute vec2 aFromNormal;
attribute vec2 aCtrlNormal;
attribute vec2 aToNormal;
attribute float aPathID;

varying vec2 vFrom;
varying vec2 vCtrl;
varying vec2 vTo;

void main() {
    // Unpack.
    vec2 emboldenAmount = uEmboldenAmount * 0.5;
    int pathID = int(aPathID);

    // Hint positions.
    vec2 from = hintPosition(aFromPosition, uHints);
    vec2 ctrl = hintPosition(aCtrlPosition, uHints);
    vec2 to = hintPosition(aToPosition, uHints);

    // Embolden as necessary.
    from -= aFromNormal * emboldenAmount;
    ctrl -= aCtrlNormal * emboldenAmount;
    to -= aToNormal * emboldenAmount;

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
    from = transformLinear * from;
    ctrl = transformLinear * ctrl;
    to = transformLinear * to;

    // Choose correct quadrant for rotation.
    vec4 bounds = fetchFloat4Data(uPathBounds, pathID, uPathBoundsDimensions);
    vec2 fillVector = transformLinear * vec2(0.0, 1.0);
    vec2 corner = transformLinear * vec2(fillVector.x < 0.0 ? bounds.z : bounds.x,
                                         fillVector.y < 0.0 ? bounds.y : bounds.w);

    // Compute edge vectors. De Casteljau subdivide if necessary.
    vec2 v01 = ctrl - from, v12 = to - ctrl;
    float t = clamp(v01.x / (v01.x - v12.x), 0.0, 1.0);
    vec2 ctrl0 = mix(from, ctrl, t), ctrl1 = mix(ctrl, to, t);
    vec2 mid = mix(ctrl0, ctrl1, t);
    if (uSide == 0) {
        from = mid;
        ctrl = ctrl1;
    } else {
        ctrl = ctrl0;
        to = mid;
    }

    // Compute position and dilate. If too thin, discard to avoid artefacts.
    vec2 dilation, position;
    bool zeroArea = abs(from.x - to.x) < 0.00001;
    if (aTessCoord.x < 0.5) {
        position.x = min(min(from.x, to.x), ctrl.x);
        dilation.x = zeroArea ? 0.0 : -1.0;
    } else {
        position.x = max(max(from.x, to.x), ctrl.x);
        dilation.x = zeroArea ? 0.0 : 1.0;
    }
    if (aTessCoord.y < 0.5) {
        position.y = min(min(from.y, to.y), ctrl.y);
        dilation.y = zeroArea ? 0.0 : -1.0;
    } else {
        position.y = corner.y;
        dilation.y = 0.0;
    }
    position += dilation * 2.0 / vec2(uFramebufferSize);

    // Compute final position and depth.
    vec2 offsetPosition = position + uTransformST.zw + globalTransformLinear * transformST.zw;
    float depth = convertPathIndexToViewportDepthValue(pathID);

    // Compute transformed framebuffer size.
    vec2 framebufferSizeVector = 0.5 * vec2(uFramebufferSize);

    // Finish up.
    gl_Position = vec4(offsetPosition, depth, 1.0);
    vFrom = (from - position) * framebufferSizeVector;
    vCtrl = (ctrl - position) * framebufferSizeVector;
    vTo = (to - position) * framebufferSizeVector;
}
