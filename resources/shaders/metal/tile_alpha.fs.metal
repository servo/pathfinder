// Automatically generated from files in pathfinder/shaders/. Do not edit!
#pragma clang diagnostic ignored "-Wmissing-prototypes"

#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct spvDescriptorSetBuffer0
{
    constant float2* uTileSize [[id(0)]];
    constant float2* uStencilTextureSize [[id(1)]];
    texture2d<float> uStencilTexture [[id(2)]];
    sampler uStencilTextureSmplr [[id(3)]];
};

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

// Implementation of the GLSL mod() function, which is slightly different than Metal fmod()
template<typename Tx, typename Ty>
Tx mod(Tx x, Ty y)
{
    return x - y * floor(x / y);
}

float avoidTileEdges(thread const float& edgeCoord, thread const float& texCoord, thread const float& tileSize)
{
    if (edgeCoord < 0.5)
    {
        return (texCoord - edgeCoord) + 0.5;
    }
    else
    {
        if (edgeCoord > (tileSize - 0.5))
        {
            return (texCoord - edgeCoord) + 15.5;
        }
    }
    return texCoord;
}

fragment main0_out main0(main0_in in [[stage_in]], constant spvDescriptorSetBuffer0& spvDescriptorSet0 [[buffer(0)]])
{
    main0_out out = {};
    float2 texCoord = in.vTexCoord;
    float2 edgeCoord = mod(texCoord, (*spvDescriptorSet0.uTileSize));
    float param = edgeCoord.x;
    float param_1 = texCoord.x;
    float param_2 = (*spvDescriptorSet0.uTileSize).x;
    texCoord.x = avoidTileEdges(param, param_1, param_2);
    float param_3 = edgeCoord.y;
    float param_4 = texCoord.y;
    float param_5 = (*spvDescriptorSet0.uTileSize).y;
    texCoord.y = avoidTileEdges(param_3, param_4, param_5);
    texCoord /= (*spvDescriptorSet0.uStencilTextureSize);
    float coverage = abs(spvDescriptorSet0.uStencilTexture.sample(spvDescriptorSet0.uStencilTextureSmplr, texCoord).x + in.vBackdrop);
    out.oFragColor = float4(in.vColor.xyz, in.vColor.w * coverage);
    return out;
}

