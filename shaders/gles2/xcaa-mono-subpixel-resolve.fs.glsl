// pathfinder/shaders/gles2/xcaa-mono-subpixel-resolve.fs.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Performs subpixel antialiasing for LCD screens by converting a
//! 3x-oversampled single-channel color buffer to an RGB framebuffer, applying
//! the FreeType color defringing filter as necessary.

precision mediump float;

/// The background color of the monochrome path.
uniform vec4 uBGColor;
/// The foreground color of the monochrome path.
uniform vec4 uFGColor;
/// The alpha coverage texture.
uniform sampler2D uAAAlpha;
/// The dimensions of the alpha coverage texture, in texels.
uniform ivec2 uAAAlphaDimensions;
uniform vec4 uKernel;

varying vec2 vTexCoord;

void main() {
    float onePixel = 1.0 / float(uAAAlphaDimensions.x);
    vec4 shadesL, shadesR;
    float shadeC;
    sample9Tap(shadesL, shadeC, shadesR, uAAAlpha, vTexCoord, onePixel, uKernel);

    vec3 shades = vec3(convolve7Tap(shadesL, vec3(shadeC, shadesR.xy), uKernel),
                       convolve7Tap(vec4(shadesL.yzw, shadeC), shadesR.xyz, uKernel),
                       convolve7Tap(vec4(shadesL.zw, shadeC, shadesR.x), shadesR.yzw, uKernel));

    vec3 color = mix(uBGColor.rgb, uFGColor.rgb, shades);
    float alpha = any(greaterThan(shades, vec3(0.0))) ? uFGColor.a : uBGColor.a;
    gl_FragColor = alpha * vec4(color, 1.0);
}
