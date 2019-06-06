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
    float2 aTexCoord [[attribute(1)]];
};

vertex main0_out main0(main0_in in [[stage_in]], float2 uTextureSize [[buffer(0)]], float2 uFramebufferSize [[buffer(1)]], uint gl_VertexID [[vertex_id]], uint gl_InstanceID [[instance_id]])
{
    main0_out out = {};
    out.vTexCoord = in.aTexCoord / uTextureSize;
    float2 position = ((in.aPosition / uFramebufferSize) * 2.0) - float2(1.0);
    out.gl_Position = float4(position.x, -position.y, 0.0, 1.0);
    return out;
}

