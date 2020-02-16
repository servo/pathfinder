// Automatically generated from files in pathfinder/shaders/. Do not edit!
#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct main0_out
{
    float2 vFillTexCoord [[user(locn0)]];
    float vBackdrop [[user(locn1)]];
    float4 gl_Position [[position]];
};

struct main0_in
{
    float2 aPosition [[attribute(0)]];
    float2 aFillTexCoord [[attribute(1)]];
    int aBackdrop [[attribute(2)]];
};

vertex main0_out main0(main0_in in [[stage_in]])
{
    main0_out out = {};
    float2 position = mix(float2(-1.0), float2(1.0), in.aPosition);
    position.y = -position.y;
    out.vFillTexCoord = in.aFillTexCoord;
    out.vBackdrop = float(in.aBackdrop);
    out.gl_Position = float4(position, 0.0, 1.0);
    return out;
}

