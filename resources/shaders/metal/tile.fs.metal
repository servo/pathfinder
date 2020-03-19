// Automatically generated from files in pathfinder/shaders/. Do not edit!
#pragma clang diagnostic ignored "-Wmissing-prototypes"

#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct spvDescriptorSetBuffer0
{
    texture2d<float> uMaskTexture0 [[id(0)]];
    sampler uMaskTexture0Smplr [[id(1)]];
    texture2d<float> uMaskTexture1 [[id(2)]];
    sampler uMaskTexture1Smplr [[id(3)]];
    texture2d<float> uColorTexture0 [[id(4)]];
    sampler uColorTexture0Smplr [[id(5)]];
    texture2d<float> uGammaLUT [[id(6)]];
    sampler uGammaLUTSmplr [[id(7)]];
    constant float2* uColorTexture0Size [[id(8)]];
    constant float4* uFilterParams0 [[id(9)]];
    constant float4* uFilterParams1 [[id(10)]];
    constant float4* uFilterParams2 [[id(11)]];
    texture2d<float> uColorTexture1 [[id(12)]];
    sampler uColorTexture1Smplr [[id(13)]];
    texture2d<float> uDestTexture [[id(14)]];
    sampler uDestTextureSmplr [[id(15)]];
    constant float2* uDestTextureSize [[id(16)]];
    constant int* uCtrl [[id(17)]];
};

constant float3 _862 = {};

struct main0_out
{
    float4 oFragColor [[color(0)]];
};

struct main0_in
{
    float3 vMaskTexCoord0 [[user(locn0)]];
    float3 vMaskTexCoord1 [[user(locn1)]];
    float2 vColorTexCoord0 [[user(locn2)]];
    float2 vColorTexCoord1 [[user(locn3)]];
};

// Implementation of the GLSL mod() function, which is slightly different than Metal fmod()
template<typename Tx, typename Ty>
Tx mod(Tx x, Ty y)
{
    return x - y * floor(x / y);
}

float sampleMask(thread const float& maskAlpha, thread const texture2d<float> maskTexture, thread const sampler maskTextureSmplr, thread const float3& maskTexCoord, thread const int& maskCtrl)
{
    if (maskCtrl == 0)
    {
        return maskAlpha;
    }
    float coverage = maskTexture.sample(maskTextureSmplr, maskTexCoord.xy).x + maskTexCoord.z;
    if ((maskCtrl & 1) != 0)
    {
        coverage = abs(coverage);
    }
    else
    {
        coverage = 1.0 - abs(1.0 - mod(coverage, 2.0));
    }
    return fast::min(maskAlpha, coverage);
}

float4 filterBlur(thread const float2& colorTexCoord, thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr, thread const float2& colorTextureSize, thread const float4& filterParams0, thread const float4& filterParams1)
{
    float2 srcOffsetScale = filterParams0.xy / colorTextureSize;
    int support = int(filterParams0.z);
    float3 gaussCoeff = filterParams1.xyz;
    float gaussSum = gaussCoeff.x;
    float4 color = colorTexture.sample(colorTextureSmplr, colorTexCoord) * gaussCoeff.x;
    float2 _435 = gaussCoeff.xy * gaussCoeff.yz;
    gaussCoeff = float3(_435.x, _435.y, gaussCoeff.z);
    for (int i = 1; i <= support; i += 2)
    {
        float gaussPartialSum = gaussCoeff.x;
        float2 _455 = gaussCoeff.xy * gaussCoeff.yz;
        gaussCoeff = float3(_455.x, _455.y, gaussCoeff.z);
        gaussPartialSum += gaussCoeff.x;
        float2 srcOffset = srcOffsetScale * (float(i) + (gaussCoeff.x / gaussPartialSum));
        color += ((colorTexture.sample(colorTextureSmplr, (colorTexCoord - srcOffset)) + colorTexture.sample(colorTextureSmplr, (colorTexCoord + srcOffset))) * gaussPartialSum);
        gaussSum += (2.0 * gaussPartialSum);
        float2 _495 = gaussCoeff.xy * gaussCoeff.yz;
        gaussCoeff = float3(_495.x, _495.y, gaussCoeff.z);
    }
    return color / float4(gaussSum);
}

