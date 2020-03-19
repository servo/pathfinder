#version 330

// pathfinder/shaders/tile.fs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//      Mask UV 0         Mask UV 1
//          +                 +
//          |                 |
//    +-----v-----+     +-----v-----+
//    |           | MIN |           |
//    |  Mask  0  +----->  Mask  1  +------+
//    |           |     |           |      |
//    +-----------+     +-----------+      v       +-------------+
//                                       Apply     |             |       GPU
//                                       Mask +---->  Composite  +---->Blender
//                                         ^       |             |
//    +-----------+     +-----------+      |       +-------------+
//    |           |     |           |      |
//    |  Color 0  +----->  Color 1  +------+
//    |  Filter   |  ×  |           |
//    |           |     |           |
//    +-----^-----+     +-----^-----+
//          |                 |
//          +                 +
//     Color UV 0        Color UV 1

#extension GL_GOOGLE_include_directive : enable

precision highp float;

#define FRAC_6_PI   1.9098593171027443
#define FRAC_PI_3   1.0471975511965976

#define COMBINER_CTRL_MASK_MASK                 0x3
#define COMBINER_CTRL_MASK_WINDING              0x1
#define COMBINER_CTRL_MASK_EVEN_ODD             0x2

#define COMBINER_CTRL_COLOR_ENABLE_MASK         0x1

#define COMBINER_CTRL_FILTER_MASK               0x3
#define COMBINER_CTRL_FILTER_RADIAL_GRADIENT    0x1
#define COMBINER_CTRL_FILTER_TEXT               0x2
#define COMBINER_CTRL_FILTER_BLUR               0x3

#define COMBINER_CTRL_COMPOSITE_MASK            0xf
#define COMBINER_CTRL_COMPOSITE_NORMAL          0x0
#define COMBINER_CTRL_COMPOSITE_MULTIPLY        0x1
#define COMBINER_CTRL_COMPOSITE_SCREEN          0x2
#define COMBINER_CTRL_COMPOSITE_OVERLAY         0x3
#define COMBINER_CTRL_COMPOSITE_DARKEN          0x4
#define COMBINER_CTRL_COMPOSITE_LIGHTEN         0x5
#define COMBINER_CTRL_COMPOSITE_COLOR_DODGE     0x6
#define COMBINER_CTRL_COMPOSITE_COLOR_BURN      0x7
#define COMBINER_CTRL_COMPOSITE_HARD_LIGHT      0x8
#define COMBINER_CTRL_COMPOSITE_SOFT_LIGHT      0x9
#define COMBINER_CTRL_COMPOSITE_DIFFERENCE      0xa
#define COMBINER_CTRL_COMPOSITE_EXCLUSION       0xb
#define COMBINER_CTRL_COMPOSITE_HUE             0xc
#define COMBINER_CTRL_COMPOSITE_SATURATION      0xd
#define COMBINER_CTRL_COMPOSITE_COLOR           0xe
#define COMBINER_CTRL_COMPOSITE_LUMINOSITY      0xf

#define COMBINER_CTRL_MASK_0_SHIFT              0
#define COMBINER_CTRL_MASK_1_SHIFT              2
#define COMBINER_CTRL_COLOR_0_FILTER_SHIFT      4
#define COMBINER_CTRL_COLOR_0_ENABLE_SHIFT      6
#define COMBINER_CTRL_COLOR_1_ENABLE_SHIFT      7
#define COMBINER_CTRL_COMPOSITE_SHIFT           8

uniform sampler2D uColorTexture0;
uniform sampler2D uColorTexture1;
uniform sampler2D uMaskTexture0;
uniform sampler2D uMaskTexture1;
uniform sampler2D uDestTexture;
uniform sampler2D uGammaLUT;
uniform vec4 uFilterParams0;
uniform vec4 uFilterParams1;
uniform vec4 uFilterParams2;
uniform vec2 uDestTextureSize;
uniform vec2 uColorTexture0Size;
uniform int uCtrl;

in vec3 vMaskTexCoord0;
in vec3 vMaskTexCoord1;
in vec2 vColorTexCoord0;
in vec2 vColorTexCoord1;

out vec4 oFragColor;

// Color sampling

vec4 sampleColor(sampler2D colorTexture, vec2 colorTexCoord) {
    return texture(colorTexture, colorTexCoord);
}

