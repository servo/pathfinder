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

void main() {
    // Unpack.
    vec3 uv = vUV;
    vec2 dUVDX = dFdx(uv.xy), dUVDY = dFdy(uv.xy);

    // Calculate the reciprocal of the Jacobian determinant. This will be useful for determining
    // distance from endpoints.
    //
    // http://pcwalton.github.io/2018/02/14/determining-triangle-geometry-in-fragment-shaders.html
    float recipJ = 1.0 / det2(mat2(dUVDX, dUVDY));

    // Calculate X distances between endpoints.
    float v02DX = dUVDY.y - dUVDY.x, v10DX = -dUVDY.y, v21DX = 2.0 * dUVDY.x - dUVDY.y;
    float v02X = v02DX * recipJ, v10X = v10DX * recipJ;

    // Compute winding number and convexity.
    bool inCurve = insideCurve(uv);
    float openWinding = fastSign(-v02X);
    float convex = uv.z != 0.0 ? uv.z : -fastSign(dUVDY.y) * openWinding;

    // Compute open rect area.
    vec2 areas = clamp(vec2(det2(mat2(uv.xy, dUVDY))) * recipJ - vec2(0.0, v02X), -0.5, 0.5);
    float openRectArea = openWinding * (areas.y - areas.x);

    // Compute closed rect area and winding, if necessary.
    float closedRectArea = 0.0, closedWinding = 0.0;
    if (inCurve && v10DX * v21DX < 0.0) {
        closedRectArea = 0.5 - fastSign(v10X) * (v10X * openWinding < 0.0 ? areas.x : areas.y);
        closedWinding = fastSign((dUVDX.y - dUVDX.x) * dUVDY.y);
    }

    // Calculate approximate area of the curve covering this pixel square.
    float curveArea = estimateArea(signedDistanceToCurve(uv.xy, dUVDX, dUVDY, inCurve));

    // Calculate alpha.
    vec2 alpha = vec2(openWinding, closedWinding) * 0.5 + convex * curveArea;
    alpha *= vec2(openRectArea, closedRectArea);

    // Finish up.
    gl_FragColor = vec4(alpha.x + alpha.y);
}