float filterTextSample1Tap(thread const float& offset, thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr, thread const float2& colorTexCoord)
{
    return colorTexture.sample(colorTextureSmplr, (colorTexCoord + float2(offset, 0.0))).x;
}

void filterTextSample9Tap(thread float4& outAlphaLeft, thread float& outAlphaCenter, thread float4& outAlphaRight, thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr, thread const float2& colorTexCoord, thread const float4& kernel0, thread const float& onePixel)
{
    bool wide = kernel0.x > 0.0;
    float _183;
    if (wide)
    {
        float param = (-4.0) * onePixel;
        float2 param_1 = colorTexCoord;
        _183 = filterTextSample1Tap(param, colorTexture, colorTextureSmplr, param_1);
    }
    else
    {
        _183 = 0.0;
    }
    float param_2 = (-3.0) * onePixel;
    float2 param_3 = colorTexCoord;
    float param_4 = (-2.0) * onePixel;
    float2 param_5 = colorTexCoord;
    float param_6 = (-1.0) * onePixel;
    float2 param_7 = colorTexCoord;
    outAlphaLeft = float4(_183, filterTextSample1Tap(param_2, colorTexture, colorTextureSmplr, param_3), filterTextSample1Tap(param_4, colorTexture, colorTextureSmplr, param_5), filterTextSample1Tap(param_6, colorTexture, colorTextureSmplr, param_7));
    float param_8 = 0.0;
    float2 param_9 = colorTexCoord;
    outAlphaCenter = filterTextSample1Tap(param_8, colorTexture, colorTextureSmplr, param_9);
    float param_10 = 1.0 * onePixel;
    float2 param_11 = colorTexCoord;
    float param_12 = 2.0 * onePixel;
    float2 param_13 = colorTexCoord;
    float param_14 = 3.0 * onePixel;
    float2 param_15 = colorTexCoord;
    float _243;
    if (wide)
    {
        float param_16 = 4.0 * onePixel;
        float2 param_17 = colorTexCoord;
        _243 = filterTextSample1Tap(param_16, colorTexture, colorTextureSmplr, param_17);
    }
    else
    {
        _243 = 0.0;
    }
    outAlphaRight = float4(filterTextSample1Tap(param_10, colorTexture, colorTextureSmplr, param_11), filterTextSample1Tap(param_12, colorTexture, colorTextureSmplr, param_13), filterTextSample1Tap(param_14, colorTexture, colorTextureSmplr, param_15), _243);
}

float filterTextConvolve7Tap(thread const float4& alpha0, thread const float3& alpha1, thread const float4& kernel0)
{
    return dot(alpha0, kernel0) + dot(alpha1, kernel0.zyx);
}

float filterTextGammaCorrectChannel(thread const float& bgColor, thread const float& fgColor, thread const texture2d<float> gammaLUT, thread const sampler gammaLUTSmplr)
{
    return gammaLUT.sample(gammaLUTSmplr, float2(fgColor, 1.0 - bgColor)).x;
}

float3 filterTextGammaCorrect(thread const float3& bgColor, thread const float3& fgColor, thread const texture2d<float> gammaLUT, thread const sampler gammaLUTSmplr)
{
    float param = bgColor.x;
    float param_1 = fgColor.x;
    float param_2 = bgColor.y;
    float param_3 = fgColor.y;
    float param_4 = bgColor.z;
    float param_5 = fgColor.z;
    return float3(filterTextGammaCorrectChannel(param, param_1, gammaLUT, gammaLUTSmplr), filterTextGammaCorrectChannel(param_2, param_3, gammaLUT, gammaLUTSmplr), filterTextGammaCorrectChannel(param_4, param_5, gammaLUT, gammaLUTSmplr));
}

