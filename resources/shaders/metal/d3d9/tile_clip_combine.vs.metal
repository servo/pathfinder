// Automatically generated from files in pathfinder/shaders/. Do not edit!
#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct main0_out
{
    float2 vTexCoord0 [[user(locn0)]];
    float vBackdrop0 [[user(locn1)]];
    float2 vTexCoord1 [[user(locn2)]];
    float vBackdrop1 [[user(locn3)]];
    float4 gl_Position [[position]];
};

struct main0_in
{
    int2 aTileOffset [[attribute(0)]];
    int aDestTileIndex [[attribute(1)]];
    int aDestBackdrop [[attribute(2)]];
    int aSrcTileIndex [[attribute(3)]];
    int aSrcBackdrop [[attribute(4)]];
};

vertex main0_out main0(main0_in in [[stage_in]], constant float2& uFramebufferSize [[buffer(0)]])
{
    main0_out out = {};
    float2 destPosition = float2(int2(in.aDestTileIndex % 256, in.aDestTileIndex / 256) + in.aTileOffset);
    float2 srcPosition = float2(int2(in.aSrcTileIndex % 256, in.aSrcTileIndex / 256) + in.aTileOffset);
    destPosition *= (float2(16.0, 4.0) / uFramebufferSize);
    srcPosition *= (float2(16.0, 4.0) / uFramebufferSize);
    out.vTexCoord0 = destPosition;
    out.vTexCoord1 = srcPosition;
    out.vBackdrop0 = float(in.aDestBackdrop);
    out.vBackdrop1 = float(in.aSrcBackdrop);
    if (in.aDestTileIndex < 0)
    {
        destPosition = float2(0.0);
    }
    destPosition.y = 1.0 - destPosition.y;
    out.gl_Position = float4(mix(float2(-1.0), float2(1.0), destPosition), 0.0, 1.0);
    return out;
}

