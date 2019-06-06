#pragma clang diagnostic ignored "-Wmissing-prototypes"

#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct main0_out
{
    float4 oFragColor [[color(0)]];
};

struct main0_in
{
    float2 vTexCoord [[user(locn0)]];
};

float sample1Tap(thread const float& offset, thread texture2d<float> uSource, thread const sampler uSourceSmplr, thread float2& vTexCoord)
{
    return uSource.sample(uSourceSmplr, float2(vTexCoord.x + offset, vTexCoord.y)).x;
}

void sample9Tap(thread float4& outAlphaLeft, thread float& outAlphaCenter, thread float4& outAlphaRight, thread const float& onePixel, thread float4 uKernel, thread texture2d<float> uSource, thread const sampler uSourceSmplr, thread float2& vTexCoord)
{
    float _89;
    if (uKernel.x > 0.0)
    {
        float param = (-4.0) * onePixel;
        _89 = sample1Tap(param, uSource, uSourceSmplr, vTexCoord);
    }
    else
    {
        _89 = 0.0;
    }
    float param_1 = (-3.0) * onePixel;
    float param_2 = (-2.0) * onePixel;
    float param_3 = (-1.0) * onePixel;
    outAlphaLeft = float4(_89, sample1Tap(param_1, uSource, uSourceSmplr, vTexCoord), sample1Tap(param_2, uSource, uSourceSmplr, vTexCoord), sample1Tap(param_3, uSource, uSourceSmplr, vTexCoord));
    float param_4 = 0.0;
    outAlphaCenter = sample1Tap(param_4, uSource, uSourceSmplr, vTexCoord);
    float param_5 = 1.0 * onePixel;
    float _120 = sample1Tap(param_5, uSource, uSourceSmplr, vTexCoord);
    float param_6 = 2.0 * onePixel;
    float _125 = sample1Tap(param_6, uSource, uSourceSmplr, vTexCoord);
    float param_7 = 3.0 * onePixel;
    float _130 = sample1Tap(param_7, uSource, uSourceSmplr, vTexCoord);
    float _134;
    if (uKernel.x > 0.0)
    {
        float param_8 = 4.0 * onePixel;
        _134 = sample1Tap(param_8, uSource, uSourceSmplr, vTexCoord);
    }
    else
    {
        _134 = 0.0;
    }
    outAlphaRight = float4(_120, _125, _130, _134);
}

float convolve7Tap(thread const float4& alpha0, thread const float3& alpha1, thread float4 uKernel)
{
    return dot(alpha0, uKernel) + dot(alpha1, uKernel.zyx);
}

float gammaCorrectChannel(thread const float& bgColor, thread const float& fgColor, thread texture2d<float> uGammaLUT, thread const sampler uGammaLUTSmplr)
{
    return uGammaLUT.sample(uGammaLUTSmplr, float2(fgColor, 1.0 - bgColor)).x;
}

float3 gammaCorrect(thread const float3& bgColor, thread const float3& fgColor, thread texture2d<float> uGammaLUT, thread const sampler uGammaLUTSmplr)
{
    float param = bgColor.x;
    float param_1 = fgColor.x;
    float param_2 = bgColor.y;
    float param_3 = fgColor.y;
    float param_4 = bgColor.z;
    float param_5 = fgColor.z;
    return float3(gammaCorrectChannel(param, param_1, uGammaLUT, uGammaLUTSmplr), gammaCorrectChannel(param_2, param_3, uGammaLUT, uGammaLUTSmplr), gammaCorrectChannel(param_4, param_5, uGammaLUT, uGammaLUTSmplr));
}

fragment main0_out main0(main0_in in [[stage_in]], int uGammaCorrectionEnabled [[buffer(2)]], float4 uKernel [[buffer(0)]], float2 uSourceSize [[buffer(1)]], float4 uBGColor [[buffer(3)]], float4 uFGColor [[buffer(4)]], texture2d<float> uGammaLUT [[texture(0)]], texture2d<float> uSource [[texture(0)]], sampler uGammaLUTSmplr [[sampler(0)]], sampler uSourceSmplr [[sampler(0)]])
{
    main0_out out = {};
    float3 alpha;
    if (uKernel.w == 0.0)
    {
        alpha = uSource.sample(uSourceSmplr, in.vTexCoord).xxx;
    }
    else
    {
        float param_3 = 1.0 / uSourceSize.x;
        float4 param;
        float param_1;
        float4 param_2;
        sample9Tap(param, param_1, param_2, param_3, uKernel, uSource, uSourceSmplr, in.vTexCoord);
        float4 alphaLeft = param;
        float alphaCenter = param_1;
        float4 alphaRight = param_2;
        float4 param_4 = alphaLeft;
        float3 param_5 = float3(alphaCenter, alphaRight.xy);
        float r = convolve7Tap(param_4, param_5, uKernel);
        float4 param_6 = float4(alphaLeft.yzw, alphaCenter);
        float3 param_7 = alphaRight.xyz;
        float g = convolve7Tap(param_6, param_7, uKernel);
        float4 param_8 = float4(alphaLeft.zw, alphaCenter, alphaRight.x);
        float3 param_9 = alphaRight.yzw;
        float b = convolve7Tap(param_8, param_9, uKernel);
        alpha = float3(r, g, b);
    }
    if (uGammaCorrectionEnabled != 0)
    {
        float3 param_10 = uBGColor.xyz;
        float3 param_11 = alpha;
        alpha = gammaCorrect(param_10, param_11, uGammaLUT, uGammaLUTSmplr);
    }
    out.oFragColor = float4(mix(uBGColor.xyz, uFGColor.xyz, alpha), 1.0);
    return out;
}