float4 filterText(thread const float2& colorTexCoord, thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr, thread const texture2d<float> gammaLUT, thread const sampler gammaLUTSmplr, thread const float2& colorTextureSize, thread const float4& filterParams0, thread const float4& filterParams1, thread const float4& filterParams2)
{
    float4 kernel0 = filterParams0;
    float3 bgColor = filterParams1.xyz;
    float3 fgColor = filterParams2.xyz;
    bool gammaCorrectionEnabled = filterParams2.w != 0.0;
    float3 alpha;
    if (kernel0.w == 0.0)
    {
        alpha = colorTexture.sample(colorTextureSmplr, colorTexCoord).xxx;
    }
    else
    {
        float2 param_3 = colorTexCoord;
        float4 param_4 = kernel0;
        float param_5 = 1.0 / colorTextureSize.x;
        float4 param;
        float param_1;
        float4 param_2;
        filterTextSample9Tap(param, param_1, param_2, colorTexture, colorTextureSmplr, param_3, param_4, param_5);
        float4 alphaLeft = param;
        float alphaCenter = param_1;
        float4 alphaRight = param_2;
        float4 param_6 = alphaLeft;
        float3 param_7 = float3(alphaCenter, alphaRight.xy);
        float4 param_8 = kernel0;
        float r = filterTextConvolve7Tap(param_6, param_7, param_8);
        float4 param_9 = float4(alphaLeft.yzw, alphaCenter);
        float3 param_10 = alphaRight.xyz;
        float4 param_11 = kernel0;
        float g = filterTextConvolve7Tap(param_9, param_10, param_11);
        float4 param_12 = float4(alphaLeft.zw, alphaCenter, alphaRight.x);
        float3 param_13 = alphaRight.yzw;
        float4 param_14 = kernel0;
        float b = filterTextConvolve7Tap(param_12, param_13, param_14);
        alpha = float3(r, g, b);
    }
    if (gammaCorrectionEnabled)
    {
        float3 param_15 = bgColor;
        float3 param_16 = alpha;
        alpha = filterTextGammaCorrect(param_15, param_16, gammaLUT, gammaLUTSmplr);
    }
    return float4(mix(bgColor, fgColor, alpha), 1.0);
}

float4 sampleColor(thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr, thread const float2& colorTexCoord)
{
    return colorTexture.sample(colorTextureSmplr, colorTexCoord);
}

float4 filterNone(thread const float2& colorTexCoord, thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr)
{
    float2 param = colorTexCoord;
    return sampleColor(colorTexture, colorTextureSmplr, param);
}

float4 filterColor(thread const float2& colorTexCoord, thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr, thread const texture2d<float> gammaLUT, thread const sampler gammaLUTSmplr, thread const float2& colorTextureSize, thread const float4& filterParams0, thread const float4& filterParams1, thread const float4& filterParams2, thread const int& colorFilter)
{
    switch (colorFilter)
    {
        case 3:
        {
            float2 param = colorTexCoord;
            float2 param_1 = colorTextureSize;
            float4 param_2 = filterParams0;
            float4 param_3 = filterParams1;
            return filterBlur(param, colorTexture, colorTextureSmplr, param_1, param_2, param_3);
        }
        case 2:
        {
            float2 param_4 = colorTexCoord;
            float2 param_5 = colorTextureSize;
            float4 param_6 = filterParams0;
            float4 param_7 = filterParams1;
            float4 param_8 = filterParams2;
            return filterText(param_4, colorTexture, colorTextureSmplr, gammaLUT, gammaLUTSmplr, param_5, param_6, param_7, param_8);
        }
    }
    float2 param_9 = colorTexCoord;
    return filterNone(param_9, colorTexture, colorTextureSmplr);
}

float3 compositeScreen(thread const float3& destColor, thread const float3& srcColor)
{
    return (destColor + srcColor) - (destColor * srcColor);
}

