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
    float2 aPosition [[attribute(0)]];
};

vertex main0_out main0(main0_in in [[stage_in]], int uGridlineCount [[buffer(0)]], float4x4 uTransform [[buffer(1)]], uint gl_VertexID [[vertex_id]], uint gl_InstanceID [[instance_id]])
{
    main0_out out = {};
    out.vTexCoord = in.aPosition * float(uGridlineCount);
    out.gl_Position = uTransform * float4(in.aPosition.x, 0.0, in.aPosition.y, 1.0);
    return out;
}

