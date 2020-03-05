// Automatically generated from files in pathfinder/shaders/. Do not edit!
#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct spvDescriptorSetBuffer0
{
    constant float3* uInitialGaussCoeff [[id(0)]];
    texture2d<float> uColorTexture [[id(1)]];
    sampler uColorTextureSmplr [[id(2)]];
    constant int* uSupport [[id(3)]];
    constant float2* uSrcOffsetScale [[id(4)]];
};

struct main0_out
{
    float4 oFragColor [[color(0)]];
};

struct main0_in
{
    float2 vColorTexCoord [[user(locn0)]];
};

fragment main0_out main0(main0_in in [[stage_in]], constant spvDescriptorSetBuffer0& spvDescriptorSet0 [[buffer(0)]])
{
    main0_out out = {};
    float3 gaussCoeff = (*spvDescriptorSet0.uInitialGaussCoeff);
    float gaussSum = gaussCoeff.x;
    float4 color = spvDescriptorSet0.uColorTexture.sample(spvDescriptorSet0.uColorTextureSmplr, in.vColorTexCoord) * gaussCoeff.x;
    float2 _39 = gaussCoeff.xy * gaussCoeff.yz;
    gaussCoeff = float3(_39.x, _39.y, gaussCoeff.z);
    for (int i = 1; i <= (*spvDescriptorSet0.uSupport); i += 2)
    {
        float gaussPartialSum = gaussCoeff.x;
        float2 _64 = gaussCoeff.xy * gaussCoeff.yz;
        gaussCoeff = float3(_64.x, _64.y, gaussCoeff.z);
        gaussPartialSum += gaussCoeff.x;
        float2 srcOffset = (*spvDescriptorSet0.uSrcOffsetScale) * (float(i) + (gaussCoeff.x / gaussPartialSum));
        color += ((spvDescriptorSet0.uColorTexture.sample(spvDescriptorSet0.uColorTextureSmplr, (in.vColorTexCoord - srcOffset)) + spvDescriptorSet0.uColorTexture.sample(spvDescriptorSet0.uColorTextureSmplr, (in.vColorTexCoord + srcOffset))) * gaussPartialSum);
        gaussSum += (2.0 * gaussPartialSum);
        float2 _108 = gaussCoeff.xy * gaussCoeff.yz;
        gaussCoeff = float3(_108.x, _108.y, gaussCoeff.z);
    }
    color /= float4(gaussSum);
    float3 _123 = color.xyz * color.w;
    color = float4(_123.x, _123.y, _123.z, color.w);
    out.oFragColor = color;
    return out;
}