float3 compositeSelect(thread const bool3& cond, thread const float3& ifTrue, thread const float3& ifFalse)
{
    float _546;
    if (cond.x)
    {
        _546 = ifTrue.x;
    }
    else
    {
        _546 = ifFalse.x;
    }
    float _557;
    if (cond.y)
    {
        _557 = ifTrue.y;
    }
    else
    {
        _557 = ifFalse.y;
    }
    float _568;
    if (cond.z)
    {
        _568 = ifTrue.z;
    }
    else
    {
        _568 = ifFalse.z;
    }
    return float3(_546, _557, _568);
}

float3 compositeHardLight(thread const float3& destColor, thread const float3& srcColor)
{
    float3 param = destColor;
    float3 param_1 = (float3(2.0) * srcColor) - float3(1.0);
    bool3 param_2 = srcColor <= float3(0.5);
    float3 param_3 = (destColor * float3(2.0)) * srcColor;
    float3 param_4 = compositeScreen(param, param_1);
    return compositeSelect(param_2, param_3, param_4);
}

float3 compositeColorDodge(thread const float3& destColor, thread const float3& srcColor)
{
    bool3 destZero = destColor == float3(0.0);
    bool3 srcOne = srcColor == float3(1.0);
    bool3 param = srcOne;
    float3 param_1 = float3(1.0);
    float3 param_2 = destColor / (float3(1.0) - srcColor);
    bool3 param_3 = destZero;
    float3 param_4 = float3(0.0);
    float3 param_5 = compositeSelect(param, param_1, param_2);
    return compositeSelect(param_3, param_4, param_5);
}

float3 compositeSoftLight(thread const float3& destColor, thread const float3& srcColor)
{
    bool3 param = destColor <= float3(0.25);
    float3 param_1 = ((((float3(16.0) * destColor) - float3(12.0)) * destColor) + float3(4.0)) * destColor;
    float3 param_2 = sqrt(destColor);
    float3 darkenedDestColor = compositeSelect(param, param_1, param_2);
    bool3 param_3 = srcColor <= float3(0.5);
    float3 param_4 = destColor * (float3(1.0) - destColor);
    float3 param_5 = darkenedDestColor - destColor;
    float3 factor = compositeSelect(param_3, param_4, param_5);
    return destColor + (((srcColor * 2.0) - float3(1.0)) * factor);
}

float compositeDivide(thread const float& num, thread const float& denom)
{
    float _582;
    if (denom != 0.0)
    {
        _582 = num / denom;
    }
    else
    {
        _582 = 0.0;
    }
    return _582;
}

float3 compositeRGBToHSL(thread const float3& rgb)
{
    float v = fast::max(fast::max(rgb.x, rgb.y), rgb.z);
    float xMin = fast::min(fast::min(rgb.x, rgb.y), rgb.z);
    float c = v - xMin;
    float l = mix(xMin, v, 0.5);
    float3 _688;
    if (rgb.x == v)
    {
        _688 = float3(0.0, rgb.yz);
    }
    else
    {
        float3 _701;
        if (rgb.y == v)
        {
            _701 = float3(2.0, rgb.zx);
        }
        else
        {
            _701 = float3(4.0, rgb.xy);
        }
        _688 = _701;
    }
    float3 terms = _688;
    float param = ((terms.x * c) + terms.y) - terms.z;
    float param_1 = c;
    float h = 1.0471975803375244140625 * compositeDivide(param, param_1);
    float param_2 = c;
    float param_3 = v;
    float s = compositeDivide(param_2, param_3);
    return float3(h, s, l);
}

float3 compositeHSL(thread const float3& destColor, thread const float3& srcColor, thread const int& op)
{
    switch (op)
    {
        case 12:
        {
            return float3(srcColor.x, destColor.y, destColor.z);
        }
        case 13:
        {
            return float3(destColor.x, srcColor.y, destColor.z);
        }
        case 14:
        {
            return float3(srcColor.x, srcColor.y, destColor.z);
        }
        default:
        {
            return float3(destColor.x, destColor.y, srcColor.z);
        }
    }
}

