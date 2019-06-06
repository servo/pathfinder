#pragma clang diagnostic ignored "-Wmissing-prototypes"

#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct main0_out
{
    float4 vColor [[user(locn0)]];
    float4 gl_Position [[position]];
};

struct main0_in
{
    float2 aTessCoord [[attribute(0)]];
    float2 aTileOrigin [[attribute(1)]];
    float2 aColorTexCoord [[attribute(2)]];
};

float4 getColor(thread texture2d<float> uPaintTexture, thread const sampler uPaintTextureSmplr, thread float2& aColorTexCoord)
{
    return uPaintTexture.sample(uPaintTextureSmplr, aColorTexCoord, level(0.0));
}

void computeVaryings(thread float2& aTileOrigin, thread float2& aTessCoord, thread float2 uTileSize, thread float2 uViewBoxOrigin, thread float2 uFramebufferSize, thread float4& vColor, thread float4& gl_Position, thread texture2d<float> uPaintTexture, thread const sampler uPaintTextureSmplr, thread float2& aColorTexCoord)
{
    float2 pixelPosition = ((aTileOrigin + aTessCoord) * uTileSize) + uViewBoxOrigin;
    float2 position = (((pixelPosition / uFramebufferSize) * 2.0) - float2(1.0)) * float2(1.0, -1.0);
    vColor = getColor(uPaintTexture, uPaintTextureSmplr, aColorTexCoord);
    gl_Position = float4(position, 0.0, 1.0);
}

vertex main0_out main0(main0_in in [[stage_in]], float2 uTileSize [[buffer(0)]], float2 uViewBoxOrigin [[buffer(1)]], float2 uFramebufferSize [[buffer(2)]], texture2d<float> uPaintTexture [[texture(0)]], sampler uPaintTextureSmplr [[sampler(0)]], uint gl_VertexID [[vertex_id]], uint gl_InstanceID [[instance_id]])
{
    main0_out out = {};
    computeVaryings(in.aTileOrigin, in.aTessCoord, uTileSize, uViewBoxOrigin, uFramebufferSize, out.vColor, out.gl_Position, uPaintTexture, uPaintTextureSmplr, in.aColorTexCoord);
    return out;
}

