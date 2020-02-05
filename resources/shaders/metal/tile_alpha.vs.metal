// Automatically generated from files in pathfinder/shaders/. Do not edit!
#pragma clang diagnostic ignored "-Wmissing-prototypes"

#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct spvDescriptorSetBuffer0
{
    constant float2* uTileSize [[id(0)]];
    constant float2* uStencilTextureSize [[id(1)]];
    constant float4x4* uTransform [[id(2)]];
};

struct main0_out
{
    float2 vMaskTexCoord [[user(locn0)]];
    float2 vColorTexCoord [[user(locn1)]];
    float vBackdrop [[user(locn2)]];
    float4 gl_Position [[position]];
};

struct main0_in
{
    uint2 aTessCoord [[attribute(0)]];
    uint3 aTileOrigin [[attribute(1)]];
    float2 aColorTexCoord [[attribute(2)]];
    int aBackdrop [[attribute(3)]];
    int aTileIndex [[attribute(4)]];
};

float2 computeTileOffset(thread const uint& tileIndex, thread const float& stencilTextureWidth, thread float2 uTileSize)
{
    uint tilesPerRow = uint(stencilTextureWidth / uTileSize.x);
    uint2 tileOffset = uint2(tileIndex % tilesPerRow, tileIndex / tilesPerRow);
    return float2(tileOffset) * uTileSize;
}

vertex main0_out main0(main0_in in [[stage_in]], constant spvDescriptorSetBuffer0& spvDescriptorSet0 [[buffer(0)]])
{
    main0_out out = {};
    float2 origin = float2(in.aTileOrigin.xy) + (float2(float(in.aTileOrigin.z & 15u), float(in.aTileOrigin.z >> 4u)) * 256.0);
    float2 position = (origin + float2(in.aTessCoord)) * (*spvDescriptorSet0.uTileSize);
    uint param = uint(in.aTileIndex);
    float param_1 = (*spvDescriptorSet0.uStencilTextureSize).x;
    float2 maskTexCoordOrigin = computeTileOffset(param, param_1, (*spvDescriptorSet0.uTileSize));
    float2 maskTexCoord = maskTexCoordOrigin + (float2(in.aTessCoord) * (*spvDescriptorSet0.uTileSize));
    out.vMaskTexCoord = maskTexCoord / (*spvDescriptorSet0.uStencilTextureSize);
    out.vColorTexCoord = in.aColorTexCoord;
    out.vBackdrop = float(in.aBackdrop);
    out.gl_Position = (*spvDescriptorSet0.uTransform) * float4(position, 0.0, 1.0);
    return out;
}

