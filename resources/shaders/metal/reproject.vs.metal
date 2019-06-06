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

vertex main0_out main0(main0_in in [[stage_in]], float4x4 uNewTransform [[buffer(0)]], uint gl_VertexID [[vertex_id]], uint gl_InstanceID [[instance_id]])
{
    main0_out out = {};
    out.vTexCoord = in.aPosition;
    out.gl_Position = uNewTransform * float4(in.aPosition, 0.0, 1.0);
    return out;
}

