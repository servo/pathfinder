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
    constant int* uBurn [[id(7)]];
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

float4 blendColors(thread const float4& destRGBA, thread const float4& srcRGBA, thread const float3& blendedRGB)
{
    return float4(((srcRGBA.xyz * (srcRGBA.w * (1.0 - destRGBA.w))) + (blendedRGB * (srcRGBA.w * destRGBA.w))) + (destRGBA.xyz * ((1.0 - srcRGBA.w) * destRGBA.w)), 1.0);
}

fragment main0_out main0(main0_in in [[stage_in]], constant spvDescriptorSetBuffer0& spvDescriptorSet0 [[buffer(0)]], float4 gl_FragCoord [[position]])
{
    main0_out out = {};
    float4 srcRGBA = sampleSrcColor(spvDescriptorSet0.uStencilTexture, spvDescriptorSet0.uStencilTextureSmplr, in.vMaskTexCoord, spvDescriptorSet0.uPaintTexture, spvDescriptorSet0.uPaintTextureSmplr, in.vColorTexCoord, in.vOpacity);
    float4 destRGBA = sampleDestColor(gl_FragCoord, (*spvDescriptorSet0.uFramebufferSize), spvDescriptorSet0.uDest, spvDescriptorSet0.uDestSmplr);
    float3 _122;
    if ((*spvDescriptorSet0.uBurn) == 0)
    {
        _122 = destRGBA.xyz;
    }
    else
    {
        _122 = float3(1.0) - destRGBA.xyz;
    }
    float3 dest = _122;
    float3 _136;
    if ((*spvDescriptorSet0.uBurn) == 0)
    {
        _136 = float3(1.0) - srcRGBA.xyz;
    }
    else
    {
        _136 = srcRGBA.xyz;
    }
    float3 src = _136;
    bool3 srcNonzero = src != float3(0.0);
    float _157;
    if (srcNonzero.x)
    {
        _157 = dest.x / src.x;
    }
    else
    {
        _157 = 1.0;
    }
    float _170;
    if (srcNonzero.y)
    {
        _170 = dest.y / src.y;
    }
    else
    {
        _170 = 1.0;
    }
    float _183;
    if (srcNonzero.z)
    {
        _183 = dest.z / src.z;
    }
    else
    {
        _183 = 1.0;
    }
    float3 blended = fast::min(float3(_157, _170, _183), float3(1.0));
    if ((*spvDescriptorSet0.uBurn) != 0)
    {
        blended = float3(1.0) - blended;
    }
    float4 param = destRGBA;
    float4 param_1 = srcRGBA;
    float3 param_2 = blended;
    out.oFragColor = blendColors(param, param_1, param_2);
    return out;
}