// Text filter

float filterTextSample1Tap(float offset, sampler2D colorTexture, vec2 colorTexCoord) {
    return texture(colorTexture, colorTexCoord + vec2(offset, 0.0)).r;
}

// Samples 9 taps around the current pixel.
void filterTextSample9Tap(out vec4 outAlphaLeft,
                          out float outAlphaCenter,
                          out vec4 outAlphaRight,
                          sampler2D colorTexture,
                          vec2 colorTexCoord,
                          vec4 kernel,
                          float onePixel) {
    bool wide = kernel.x > 0.0;
    outAlphaLeft =
        vec4(wide ? filterTextSample1Tap(-4.0 * onePixel, colorTexture, colorTexCoord) : 0.0,
             filterTextSample1Tap(-3.0 * onePixel, colorTexture, colorTexCoord),
             filterTextSample1Tap(-2.0 * onePixel, colorTexture, colorTexCoord),
             filterTextSample1Tap(-1.0 * onePixel, colorTexture, colorTexCoord));
    outAlphaCenter = filterTextSample1Tap(0.0, colorTexture, colorTexCoord);
    outAlphaRight =
        vec4(filterTextSample1Tap(1.0 * onePixel, colorTexture, colorTexCoord),
             filterTextSample1Tap(2.0 * onePixel, colorTexture, colorTexCoord),
             filterTextSample1Tap(3.0 * onePixel, colorTexture, colorTexCoord),
             wide ? filterTextSample1Tap(4.0 * onePixel, colorTexture, colorTexCoord) : 0.0);
}

float filterTextConvolve7Tap(vec4 alpha0, vec3 alpha1, vec4 kernel) {
    return dot(alpha0, kernel) + dot(alpha1, kernel.zyx);
}

float filterTextGammaCorrectChannel(float bgColor, float fgColor, sampler2D gammaLUT) {
    return texture(gammaLUT, vec2(fgColor, 1.0 - bgColor)).r;
}

// `fgColor` is in linear space.
vec3 filterTextGammaCorrect(vec3 bgColor, vec3 fgColor, sampler2D gammaLUT) {
    return vec3(filterTextGammaCorrectChannel(bgColor.r, fgColor.r, gammaLUT),
                filterTextGammaCorrectChannel(bgColor.g, fgColor.g, gammaLUT),
                filterTextGammaCorrectChannel(bgColor.b, fgColor.b, gammaLUT));
}

//                | x          y          z          w
//  --------------+--------------------------------------------------------
//  filterParams0 | kernel[0]  kernel[1]  kernel[2]  kernel[3]
//  filterParams1 | bgColor.r  bgColor.g  bgColor.b  -
//  filterParams2 | fgColor.r  fgColor.g  fgColor.b  gammaCorrectionEnabled
vec4 filterText(vec2 colorTexCoord,
                sampler2D colorTexture,
                sampler2D gammaLUT,
                vec2 colorTextureSize,
                vec4 filterParams0,
                vec4 filterParams1,
                vec4 filterParams2) {
    // Unpack.
    vec4 kernel = filterParams0;
    vec3 bgColor = filterParams1.rgb;
    vec3 fgColor = filterParams2.rgb;
    bool gammaCorrectionEnabled = filterParams2.a != 0.0;

    // Apply defringing if necessary.
    vec3 alpha;
    if (kernel.w == 0.0) {
        alpha = texture(colorTexture, colorTexCoord).rrr;
    } else {
        vec4 alphaLeft, alphaRight;
        float alphaCenter;
        filterTextSample9Tap(alphaLeft,
                             alphaCenter,
                             alphaRight,
                             colorTexture,
                             colorTexCoord,
                             kernel,
                             1.0 / colorTextureSize.x);

        float r = filterTextConvolve7Tap(alphaLeft, vec3(alphaCenter, alphaRight.xy), kernel);
        float g = filterTextConvolve7Tap(vec4(alphaLeft.yzw, alphaCenter), alphaRight.xyz, kernel);
        float b = filterTextConvolve7Tap(vec4(alphaLeft.zw, alphaCenter, alphaRight.x),
                                         alphaRight.yzw,
                                         kernel);

        alpha = vec3(r, g, b);
    }

    // Apply gamma correction if necessary.
    if (gammaCorrectionEnabled)
        alpha = filterTextGammaCorrect(bgColor, alpha, gammaLUT);

    // Finish.
    return vec4(mix(bgColor, fgColor, alpha), 1.0);
}