float3 compositeHSLToRGB(thread const float3& hsl)
{
    float a = hsl.y * fast::min(hsl.z, 1.0 - hsl.z);
    float3 ks = mod(float3(0.0, 8.0, 4.0) + float3(hsl.x * 1.90985929965972900390625), float3(12.0));
    return hsl.zzz - (fast::clamp(fast::min(ks - float3(3.0), float3(9.0) - ks), float3(-1.0), float3(1.0)) * a);
}

float3 compositeRGB(thread const float3& destColor, thread const float3& srcColor, thread const int& op)
{
    switch (op)
    {
        case 1:
        {
            return destColor * srcColor;
        }
        case 2:
        {
            float3 param = destColor;
            float3 param_1 = srcColor;
            return compositeScreen(param, param_1);
        }
        case 3:
        {
            float3 param_2 = srcColor;
            float3 param_3 = destColor;
            return compositeHardLight(param_2, param_3);
        }
        case 4:
        {
            return fast::min(destColor, srcColor);
        }
        case 5:
        {
            return fast::max(destColor, srcColor);
        }
        case 6:
        {
            float3 param_4 = destColor;
            float3 param_5 = srcColor;
            return compositeColorDodge(param_4, param_5);
        }
        case 7:
        {
            float3 param_6 = float3(1.0) - destColor;
            float3 param_7 = float3(1.0) - srcColor;
            return float3(1.0) - compositeColorDodge(param_6, param_7);
        }
        case 8:
        {
            float3 param_8 = destColor;
            float3 param_9 = srcColor;
            return compositeHardLight(param_8, param_9);
        }
        case 9:
        {
            float3 param_10 = destColor;
            float3 param_11 = srcColor;
            return compositeSoftLight(param_10, param_11);
        }
        case 10:
        {
            return abs(destColor - srcColor);
        }
        case 11:
        {
            return (destColor + srcColor) - ((float3(2.0) * destColor) * srcColor);
        }
        case 12:
        case 13:
        case 14:
        case 15:
        {
            float3 param_12 = destColor;
            float3 param_13 = srcColor;
            float3 param_14 = compositeRGBToHSL(param_12);
            float3 param_15 = compositeRGBToHSL(param_13);
            int param_16 = op;
            float3 param_17 = compositeHSL(param_14, param_15, param_16);
            return compositeHSLToRGB(param_17);
        }
    }
    return srcColor;
}

float4 composite(thread const float4& srcColor, thread const texture2d<float> destTexture, thread const sampler destTextureSmplr, thread const float2& destTextureSize, thread const float2& fragCoord, thread const int& op)
{
    if (op == 0)
    {
        return srcColor;
    }
    float2 destTexCoord = fragCoord / destTextureSize;
    float4 destColor = destTexture.sample(destTextureSmplr, destTexCoord);
    float3 param = destColor.xyz;
    float3 param_1 = srcColor.xyz;
    int param_2 = op;
    float3 blendedRGB = compositeRGB(param, param_1, param_2);
    return float4(((srcColor.xyz * (srcColor.w * (1.0 - destColor.w))) + (blendedRGB * (srcColor.w * destColor.w))) + (destColor.xyz * (1.0 - srcColor.w)), 1.0);
}

