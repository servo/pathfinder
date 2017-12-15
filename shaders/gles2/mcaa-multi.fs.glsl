// pathfinder/shaders/gles2/mcaa-multi.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

varying vec4 vUpperEndpoints;
varying vec4 vLowerEndpoints;
varying vec4 vControlPoints;
varying vec4 vColor;

float computeCoverageForSide(vec2 p0, vec2 cp, vec2 p1, float winding) {
    // Compute pixel extents.
    vec2 pixelCenter = gl_FragCoord.xy;
    vec2 pixelColumnBounds = pixelCenter.xx + vec2(-0.5, 0.5);

    vec2 clippedP0, clippedDP;
    if (cp == vec2(0.0)) {
        vec4 p0DPX = clipLineToPixelColumn(p0, p1 - p0, pixelCenter.x);
        clippedP0 = p0DPX.xy;
        clippedDP = p0DPX.zw;
    } else {
        // Clip the curve to the left and right edges to create a line.
        vec2 t = solveCurveT(p0.x, cp.x, p1.x, pixelColumnBounds);

        // Handle endpoints properly. These tests are negated to handle NaNs.
        if (!(p0.x < pixelColumnBounds.x))
            t.x = 0.0;
        if (!(p1.x > pixelColumnBounds.y))
            t.y = 1.0;

        clippedP0 = mix(mix(p0, cp, t.x), mix(cp, p1, t.x), t.x);
        clippedDP = mix(mix(p0, cp, t.y), mix(cp, p1, t.y), t.y) - clippedP0;
    }

    return computeCoverage(clippedP0, clippedDP, pixelCenter.y, winding);
}

void main() {
    float alpha = computeCoverageForSide(vLowerEndpoints.xy,
                                         vControlPoints.zw,
                                         vLowerEndpoints.zw,
                                         -1.0);

    alpha += computeCoverageForSide(vUpperEndpoints.xy,
                                    vControlPoints.xy,
                                    vUpperEndpoints.zw,
                                    1.0);

    // Compute area.
    gl_FragColor = vec4(vColor.rgb, vColor.a * alpha);
}
