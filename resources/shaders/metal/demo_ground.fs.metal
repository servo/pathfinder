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
};

fragment main0_out main0(main0_in in [[stage_in]], float4 uGridlineColor [[buffer(0)]], float4 uGroundColor [[buffer(1)]])
{
    main0_out out = {};
    float2 texCoordPx = fract(in.vTexCoord) / fwidth(in.vTexCoord);
    bool4 _33 = bool4(any(texCoordPx <= float2(1.0)));
    out.oFragColor = float4(_33.x ? uGridlineColor.x : uGroundColor.x, _33.y ? uGridlineColor.y : uGroundColor.y, _33.z ? uGridlineColor.z : uGroundColor.z, _33.w ? uGridlineColor.w : uGroundColor.w);
    return out;
}

