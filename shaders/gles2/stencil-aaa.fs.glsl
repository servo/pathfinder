// pathfinder/shaders/gles2/stencil-aaa.fs.glsl
//
// Copyright (c) 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

varying vec3 vUV;
varying vec3 vXDist;

void main() {
    // Unpack.
    vec3 uv = vUV;
    vec2 dUVDX = dFdx(uv.xy), dUVDY = dFdy(uv.xy);
    vec3 xDist = vXDist;
    vec2 dXDistDX = dFdx(xDist.xz);

    // Calculate X distances between endpoints (x02, x10, and x21 respectively).
    vec3 vDist = xDist - xDist.zxy;

    // Compute winding number and convexity.
    bool inCurve = insideCurve(uv);
    float openWinding = fastSign(-vDist.x);
    float convex = uv.z != 0.0 ? uv.z : fastSign(vDist.x * dUVDY.y);

    // Compute open rect area.
    vec2 areas = clamp(xDist.xz / dXDistDX, -0.5, 0.5);
    float openRectArea = openWinding * (areas.y - areas.x);

    // Compute closed rect area and winding, if necessary.
    float closedRectArea = 0.0, closedWinding = 0.0;
    if (inCurve && vDist.y * vDist.z < 0.0) {
        closedRectArea = 0.5 - fastSign(vDist.y) * (vDist.x * vDist.y < 0.0 ? areas.y : areas.x);
        closedWinding = fastSign(vDist.y * dUVDY.y);
    }

    // Calculate approximate area of the curve covering this pixel square.
    float curveArea = estimateArea(signedDistanceToCurve(uv.xy, dUVDX, dUVDY, inCurve));

    // Calculate alpha.
    vec2 alpha = vec2(openWinding, closedWinding) * 0.5 + convex * curveArea;
    alpha *= vec2(openRectArea, closedRectArea);

    // Finish up.
    gl_FragColor = vec4(alpha.x + alpha.y);
}
