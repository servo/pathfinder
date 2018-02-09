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

void main() {
    bool inUpperCurve = insideCurve(vec3(vUV.xy, vSignMode.z > 0.0 ? 1.0 : 0.0));
    bool inLowerCurve = insideCurve(vec3(vUV.zw, vSignMode.w > 0.0 ? 1.0 : 0.0));

    float upperDist = signedDistanceToCurve(vUV.xy, dFdx(vUV.xy), dFdy(vUV.xy), inUpperCurve);
    float lowerDist = signedDistanceToCurve(vUV.zw, dFdx(vUV.zw), dFdy(vUV.zw), inLowerCurve);

    float alpha = -estimateArea(upperDist * vSignMode.x) - estimateArea(lowerDist * vSignMode.y);
    gl_FragColor = alpha * vColor;
}
