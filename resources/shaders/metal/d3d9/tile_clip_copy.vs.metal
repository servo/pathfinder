// Automatically generated from files in pathfinder/shaders/. Do not edit!
#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct main0_out
{
    float2 vTexCoord [[user(locn0)]];
    float4 gl_Position [[position]];
};

struct main0_in
{
    int2 aTileOffset [[attribute(0)]];
    int aTileIndex [[attribute(1)]];
};

vertex main0_out main0(main0_in in [[stage_in]], constant float2& uFramebufferSize [[buffer(0)]])
{
    main0_out out = {};
    float2 position = float2(int2(in.aTileIndex % 256, in.aTileIndex / 256) + in.aTileOffset);
    position *= (float2(16.0, 4.0) / uFramebufferSize);
    out.vTexCoord = position;
    if (in.aTileIndex < 0)
    {
        position = float2(0.0);
    }
    position.y = 1.0 - position.y;
    out.gl_Position = float4(mix(float2(-1.0), float2(1.0), position), 0.0, 1.0);
    return out;
}

