// pathfinder/shaders/gles2/direct-curve.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implements the quadratic Loop-Blinn formulation to render curved parts of
//! the mesh.
//!
//! This shader performs no antialiasing; if you want antialiased output from
//! this shader, use MSAA with sample-level shading (GL 4.x) or else perform
//! SSAA by rendering to a higher-resolution framebuffer and downsampling (GL
//! 3.x and below).
//!
//! If you know your mesh has no curves (i.e. it consists solely of polygons),
//! then you don't need to run this shader.

precision highp float;

/// The fill color of this path.
varying vec4 vColor;
/// The abstract Loop-Blinn texture coordinate.
varying vec2 vTexCoord;

void main() {
    float side = sign(vTexCoord.x * vTexCoord.x - vTexCoord.y);
    float winding = gl_FrontFacing ? -1.0 : 1.0;
    float alpha = float(side == winding);
    //float alpha = mod(gl_FragCoord.x, 2.0) < 1.0 ? 1.0 : 0.0;
    gl_FragColor = alpha * vColor;
}