void calculateColor(thread const int& ctrl, thread texture2d<float> uMaskTexture0, thread const sampler uMaskTexture0Smplr, thread float3& vMaskTexCoord0, thread texture2d<float> uMaskTexture1, thread const sampler uMaskTexture1Smplr, thread float3& vMaskTexCoord1, thread float2& vColorTexCoord0, thread texture2d<float> uColorTexture0, thread const sampler uColorTexture0Smplr, thread texture2d<float> uGammaLUT, thread const sampler uGammaLUTSmplr, thread float2 uColorTexture0Size, thread float4 uFilterParams0, thread float4 uFilterParams1, thread float4 uFilterParams2, thread texture2d<float> uColorTexture1, thread const sampler uColorTexture1Smplr, thread float2& vColorTexCoord1, thread texture2d<float> uDestTexture, thread const sampler uDestTextureSmplr, thread float2 uDestTextureSize, thread float4& gl_FragCoord, thread float4& oFragColor)
{
    int maskCtrl0 = (ctrl >> 0) & 3;
    int maskCtrl1 = (ctrl >> 2) & 3;
    float maskAlpha = 1.0;
    float param = maskAlpha;
    float3 param_1 = vMaskTexCoord0;
    int param_2 = maskCtrl0;
    maskAlpha = sampleMask(param, uMaskTexture0, uMaskTexture0Smplr, param_1, param_2);
    float param_3 = maskAlpha;
    float3 param_4 = vMaskTexCoord1;
    int param_5 = maskCtrl1;
    maskAlpha = sampleMask(param_3, uMaskTexture1, uMaskTexture1Smplr, param_4, param_5);
    float4 color = float4(0.0);
    if (((ctrl >> 6) & 1) != 0)
    {
        int color0Filter = (ctrl >> 4) & 3;
        float2 param_6 = vColorTexCoord0;
        float2 param_7 = uColorTexture0Size;
        float4 param_8 = uFilterParams0;
        float4 param_9 = uFilterParams1;
        float4 param_10 = uFilterParams2;
        int param_11 = color0Filter;
        color += filterColor(param_6, uColorTexture0, uColorTexture0Smplr, uGammaLUT, uGammaLUTSmplr, param_7, param_8, param_9, param_10, param_11);
    }
    if (((ctrl >> 7) & 1) != 0)
    {
        float2 param_12 = vColorTexCoord1;
        color *= sampleColor(uColorTexture1, uColorTexture1Smplr, param_12);
    }
    color.w *= maskAlpha;
    int compositeOp = (ctrl >> 8) & 15;
    float4 param_13 = color;
    float2 param_14 = uDestTextureSize;
    float2 param_15 = gl_FragCoord.xy;
    int param_16 = compositeOp;
    color = composite(param_13, uDestTexture, uDestTextureSmplr, param_14, param_15, param_16);
    float3 _1159 = color.xyz * color.w;
    color = float4(_1159.x, _1159.y, _1159.z, color.w);
    oFragColor = color;
}

fragment main0_out main0(main0_in in [[stage_in]], constant spvDescriptorSetBuffer0& spvDescriptorSet0 [[buffer(0)]], float4 gl_FragCoord [[position]])
{
    main0_out out = {};
    int param = (*spvDescriptorSet0.uCtrl);
    calculateColor(param, spvDescriptorSet0.uMaskTexture0, spvDescriptorSet0.uMaskTexture0Smplr, in.vMaskTexCoord0, spvDescriptorSet0.uMaskTexture1, spvDescriptorSet0.uMaskTexture1Smplr, in.vMaskTexCoord1, in.vColorTexCoord0, spvDescriptorSet0.uColorTexture0, spvDescriptorSet0.uColorTexture0Smplr, spvDescriptorSet0.uGammaLUT, spvDescriptorSet0.uGammaLUTSmplr, (*spvDescriptorSet0.uColorTexture0Size), (*spvDescriptorSet0.uFilterParams0), (*spvDescriptorSet0.uFilterParams1), (*spvDescriptorSet0.uFilterParams2), spvDescriptorSet0.uColorTexture1, spvDescriptorSet0.uColorTexture1Smplr, in.vColorTexCoord1, spvDescriptorSet0.uDestTexture, spvDescriptorSet0.uDestTextureSmplr, (*spvDescriptorSet0.uDestTextureSize), gl_FragCoord, out.oFragColor);
    return out;
}

