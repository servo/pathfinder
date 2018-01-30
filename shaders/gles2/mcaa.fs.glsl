// pathfinder/shaders/gles2/mcaa.fs.glsl
//
// Copyright (c) 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Renders paths when performing multicolor *mesh coverage antialiasing*
//! (MCAA). This one shader handles both lines and curves.
//!
//! This shader expects to render to a standard RGB color buffer.

precision highp float;

varying vec4 vColor;
varying vec4 vUV;
varying vec4 vSignMode;

// Cubic approximation to the square area coverage, accurate to about 4%.
float estimateArea(float dist) {
    if (dist >= 0.707107)
        return 1.0;
    // Catch NaNs here.
    if (!(dist > -0.707107))
        return 0.0;
    return 0.5 + 1.14191 * dist - 0.83570 * dist * dist * dist;
}

float computeAlpha(vec2 uv, float curveSign, float mode) {
    vec2 dUVDX = dFdx(uv), dUVDY = dFdy(uv);

    // u^2 - v for curves inside uv square; u - v otherwise.
    float g = uv.x;
    vec2 dG = vec2(dUVDX.x, dUVDY.x);
    if (mode > 0.0 && uv.x > 0.0 && uv.x < 1.0 && uv.y > 0.0 && uv.y < 1.0) {
        g *= uv.x;
        dG *= 2.0 * uv.x;
    }
    g -= uv.y;
    dG -= vec2(dUVDX.y, dUVDY.y);

    float signedDistance = g / length(dG);
    return estimateArea(signedDistance * curveSign);
}

void main() {
    float alpha = 1.0;
    alpha -= computeAlpha(vUV.xy, vSignMode.x, vSignMode.z);
    alpha -= computeAlpha(vUV.zw, vSignMode.y, vSignMode.w);
    gl_FragColor = alpha * vColor;
}
