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
};

struct main0_out
{
    float4 oFragColor [[color(0)]];
};

struct main0_in
{
    float2 vColorTexCoord [[user(locn0)]];
    float2 vMaskTexCoord [[user(locn1)]];
    float vOpacity [[user(locn2)]];
};

float4 sampleSrcColor(thread texture2d<float> uStencilTexture, thread const sampler uStencilTextureSmplr, thread float2& vMaskTexCoord, thread texture2d<float> uPaintTexture, thread const sampler uPaintTextureSmplr, thread float2& vColorTexCoord, thread float& vOpacity)
{
    float coverage = uStencilTexture.sample(uStencilTextureSmplr, vMaskTexCoord).x;
    float4 srcRGBA = uPaintTexture.sample(uPaintTextureSmplr, vColorTexCoord);
    return float4(srcRGBA.xyz, (srcRGBA.w * coverage) * vOpacity);
}

float4 sampleDestColor(thread float4& gl_FragCoord, thread float2 uFramebufferSize, thread texture2d<float> uDest, thread const sampler uDestSmplr)
{
    float2 destTexCoord = gl_FragCoord.xy / uFramebufferSize;
    return uDest.sample(uDestSmplr, destTexCoord);
}

float3 select3(thread const bool3& cond, thread const float3& a, thread const float3& b)
{
    float _122;
    if (cond.x)
    {
        _122 = a.x;
    }
    else
    {
        _122 = b.x;
    }
    float _134;
    if (cond.y)
    {
        _134 = a.y;
    }
    else
    {
        _134 = b.y;
    }
    float _146;
    if (cond.z)
    {
        _146 = a.z;
    }
    else
    {
        _146 = b.z;
    }
    return float3(_122, _134, _146);
}

float4 blendColors(thread const float4& destRGBA, thread const float4& srcRGBA, thread const float3& blendedRGB)
{
    return float4(((srcRGBA.xyz * (srcRGBA.w * (1.0 - destRGBA.w))) + (blendedRGB * (srcRGBA.w * destRGBA.w))) + (destRGBA.xyz * ((1.0 - srcRGBA.w) * destRGBA.w)), 1.0);
}

fragment main0_out main0(main0_in in [[stage_in]], constant spvDescriptorSetBuffer0& spvDescriptorSet0 [[buffer(0)]], float4 gl_FragCoord [[position]])
{
    main0_out out = {};
    float4 srcRGBA = sampleSrcColor(spvDescriptorSet0.uStencilTexture, spvDescriptorSet0.uStencilTextureSmplr, in.vMaskTexCoord, spvDescriptorSet0.uPaintTexture, spvDescriptorSet0.uPaintTextureSmplr, in.vColorTexCoord, in.vOpacity);
    float4 destRGBA = sampleDestColor(gl_FragCoord, (*spvDescriptorSet0.uFramebufferSize), spvDescriptorSet0.uDest, spvDescriptorSet0.uDestSmplr);
    float3 dest = destRGBA.xyz;
    float3 src = srcRGBA.xyz;
    bool3 destDark = dest <= float3(0.25);
    bool3 srcDark = src <= float3(0.5);
    bool3 param = destDark;
    float3 param_1 = (((dest * 16.0) - float3(12.0)) * dest) + float3(4.0);
    float3 param_2 = rsqrt(dest);
    float3 d = select3(param, param_1, param_2);
    bool3 param_3 = srcDark;
    float3 param_4 = float3(1.0) - dest;
    float3 param_5 = d - float3(1.0);
    float3 x = select3(param_3, param_4, param_5);
    float3 blended = dest * ((((src * 2.0) - float3(1.0)) * x) + float3(1.0));
    float4 param_6 = destRGBA;
    float4 param_7 = srcRGBA;
    float3 param_8 = blended;
    out.oFragColor = blendColors(param_6, param_7, param_8);
    return out;
}

