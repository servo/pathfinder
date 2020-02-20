// pathfinder/shaders/filter_text_convolve.inc.glsl
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Zero if no convolution is to be performed.
uniform vec4 uKernel;

// This function is expected to return the alpha value of the pixel at the
// given offset in pixels. Offset 0.0 represents the current pixel.
float sample1Tap(float offset);

// Samples 9 taps around the current pixel.
void sample9Tap(out vec4 outAlphaLeft,
                out float outAlphaCenter,
                out vec4 outAlphaRight,
                float onePixel) {
    outAlphaLeft   = vec4(uKernel.x > 0.0 ? sample1Tap(-4.0 * onePixel) : 0.0,
                          sample1Tap(-3.0 * onePixel),
                          sample1Tap(-2.0 * onePixel),
                          sample1Tap(-1.0 * onePixel));
    outAlphaCenter = sample1Tap(0.0);
    outAlphaRight  = vec4(sample1Tap(1.0 * onePixel),
                          sample1Tap(2.0 * onePixel),
                          sample1Tap(3.0 * onePixel),
                          uKernel.x > 0.0 ? sample1Tap(4.0 * onePixel) : 0.0);
}

// Convolves 7 values with the kernel.
float convolve7Tap(vec4 alpha0, vec3 alpha1) {
    return dot(alpha0, uKernel) + dot(alpha1, uKernel.zyx);
}
