#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct main0_out
{
    float4 oFragColor [[color(0)]];
};

struct main0_in
{
    float2 vTexCoord [[user(locn0)]];
    float vBackdrop [[user(locn1)]];
    float4 vColor [[user(locn2)]];
};

fragment main0_out main0(main0_in in [[stage_in]], texture2d<float> uStencilTexture [[texture(0)]], sampler uStencilTextureSmplr [[sampler(0)]])
{
    main0_out out = {};
    float coverage = abs(uStencilTexture.sample(uStencilTextureSmplr, in.vTexCoord).x + in.vBackdrop);
    out.oFragColor = float4(in.vColor.xyz, in.vColor.w * coverage);
    return out;
}

