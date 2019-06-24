// Automatically generated from files in pathfinder/shaders/. Do not edit!
#pragma clang diagnostic ignored "-Wmissing-prototypes"

#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct spvDescriptorSetBuffer0
{
    constant float2* uTileSize [[id(0)]];
    constant float2* uViewBoxOrigin [[id(1)]];
    constant float2* uFramebufferSize [[id(2)]];
    texture2d<float> uPaintTexture [[id(3)]];
    sampler uPaintTextureSmplr [[id(4)]];
};

struct main0_out
{
    float4 vColor [[user(locn0)]];
    float4 gl_Position [[position]];
};

struct main0_in
{
    uint2 aTessCoord [[attribute(0)]];
    int2 aTileOrigin [[attribute(1)]];
    float2 aColorTexCoord [[attribute(2)]];
};

float4 getColor(thread texture2d<float> uPaintTexture, thread const sampler uPaintTextureSmplr, thread float2& aColorTexCoord)
{
    return uPaintTexture.sample(uPaintTextureSmplr, aColorTexCoord, level(0.0));
}

void computeVaryings(thread int2& aTileOrigin, thread uint2& aTessCoord, thread float2 uTileSize, thread float2 uViewBoxOrigin, thread float2 uFramebufferSize, thread float4& vColor, thread float4& gl_Position, thread texture2d<float> uPaintTexture, thread const sampler uPaintTextureSmplr, thread float2& aColorTexCoord)
{
    float2 pixelPosition = (float2(aTileOrigin + int2(aTessCoord)) * uTileSize) + uViewBoxOrigin;
    float2 position = (((pixelPosition / uFramebufferSize) * 2.0) - float2(1.0)) * float2(1.0, -1.0);
    vColor = getColor(uPaintTexture, uPaintTextureSmplr, aColorTexCoord);
    gl_Position = float4(position, 0.0, 1.0);
}

vertex main0_out main0(main0_in in [[stage_in]], constant spvDescriptorSetBuffer0& spvDescriptorSet0 [[buffer(0)]])
{
    main0_out out = {};
    computeVaryings(in.aTileOrigin, in.aTessCoord, (*spvDescriptorSet0.uTileSize), (*spvDescriptorSet0.uViewBoxOrigin), (*spvDescriptorSet0.uFramebufferSize), out.vColor, out.gl_Position, spvDescriptorSet0.uPaintTexture, spvDescriptorSet0.uPaintTextureSmplr, in.aColorTexCoord);
    return out;
}

