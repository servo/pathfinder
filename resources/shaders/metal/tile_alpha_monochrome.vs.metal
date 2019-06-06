#pragma clang diagnostic ignored "-Wmissing-prototypes"

#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct main0_out
{
    float2 vTexCoord [[user(locn0)]];
    float vBackdrop [[user(locn1)]];
    float4 vColor [[user(locn2)]];
    float4 gl_Position [[position]];
};

struct main0_in
{
    float2 aTessCoord [[attribute(0)]];
    uint3 aTileOrigin [[attribute(1)]];
    int aBackdrop [[attribute(2)]];
    uint aTileIndex [[attribute(3)]];
};

float2 computeTileOffset(thread const uint& tileIndex, thread const float& stencilTextureWidth, thread float2 uTileSize)
{
    uint tilesPerRow = uint(stencilTextureWidth / uTileSize.x);
    uint2 tileOffset = uint2(tileIndex % tilesPerRow, tileIndex / tilesPerRow);
    return float2(tileOffset) * uTileSize;
}

float4 getColor(thread float4 uColor)
{
    return uColor;
}

void computeVaryings(thread float2 uTileSize, thread uint3& aTileOrigin, thread float2& aTessCoord, thread float2 uViewBoxOrigin, thread float2 uFramebufferSize, thread uint& aTileIndex, thread float2 uStencilTextureSize, thread float2& vTexCoord, thread float& vBackdrop, thread int& aBackdrop, thread float4& vColor, thread float4& gl_Position, thread float4 uColor)
{
    float2 origin = float2(aTileOrigin.xy) + (float2(float(aTileOrigin.z & 15u), float(aTileOrigin.z >> 4u)) * 256.0);
    float2 pixelPosition = ((origin + aTessCoord) * uTileSize) + uViewBoxOrigin;
    float2 position = (((pixelPosition / uFramebufferSize) * 2.0) - float2(1.0)) * float2(1.0, -1.0);
    uint param = aTileIndex;
    float param_1 = uStencilTextureSize.x;
    float2 maskTexCoordOrigin = computeTileOffset(param, param_1, uTileSize);
    float2 maskTexCoord = maskTexCoordOrigin + (aTessCoord * uTileSize);
    vTexCoord = maskTexCoord / uStencilTextureSize;
    vBackdrop = float(aBackdrop);
    vColor = getColor(uColor);
    gl_Position = float4(position, 0.0, 1.0);
}

vertex main0_out main0(main0_in in [[stage_in]], float2 uTileSize [[buffer(0)]], float2 uViewBoxOrigin [[buffer(1)]], float2 uFramebufferSize [[buffer(2)]], float2 uStencilTextureSize [[buffer(3)]], float4 uColor [[buffer(4)]], uint gl_VertexID [[vertex_id]], uint gl_InstanceID [[instance_id]])
{
    main0_out out = {};
    computeVaryings(uTileSize, in.aTileOrigin, in.aTessCoord, uViewBoxOrigin, uFramebufferSize, in.aTileIndex, uStencilTextureSize, out.vTexCoord, out.vBackdrop, in.aBackdrop, out.vColor, out.gl_Position, uColor);
    return out;
}

