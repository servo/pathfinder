// Automatically generated from files in pathfinder/shaders/. Do not edit!
#pragma clang diagnostic ignored "-Wmissing-prototypes"

#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct bFills
{
    uint iFills[1];
};

struct bAlphaTiles
{
    uint iAlphaTiles[1];
};

struct bTiles
{
    uint iTiles[1];
};

constant uint3 gl_WorkGroupSize [[maybe_unused]] = uint3(16u, 4u, 1u);

static inline __attribute__((always_inline))
float4 computeCoverage(thread const float2& from, thread const float2& to, thread const texture2d<float> areaLUT, thread const sampler areaLUTSmplr)
{
    float2 left = select(to, from, bool2(from.x < to.x));
    float2 right = select(from, to, bool2(from.x < to.x));
    float2 window = fast::clamp(float2(from.x, to.x), float2(-0.5), float2(0.5));
    float offset = mix(window.x, window.y, 0.5) - left.x;
    float t = offset / (right.x - left.x);
    float y = mix(left.y, right.y, t);
    float d = (right.y - left.y) / (right.x - left.x);
    float dX = window.x - window.y;
    return areaLUT.sample(areaLUTSmplr, (float2(y + 8.0, abs(d * dX)) / float2(16.0)), level(0.0)) * dX;
}

static inline __attribute__((always_inline))
float4 accumulateCoverageForFillList(thread int& fillIndex, thread const int2& tileSubCoord, const device bFills& v_148, thread texture2d<float> uAreaLUT, thread const sampler uAreaLUTSmplr)
{
    float2 tileFragCoord = float2(tileSubCoord) + float2(0.5);
    float4 coverages = float4(0.0);
    int iteration = 0;
    do
    {
        uint fillFrom = v_148.iFills[(fillIndex * 3) + 0];
        uint fillTo = v_148.iFills[(fillIndex * 3) + 1];
        float4 lineSegment = float4(float(fillFrom & 65535u), float(fillFrom >> uint(16)), float(fillTo & 65535u), float(fillTo >> uint(16))) / float4(256.0);
        lineSegment -= tileFragCoord.xyxy;
        float2 param = lineSegment.xy;
        float2 param_1 = lineSegment.zw;
        coverages += computeCoverage(param, param_1, uAreaLUT, uAreaLUTSmplr);
        fillIndex = int(v_148.iFills[(fillIndex * 3) + 2]);
        iteration++;
    } while ((fillIndex >= 0) && (iteration < 1024));
    return coverages;
}

static inline __attribute__((always_inline))
int2 computeTileCoord(thread const uint& alphaTileIndex, thread uint3& gl_LocalInvocationID)
{
    uint x = alphaTileIndex & 255u;
    uint y = (alphaTileIndex >> 8u) & (255u + (((alphaTileIndex >> 16u) & 255u) << 8u));
    return (int2(16, 4) * int2(int(x), int(y))) + int2(gl_LocalInvocationID.xy);
}

kernel void main0(constant int2& uAlphaTileRange [[buffer(1)]], const device bFills& v_148 [[buffer(0)]], const device bAlphaTiles& _284 [[buffer(2)]], device bTiles& _294 [[buffer(3)]], texture2d<float> uAreaLUT [[texture(0)]], texture2d<float, access::read_write> uDest [[texture(1)]], sampler uAreaLUTSmplr [[sampler(0)]], uint3 gl_LocalInvocationID [[thread_position_in_threadgroup]], uint3 gl_WorkGroupID [[threadgroup_position_in_grid]])
{
    int2 tileSubCoord = int2(gl_LocalInvocationID.xy) * int2(1, 4);
    uint batchAlphaTileIndex = gl_WorkGroupID.x | (gl_WorkGroupID.y << uint(15));
    uint alphaTileIndex = batchAlphaTileIndex + uint(uAlphaTileRange.x);
    if (alphaTileIndex >= uint(uAlphaTileRange.y))
    {
        return;
    }
    uint tileIndex = _284.iAlphaTiles[(batchAlphaTileIndex * 2u) + 0u];
    if ((int(_294.iTiles[(tileIndex * 4u) + 2u] << uint(8)) >> 8) < 0)
    {
        return;
    }
    int fillIndex = int(_294.iTiles[(tileIndex * 4u) + 1u]);
    int backdrop = int(_294.iTiles[(tileIndex * 4u) + 3u]) >> 24;
    float4 coverages = float4(float(backdrop));
    int param = fillIndex;
    int2 param_1 = tileSubCoord;
    float4 _334 = accumulateCoverageForFillList(param, param_1, v_148, uAreaLUT, uAreaLUTSmplr);
    coverages += _334;
    coverages = fast::clamp(abs(coverages), float4(0.0), float4(1.0));
    int clipTileIndex = int(_284.iAlphaTiles[(batchAlphaTileIndex * 2u) + 1u]);
    if (clipTileIndex >= 0)
    {
        uint param_2 = uint(clipTileIndex);
        coverages = fast::min(coverages, uDest.read(uint2(computeTileCoord(param_2, gl_LocalInvocationID))));
    }
    uint param_3 = alphaTileIndex;
    uDest.write(coverages, uint2(computeTileCoord(param_3, gl_LocalInvocationID)));
}

