// Automatically generated from files in pathfinder/shaders/. Do not edit!
#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct main0_out
{
    float4 gl_Position [[position]];
};

struct main0_in
{
    int2 aTilePosition [[attribute(0)]];
};

vertex main0_out main0(main0_in in [[stage_in]], constant float2& uTileSize [[buffer(0)]], constant float4x4& uTransform [[buffer(1)]])
{
    main0_out out = {};
    float2 position = float2(in.aTilePosition) * uTileSize;
    out.gl_Position = uTransform * float4(position, 0.0, 1.0);
    return out;
}