// Filters

//                | x             y             z             w
//  --------------+----------------------------------------------------
//  filterParams0 | srcOffset.x   srcOffset.y   support       -
//  filterParams1 | gaussCoeff.x  gaussCoeff.y  gaussCoeff.z  -
//  filterParams2 | -             -                 -             -
vec4 filterBlur(vec2 colorTexCoord,
                sampler2D colorTexture,
                vec2 colorTextureSize,
                vec4 filterParams0,
                vec4 filterParams1) {
    // Unpack.
    vec2 srcOffsetScale = filterParams0.xy / colorTextureSize;
    int support = int(filterParams0.z);
    vec3 gaussCoeff = filterParams1.xyz;

    // Set up our incremental calculation.
    float gaussSum = gaussCoeff.x;
    vec4 color = texture(colorTexture, colorTexCoord) * gaussCoeff.x;
    gaussCoeff.xy *= gaussCoeff.yz;

    // This is a common trick that lets us use the texture filtering hardware to evaluate two
    // texels at a time. The basic principle is that, if c0 and c1 are colors of adjacent texels
    // and k0 and k1 are arbitrary factors, the formula `k0 * c0 + k1 * c1` is equivalent to
    // `(k0 + k1) * lerp(c0, c1, k1 / (k0 + k1))`. Linear interpolation, as performed by the
    // texturing hardware when sampling adjacent pixels in one direction, evaluates
    // `lerp(c0, c1, t)` where t is the offset from the texel with color `c0`. To evaluate the
    // formula `k0 * c0 + k1 * c1`, therefore, we can use the texture hardware to perform linear
    // interpolation with `t = k1 / (k0 + k1)`.
    for (int i = 1; i <= support; i += 2) {
        float gaussPartialSum = gaussCoeff.x;
        gaussCoeff.xy *= gaussCoeff.yz;
        gaussPartialSum += gaussCoeff.x;

        vec2 srcOffset = srcOffsetScale * (float(i) + gaussCoeff.x / gaussPartialSum);
        color += (texture(colorTexture, colorTexCoord - srcOffset) +
                  texture(colorTexture, colorTexCoord + srcOffset)) * gaussPartialSum;

        gaussSum += 2.0 * gaussPartialSum;
        gaussCoeff.xy *= gaussCoeff.yz;
    }

    // Finish.
    return color / gaussSum;
}

vec4 filterNone(vec2 colorTexCoord, sampler2D colorTexture) {
    return sampleColor(colorTexture, colorTexCoord);
}

vec4 filterColor(vec2 colorTexCoord,
                 sampler2D colorTexture,
                 sampler2D gammaLUT,
                 vec2 colorTextureSize,
                 vec4 filterParams0,
                 vec4 filterParams1,
                 vec4 filterParams2,
                 int colorFilter) {
    switch (colorFilter) {
    case COMBINER_CTRL_FILTER_BLUR:
        return filterBlur(colorTexCoord,
                          colorTexture,
                          colorTextureSize,
                          filterParams0,
                          filterParams1);
    case COMBINER_CTRL_FILTER_TEXT:
        return filterText(colorTexCoord,
                          colorTexture,
                          gammaLUT,
                          colorTextureSize,
                          filterParams0,
                          filterParams1,
                          filterParams2);
    }
    return filterNone(colorTexCoord, colorTexture);
}

// Compositing

vec3 compositeSelect(bvec3 cond, vec3 ifTrue, vec3 ifFalse) {
    return vec3(cond.x ? ifTrue.x : ifFalse.x,
                cond.y ? ifTrue.y : ifFalse.y,
                cond.z ? ifTrue.z : ifFalse.z);
}

float compositeDivide(float num, float denom) {
    return denom != 0.0 ? num / denom : 0.0;
}

vec3 compositeColorDodge(vec3 destColor, vec3 srcColor) {
    bvec3 destZero = equal(destColor, vec3(0.0)), srcOne = equal(srcColor, vec3(1.0));
    return compositeSelect(destZero,
                           vec3(0.0),
                           compositeSelect(srcOne, vec3(1.0), destColor / (vec3(1.0) - srcColor)));
}

