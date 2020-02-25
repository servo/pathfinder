// Automatically generated from files in pathfinder/shaders/. Do not edit!
#pragma clang diagnostic ignored "-Wmissing-prototypes"

#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct spvDescriptorSetBuffer0
{
    texture2d<float> uStencilTexture [[id(0)]];
    sampler uStencilTextureSmplr [[id(1)]];
    texture2d<float> uPaintTexture [[id(2)]];
    sampler uPaintTextureSmplr [[id(3)]];
    constant float2* uFramebufferSize [[id(4)]];
    texture2d<float> uDest [[id(5)]];
    sampler uDestSmplr [[id(6)]];
    constant int* uBlendMode [[id(7)]];
};

struct main0_out
{
    float4 oFragColor [[color(0)]];
};

struct main0_in
{
    float2 vColorTexCoord [[user(locn0)]];
    float2 vMaskTexCoord [[user(locn1)]];
};

float4 sampleSrcColor(thread texture2d<float> uStencilTexture, thread const sampler uStencilTextureSmplr, thread float2& vMaskTexCoord, thread texture2d<float> uPaintTexture, thread const sampler uPaintTextureSmplr, thread float2& vColorTexCoord)
{
    float coverage = uStencilTexture.sample(uStencilTextureSmplr, vMaskTexCoord).x;
    float4 srcRGBA = uPaintTexture.sample(uPaintTextureSmplr, vColorTexCoord);
    return float4(srcRGBA.xyz, srcRGBA.w * coverage);
}

float4 sampleDestColor(thread float4& gl_FragCoord, thread float2 uFramebufferSize, thread texture2d<float> uDest, thread const sampler uDestSmplr)
{
    float2 destTexCoord = gl_FragCoord.xy / uFramebufferSize;
    return uDest.sample(uDestSmplr, destTexCoord);
}

float3 select3(thread const bool3& cond, thread const float3& a, thread const float3& b)
{
    float _118;
    if (cond.x)
    {
        _118 = a.x;
    }
    else
    {
        _118 = b.x;
    }
    float _130;
    if (cond.y)
    {
        _130 = a.y;
    }
    else
    {
        _130 = b.y;
    }
    float _142;
    if (cond.z)
    {
        _142 = a.z;
    }
    else
    {
        _142 = b.z;
    }
    return float3(_118, _130, _142);
}

float4 blendColors(thread const float4& destRGBA, thread const float4& srcRGBA, thread const float3& blendedRGB)
{
    return float4(((srcRGBA.xyz * (srcRGBA.w * (1.0 - destRGBA.w))) + (blendedRGB * (srcRGBA.w * destRGBA.w))) + (destRGBA.xyz * ((1.0 - srcRGBA.w) * destRGBA.w)), 1.0);
}

fragment main0_out main0(main0_in in [[stage_in]], constant spvDescriptorSetBuffer0& spvDescriptorSet0 [[buffer(0)]], float4 gl_FragCoord [[position]])
{
    main0_out out = {};
    float4 srcRGBA = sampleSrcColor(spvDescriptorSet0.uStencilTexture, spvDescriptorSet0.uStencilTextureSmplr, in.vMaskTexCoord, spvDescriptorSet0.uPaintTexture, spvDescriptorSet0.uPaintTextureSmplr, in.vColorTexCoord);
    float4 destRGBA = sampleDestColor(gl_FragCoord, (*spvDescriptorSet0.uFramebufferSize), spvDescriptorSet0.uDest, spvDescriptorSet0.uDestSmplr);
    bool reversed = (*spvDescriptorSet0.uBlendMode) == 3;
    float3 _167;
    if (reversed)
    {
        _167 = srcRGBA.xyz;
    }
    else
    {
        _167 = destRGBA.xyz;
    }
    float3 src = _167;
    float3 _178;
    if (reversed)
    {
        _178 = destRGBA.xyz;
    }
    else
    {
        _178 = srcRGBA.xyz;
    }
    float3 dest = _178;
    float3 multiply = src * dest;
    float3 blended;
    if ((*spvDescriptorSet0.uBlendMode) == 0)
    {
        blended = multiply;
    }
    else
    {
        float3 screen = (dest + src) - multiply;
        if ((*spvDescriptorSet0.uBlendMode) == 1)
        {
            blended = screen;
        }
        else
        {
            bool3 param = src <= float3(0.5);
            float3 param_1 = multiply;
            float3 param_2 = (screen * 2.0) - float3(1.0);
            blended = select3(param, param_1, param_2);
        }
    }
    float4 param_3 = destRGBA;
    float4 param_4 = srcRGBA;
    float3 param_5 = blended;
    out.oFragColor = blendColors(param_3, param_4, param_5);
    return out;
}

