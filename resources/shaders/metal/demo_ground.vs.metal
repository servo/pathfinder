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
    int2 aPosition [[attribute(0)]];
};

vertex main0_out main0(main0_in in [[stage_in]], constant int& uGridlineCount [[buffer(0)]], constant float4x4& uTransform [[buffer(1)]])
{
    main0_out out = {};
    out.vTexCoord = float2(in.aPosition * int2(uGridlineCount));
    out.gl_Position = uTransform * float4(int4(in.aPosition.x, 0, in.aPosition.y, 1));
    return out;
}

