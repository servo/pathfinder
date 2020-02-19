// Automatically generated from files in pathfinder/shaders/. Do not edit!
#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct spvDescriptorSetBuffer0
{
    texture2d<float> uFillTexture [[id(0)]];
    sampler uFillTextureSmplr [[id(1)]];
};

struct main0_out
{
    float4 oFragColor [[color(0)]];
};

struct main0_in
{
    float2 vFillTexCoord [[user(locn0)]];
    float vBackdrop [[user(locn1)]];
};

fragment main0_out main0(main0_in in [[stage_in]], constant spvDescriptorSetBuffer0& spvDescriptorSet0 [[buffer(0)]])
{
    main0_out out = {};
    out.oFragColor = float4(abs(spvDescriptorSet0.uFillTexture.sample(spvDescriptorSet0.uFillTextureSmplr, in.vFillTexCoord).x + in.vBackdrop));
    return out;
}

