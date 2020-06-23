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

vertex main0_out main0(main0_in in [[stage_in]], constant float4& uDestRect [[buffer(0)]], constant float2& uFramebufferSize [[buffer(1)]])
{
    main0_out out = {};
    float2 position = mix(uDestRect.xy, uDestRect.zw, float2(in.aPosition)) / uFramebufferSize;
    float2 texCoord = float2(in.aPosition);
    out.vTexCoord = texCoord;
    out.gl_Position = float4(mix(float2(-1.0), float2(1.0), position), 0.0, 1.0);
    return out;
}