// https://en.wikipedia.org/wiki/HSL_and_HSV#HSL_to_RGB_alternative
vec3 compositeHSLToRGB(vec3 hsl) {
    float a = hsl.y * min(hsl.z, 1.0 - hsl.z);
    vec3 ks = mod(vec3(0.0, 8.0, 4.0) + vec3(hsl.x * FRAC_6_PI), 12.0);
    return hsl.zzz - clamp(min(ks - vec3(3.0), vec3(9.0) - ks), -1.0, 1.0) * a;
}

// https://en.wikipedia.org/wiki/HSL_and_HSV#From_RGB
vec3 compositeRGBToHSL(vec3 rgb) {
    float v = max(max(rgb.r, rgb.g), rgb.b), xMin = min(min(rgb.r, rgb.g), rgb.b);
    float c = v - xMin, l = mix(xMin, v, 0.5);
    vec3 terms = rgb.r == v ? vec3(0.0, rgb.gb) :
                 rgb.g == v ? vec3(2.0, rgb.br) :
                              vec3(4.0, rgb.rg);
    float h = FRAC_PI_3 * compositeDivide(terms.x * c + terms.y - terms.z, c);
    float s = compositeDivide(c, v);
    return vec3(h, s, l);
}

vec3 compositeScreen(vec3 destColor, vec3 srcColor) {
    return destColor + srcColor - destColor * srcColor;
}

vec3 compositeHardLight(vec3 destColor, vec3 srcColor) {
    return compositeSelect(lessThanEqual(srcColor, vec3(0.5)),
                           destColor * vec3(2.0) * srcColor,
                           compositeScreen(destColor, vec3(2.0) * srcColor - vec3(1.0)));
}

vec3 compositeSoftLight(vec3 destColor, vec3 srcColor) {
    vec3 darkenedDestColor =
        compositeSelect(lessThanEqual(destColor, vec3(0.25)),
                        ((vec3(16.0) * destColor - 12.0) * destColor + 4.0) * destColor,
                        sqrt(destColor));
    vec3 factor = compositeSelect(lessThanEqual(srcColor, vec3(0.5)),
                                  destColor * (vec3(1.0) - destColor),
                                  darkenedDestColor - destColor);
    return destColor + (srcColor * 2.0 - 1.0) * factor;
}

vec3 compositeHSL(vec3 destColor, vec3 srcColor, int op) {
    switch (op) {
    case COMBINER_CTRL_COMPOSITE_HUE:
        return vec3(srcColor.x,  destColor.y, destColor.z);
    case COMBINER_CTRL_COMPOSITE_SATURATION:
        return vec3(destColor.x, srcColor.y,  destColor.z);
    case COMBINER_CTRL_COMPOSITE_COLOR:
        return vec3(srcColor.x,  srcColor.y,  destColor.z);
    default:
        return vec3(destColor.x, destColor.y, srcColor.z);
    }
}

vec3 compositeRGB(vec3 destColor, vec3 srcColor, int op) {
    switch (op) {
    case COMBINER_CTRL_COMPOSITE_MULTIPLY:
        return destColor * srcColor;
    case COMBINER_CTRL_COMPOSITE_SCREEN:
        return compositeScreen(destColor, srcColor);
    case COMBINER_CTRL_COMPOSITE_OVERLAY:
        return compositeHardLight(srcColor, destColor);
    case COMBINER_CTRL_COMPOSITE_DARKEN:
        return min(destColor, srcColor);
    case COMBINER_CTRL_COMPOSITE_LIGHTEN:
        return max(destColor, srcColor);
    case COMBINER_CTRL_COMPOSITE_COLOR_DODGE:
        return compositeColorDodge(destColor, srcColor);
    case COMBINER_CTRL_COMPOSITE_COLOR_BURN:
        return vec3(1.0) - compositeColorDodge(vec3(1.0) - destColor, vec3(1.0) - srcColor);
    case COMBINER_CTRL_COMPOSITE_HARD_LIGHT:
        return compositeHardLight(destColor, srcColor);
    case COMBINER_CTRL_COMPOSITE_SOFT_LIGHT:
        return compositeSoftLight(destColor, srcColor);
    case COMBINER_CTRL_COMPOSITE_DIFFERENCE:
        return abs(destColor - srcColor);
    case COMBINER_CTRL_COMPOSITE_EXCLUSION:
        return destColor + srcColor - vec3(2.0) * destColor * srcColor;
    case COMBINER_CTRL_COMPOSITE_HUE:
    case COMBINER_CTRL_COMPOSITE_SATURATION:
    case COMBINER_CTRL_COMPOSITE_COLOR:
    case COMBINER_CTRL_COMPOSITE_LUMINOSITY:
        return compositeHSLToRGB(compositeHSL(compositeRGBToHSL(destColor),
                                              compositeRGBToHSL(srcColor),
                                              op));
    }
    return srcColor;
}

