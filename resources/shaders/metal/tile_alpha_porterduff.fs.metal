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
    constant int* uDestFactor [[id(7)]];
    constant int* uSrcFactor [[id(8)]];
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

float4 getFactor(thread const int& factor, thread const float4& destRGBA, thread const float4& srcRGBA)
{
    if (factor == 0)
    {
        return float4(0.0);
    }
    if (factor == 1)
    {
        return float4(destRGBA.w);
    }
    if (factor == 2)
    {
        return float4(srcRGBA.w);
    }
    return float4(1.0 - destRGBA.w);
}

fragment main0_out main0(main0_in in [[stage_in]], constant spvDescriptorSetBuffer0& spvDescriptorSet0 [[buffer(0)]], float4 gl_FragCoord [[position]])
{
    main0_out out = {};
    float4 srcRGBA = sampleSrcColor(spvDescriptorSet0.uStencilTexture, spvDescriptorSet0.uStencilTextureSmplr, in.vMaskTexCoord, spvDescriptorSet0.uPaintTexture, spvDescriptorSet0.uPaintTextureSmplr, in.vColorTexCoord);
    float4 destRGBA = sampleDestColor(gl_FragCoord, (*spvDescriptorSet0.uFramebufferSize), spvDescriptorSet0.uDest, spvDescriptorSet0.uDestSmplr);
    int param = (*spvDescriptorSet0.uDestFactor);
    float4 param_1 = destRGBA;
    float4 param_2 = srcRGBA;
    float4 destFactor = getFactor(param, param_1, param_2);
    int param_3 = (*spvDescriptorSet0.uSrcFactor);
    float4 param_4 = destRGBA;
    float4 param_5 = srcRGBA;
    float4 srcFactor = getFactor(param_3, param_4, param_5);
    float4 blended = ((destFactor * destRGBA) * float4(destRGBA.www, 1.0)) + ((srcFactor * srcRGBA) * float4(srcRGBA.www, 1.0));
    out.oFragColor = blended;
    return out;
}

