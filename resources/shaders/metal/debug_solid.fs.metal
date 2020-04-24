// Automatically generated from files in pathfinder/shaders/. Do not edit!
#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct main0_out
{
    float4 oFragColor [[color(0)]];
};

fragment main0_out main0(constant float4& uColor [[buffer(0)]])
{
    main0_out out = {};
    out.oFragColor = float4(uColor.xyz, 1.0) * uColor.w;
    return out;
}

