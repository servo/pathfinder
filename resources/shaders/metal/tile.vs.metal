// Automatically generated from files in pathfinder/shaders/. Do not edit!
#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct spvDescriptorSetBuffer0
{
    constant float2* uTileSize [[id(0)]];
    constant float4x4* uTransform [[id(1)]];
};

struct main0_out
{
    float3 vMaskTexCoord0 [[user(locn0)]];
    float3 vMaskTexCoord1 [[user(locn1)]];
    float2 vColorTexCoord0 [[user(locn2)]];
    float2 vColorTexCoord1 [[user(locn3)]];
    float4 gl_Position [[position]];
};

struct main0_in
{
    int2 aTilePosition [[attribute(0)]];
    float2 aColorTexCoord0 [[attribute(1)]];
    float2 aColorTexCoord1 [[attribute(2)]];
    float2 aMaskTexCoord0 [[attribute(3)]];
    float2 aMaskTexCoord1 [[attribute(4)]];
    int2 aMaskBackdrop [[attribute(5)]];
};

vertex main0_out main0(main0_in in [[stage_in]], constant spvDescriptorSetBuffer0& spvDescriptorSet0 [[buffer(0)]])
{
    main0_out out = {};
    float2 position = float2(in.aTilePosition) * (*spvDescriptorSet0.uTileSize);
    out.vColorTexCoord0 = in.aColorTexCoord0;
    out.vColorTexCoord1 = in.aColorTexCoord1;
    out.vMaskTexCoord0 = float3(in.aMaskTexCoord0, float(in.aMaskBackdrop.x));
    out.vMaskTexCoord1 = float3(in.aMaskTexCoord1, float(in.aMaskBackdrop.y));
    out.gl_Position = (*spvDescriptorSet0.uTransform) * float4(position, 0.0, 1.0);
    return out;
}

