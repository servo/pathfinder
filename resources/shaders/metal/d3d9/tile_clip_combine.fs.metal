// Automatically generated from files in pathfinder/shaders/. Do not edit!
#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct main0_out
{
    float4 oFragColor [[color(0)]];
};

struct main0_in
{
    float2 vTexCoord0 [[user(locn0)]];
    float vBackdrop0 [[user(locn1)]];
    float2 vTexCoord1 [[user(locn2)]];
    float vBackdrop1 [[user(locn3)]];
};

fragment main0_out main0(main0_in in [[stage_in]], texture2d<float> uSrc [[texture(0)]], sampler uSrcSmplr [[sampler(0)]])
{
    main0_out out = {};
    out.oFragColor = fast::min(abs(uSrc.sample(uSrcSmplr, in.vTexCoord0) + float4(in.vBackdrop0)), abs(uSrc.sample(uSrcSmplr, in.vTexCoord1) + float4(in.vBackdrop1)));
    return out;
}