vec4 composite(vec4 srcColor,
               sampler2D destTexture,
               vec2 destTextureSize,
               vec2 fragCoord,
               int op) {
    if (op == COMBINER_CTRL_COMPOSITE_NORMAL)
        return srcColor;

    // FIXME(pcwalton): What should the output alpha be here?
    vec2 destTexCoord = fragCoord / destTextureSize;
    vec4 destColor = texture(destTexture, destTexCoord);
    vec3 blendedRGB = compositeRGB(destColor.rgb, srcColor.rgb, op);
    return vec4(srcColor.a * (1.0 - destColor.a) * srcColor.rgb +
                srcColor.a * destColor.a * blendedRGB +
                (1.0 - srcColor.a) * destColor.rgb,
                1.0);
}

// Masks

float sampleMask(float maskAlpha,
                 sampler2D maskTexture,
                 vec3 maskTexCoord,
                 int maskCtrl) {
    if (maskCtrl == 0)
        return maskAlpha;
    float coverage = texture(maskTexture, maskTexCoord.xy).r + maskTexCoord.z;
    if ((maskCtrl & COMBINER_CTRL_MASK_WINDING) != 0)
        coverage = abs(coverage);
    else
        coverage = 1.0 - abs(1.0 - mod(coverage, 2.0));
    return min(maskAlpha, coverage);
}

// Main function

void calculateColor(int ctrl) {
    // Sample mask.
    int maskCtrl0 = (ctrl >> COMBINER_CTRL_MASK_0_SHIFT) & COMBINER_CTRL_MASK_MASK;
    int maskCtrl1 = (ctrl >> COMBINER_CTRL_MASK_1_SHIFT) & COMBINER_CTRL_MASK_MASK;
    float maskAlpha = 1.0;
    maskAlpha = sampleMask(maskAlpha, uMaskTexture0, vMaskTexCoord0, maskCtrl0);
    maskAlpha = sampleMask(maskAlpha, uMaskTexture1, vMaskTexCoord1, maskCtrl1);

    // Sample color.
    vec4 color = vec4(0.0);
    if (((ctrl >> COMBINER_CTRL_COLOR_0_ENABLE_SHIFT) & COMBINER_CTRL_COLOR_ENABLE_MASK) != 0) {
        int color0Filter = (ctrl >> COMBINER_CTRL_COLOR_0_FILTER_SHIFT) &
            COMBINER_CTRL_FILTER_MASK;
        color += filterColor(vColorTexCoord0,
                             uColorTexture0,
                             uGammaLUT,
                             uColorTexture0Size,
                             uFilterParams0,
                             uFilterParams1,
                             uFilterParams2,
                             color0Filter);
    }
    if (((ctrl >> COMBINER_CTRL_COLOR_1_ENABLE_SHIFT) & COMBINER_CTRL_COLOR_ENABLE_MASK) != 0)
        color *= sampleColor(uColorTexture1, vColorTexCoord1);

    // Apply mask.
    color.a *= maskAlpha;

    // Apply composite.
    int compositeOp = (ctrl >> COMBINER_CTRL_COMPOSITE_SHIFT) & COMBINER_CTRL_COMPOSITE_MASK;
    color = composite(color, uDestTexture, uDestTextureSize, gl_FragCoord.xy, compositeOp);

    // Premultiply alpha.
    color.rgb *= color.a;
    oFragColor = color;
}

// Entry point
//
// TODO(pcwalton): Generate this dynamically.

void main() {
    calculateColor(uCtrl);
}
