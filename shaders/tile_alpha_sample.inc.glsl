// pathfinder/shaders/tile_alpha_sample.inc.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

uniform sampler2D uStencilTexture;
uniform sampler2D uPaintTexture;
uniform sampler2D uDest;
uniform vec2 uFramebufferSize;

in vec2 vColorTexCoord;
in vec2 vMaskTexCoord;

// NB: This does not premultiply.
vec4 sampleSrcColor() {
    float coverage = texture(uStencilTexture, vMaskTexCoord).r;
    vec4 srcRGBA = texture(uPaintTexture, vColorTexCoord);
    return vec4(srcRGBA.rgb, srcRGBA.a * coverage);
}

vec4 sampleDestColor() {
    vec2 destTexCoord = gl_FragCoord.xy / uFramebufferSize;
    return texture(uDest, destTexCoord);
}

// FIXME(pcwalton): What should the output alpha be here?
vec4 blendColors(vec4 destRGBA, vec4 srcRGBA, vec3 blendedRGB) {
    return vec4(srcRGBA.a * (1.0 - destRGBA.a) * srcRGBA.rgb +
                srcRGBA.a * destRGBA.a * blendedRGB +
                (1.0 - srcRGBA.a) * destRGBA.a * destRGBA.rgb,
                1.0);
}
