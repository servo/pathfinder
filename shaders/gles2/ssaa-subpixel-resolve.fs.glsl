// pathfinder/shaders/gles2/ssaa-subpixel-resolve.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Performs subpixel antialiasing for LCD screens by converting a
//! 3x-oversampled RGBA color buffer to an RGB framebuffer, applying the
//! FreeType color defringing filter as necessary.

precision mediump float;

/// The alpha coverage texture.
uniform sampler2D uSource;
/// The dimensions of the alpha coverage texture, in texels.
uniform ivec2 uSourceDimensions;
uniform vec4 uKernel;

varying vec2 vTexCoord;

void main() {
    float onePixel = 1.0 / float(uSourceDimensions.x);
    vec4 shadesL, shadesR;
    float shadeC;
    sample9Tap(shadesL, shadeC, shadesR, uSource, vTexCoord, onePixel, uKernel);

    vec3 shades = vec3(convolve7Tap(shadesL, vec3(shadeC, shadesR.xy), uKernel),
                       convolve7Tap(vec4(shadesL.yzw, shadeC), shadesR.xyz, uKernel),
                       convolve7Tap(vec4(shadesL.zw, shadeC, shadesR.x), shadesR.yzw, uKernel));

    gl_FragColor = vec4(shades, 1.0);
}
