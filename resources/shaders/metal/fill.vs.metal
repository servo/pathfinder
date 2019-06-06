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
    float2 aTessCoord [[attribute(0)]];
    uint aFromPx [[attribute(1)]];
    uint aToPx [[attribute(2)]];
    float2 aFromSubpx [[attribute(3)]];
    float2 aToSubpx [[attribute(4)]];
    uint aTileIndex [[attribute(5)]];
};

float2 computeTileOffset(thread const uint& tileIndex, thread const float& stencilTextureWidth, thread float2 uTileSize, thread uint& aTileIndex)
{
    uint tilesPerRow = uint(stencilTextureWidth / uTileSize.x);
    uint2 tileOffset = uint2(aTileIndex % tilesPerRow, aTileIndex / tilesPerRow);
    return float2(tileOffset) * uTileSize;
}

vertex main0_out main0(main0_in in [[stage_in]], float2 uTileSize [[buffer(0)]], float2 uFramebufferSize [[buffer(1)]], uint gl_VertexID [[vertex_id]], uint gl_InstanceID [[instance_id]])
{
    main0_out out = {};
    uint param = in.aTileIndex;
    float param_1 = uFramebufferSize.x;
    float2 tileOrigin = computeTileOffset(param, param_1, uTileSize, in.aTileIndex);
    float2 from = float2(float(in.aFromPx & 15u), float(in.aFromPx >> 4u)) + in.aFromSubpx;
    float2 to = float2(float(in.aToPx & 15u), float(in.aToPx >> 4u)) + in.aToSubpx;
    float2 position;
    if (in.aTessCoord.x < 0.5)
    {
        position.x = floor(fast::min(from.x, to.x));
    }
    else
    {
        position.x = ceil(fast::max(from.x, to.x));
    }
    if (in.aTessCoord.y < 0.5)
    {
        position.y = floor(fast::min(from.y, to.y));
    }
    else
    {
        position.y = uTileSize.y;
    }
    out.vFrom = from - position;
    out.vTo = to - position;
    out.gl_Position = float4((((tileOrigin + position) / uFramebufferSize) * 2.0) - float2(1.0), 0.0, 1.0);
    return out;
}

