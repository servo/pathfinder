// Automatically generated from files in pathfinder/shaders/. Do not edit!
#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct spvDescriptorSetBuffer0
{
    texture2d<float> uPaintTexture [[id(0)]];
    sampler uPaintTextureSmplr [[id(1)]];
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
    out.oFragColor = spvDescriptorSet0.uPaintTexture.sample(spvDescriptorSet0.uPaintTextureSmplr, in.vColorTexCoord);
    return out;
}

