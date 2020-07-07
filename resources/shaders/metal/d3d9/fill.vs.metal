// Automatically generated from files in pathfinder/shaders/. Do not edit!
#pragma clang diagnostic ignored "-Wmissing-prototypes"

#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct main0_out
{
    float2 vFrom [[user(locn0)]];
    float2 vTo [[user(locn1)]];
    float4 gl_Position [[position]];
};

struct main0_in
{
    uint2 aTessCoord [[attribute(0)]];
    uint4 aLineSegment [[attribute(1)]];
    int aTileIndex [[attribute(2)]];
};

static inline __attribute__((always_inline))
float2 computeTileOffset(thread const uint& tileIndex, thread const float& stencilTextureWidth, thread const float2& tileSize)
{
    uint tilesPerRow = uint(stencilTextureWidth / tileSize.x);
    uint2 tileOffset = uint2(tileIndex % tilesPerRow, tileIndex / tilesPerRow);
    return (float2(tileOffset) * tileSize) * float2(1.0, 0.25);
}

static inline __attribute__((always_inline))
float4 computeVertexPosition(thread const uint& tileIndex, thread const uint2& tessCoord, thread const uint4& packedLineSegment, thread const float2& tileSize, thread const float2& framebufferSize, thread float2& outFrom, thread float2& outTo)
{
    uint param = tileIndex;
    float param_1 = framebufferSize.x;
    float2 param_2 = tileSize;
    float2 tileOrigin = computeTileOffset(param, param_1, param_2);
    float4 lineSegment = float4(packedLineSegment) / float4(256.0);
    float2 from = lineSegment.xy;
    float2 to = lineSegment.zw;
    float2 position;
    if (tessCoord.x == 0u)
    {
        position.x = floor(fast::min(from.x, to.x));
    }
    else
    {
        position.x = ceil(fast::max(from.x, to.x));
    }
    if (tessCoord.y == 0u)
    {
        position.y = floor(fast::min(from.y, to.y));
    }
    else
    {
        position.y = tileSize.y;
    }
    position.y = floor(position.y * 0.25);
    float2 offset = float2(0.0, 1.5) - (position * float2(1.0, 4.0));
    outFrom = from + offset;
    outTo = to + offset;
    float2 globalPosition = (((tileOrigin + position) / framebufferSize) * 2.0) - float2(1.0);
    globalPosition.y = -globalPosition.y;
    return float4(globalPosition, 0.0, 1.0);
}

vertex main0_out main0(main0_in in [[stage_in]], constant float2& uTileSize [[buffer(0)]], constant float2& uFramebufferSize [[buffer(1)]])
{
    main0_out out = {};
    uint param = uint(in.aTileIndex);
    uint2 param_1 = in.aTessCoord;
    uint4 param_2 = in.aLineSegment;
    float2 param_3 = uTileSize;
    float2 param_4 = uFramebufferSize;
    float2 param_5;
    float2 param_6;
    float4 _190 = computeVertexPosition(param, param_1, param_2, param_3, param_4, param_5, param_6);
    out.vFrom = param_5;
    out.vTo = param_6;
    out.gl_Position = _190;
    return out;
}

