// Automatically generated from files in pathfinder/shaders/. Do not edit!
#pragma clang diagnostic ignored "-Wmissing-prototypes"

#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct main0_out
{
    float3 vMaskTexCoord0 [[user(locn0)]];
    float2 vColorTexCoord0 [[user(locn1)]];
    float4 vBaseColor [[user(locn2)]];
    float vTileCtrl [[user(locn3)]];
    float4 vFilterParams0 [[user(locn4)]];
    float4 vFilterParams1 [[user(locn5)]];
    float4 vFilterParams2 [[user(locn6)]];
    float4 vFilterParams3 [[user(locn7)]];
    float4 vFilterParams4 [[user(locn8)]];
    float vCtrl [[user(locn9)]];
    float4 gl_Position [[position]];
};

struct main0_in
{
    int2 aTileOffset [[attribute(0)]];
    int2 aTileOrigin [[attribute(1)]];
    uint4 aMaskTexCoord0 [[attribute(2)]];
    int2 aCtrlBackdrop [[attribute(3)]];
    int aPathIndex [[attribute(4)]];
    int aColor [[attribute(5)]];
};

static inline __attribute__((always_inline))
float4 fetchUnscaled(thread const texture2d<float> srcTexture, thread const sampler srcTextureSmplr, thread const float2& scale, thread const float2& originCoord, thread const int& entry)
{
    return srcTexture.sample(srcTextureSmplr, (((originCoord + float2(0.5)) + float2(float(entry), 0.0)) * scale), level(0.0));
}

static inline __attribute__((always_inline))
void computeTileVaryings(thread const float2& position, thread const int& colorEntry, thread const texture2d<float> textureMetadata, thread const sampler textureMetadataSmplr, thread const int2& textureMetadataSize, thread float2& outColorTexCoord0, thread float4& outBaseColor, thread float4& outFilterParams0, thread float4& outFilterParams1, thread float4& outFilterParams2, thread float4& outFilterParams3, thread float4& outFilterParams4, thread int& outCtrl)
{
    float2 metadataScale = float2(1.0) / float2(textureMetadataSize);
    float2 metadataEntryCoord = float2(float((colorEntry % 128) * 10), float(colorEntry / 128));
    float2 param = metadataScale;
    float2 param_1 = metadataEntryCoord;
    int param_2 = 0;
    float4 colorTexMatrix0 = fetchUnscaled(textureMetadata, textureMetadataSmplr, param, param_1, param_2);
    float2 param_3 = metadataScale;
    float2 param_4 = metadataEntryCoord;
    int param_5 = 1;
    float4 colorTexOffsets = fetchUnscaled(textureMetadata, textureMetadataSmplr, param_3, param_4, param_5);
    float2 param_6 = metadataScale;
    float2 param_7 = metadataEntryCoord;
    int param_8 = 2;
    float4 baseColor = fetchUnscaled(textureMetadata, textureMetadataSmplr, param_6, param_7, param_8);
    float2 param_9 = metadataScale;
    float2 param_10 = metadataEntryCoord;
    int param_11 = 3;
    float4 filterParams0 = fetchUnscaled(textureMetadata, textureMetadataSmplr, param_9, param_10, param_11);
    float2 param_12 = metadataScale;
    float2 param_13 = metadataEntryCoord;
    int param_14 = 4;
    float4 filterParams1 = fetchUnscaled(textureMetadata, textureMetadataSmplr, param_12, param_13, param_14);
    float2 param_15 = metadataScale;
    float2 param_16 = metadataEntryCoord;
    int param_17 = 5;
    float4 filterParams2 = fetchUnscaled(textureMetadata, textureMetadataSmplr, param_15, param_16, param_17);
    float2 param_18 = metadataScale;
    float2 param_19 = metadataEntryCoord;
    int param_20 = 6;
    float4 filterParams3 = fetchUnscaled(textureMetadata, textureMetadataSmplr, param_18, param_19, param_20);
    float2 param_21 = metadataScale;
    float2 param_22 = metadataEntryCoord;
    int param_23 = 7;
    float4 filterParams4 = fetchUnscaled(textureMetadata, textureMetadataSmplr, param_21, param_22, param_23);
    float2 param_24 = metadataScale;
    float2 param_25 = metadataEntryCoord;
    int param_26 = 8;
    float4 extra = fetchUnscaled(textureMetadata, textureMetadataSmplr, param_24, param_25, param_26);
    outColorTexCoord0 = (float2x2(float2(colorTexMatrix0.xy), float2(colorTexMatrix0.zw)) * position) + colorTexOffsets.xy;
    outBaseColor = baseColor;
    outFilterParams0 = filterParams0;
    outFilterParams1 = filterParams1;
    outFilterParams2 = filterParams2;
    outFilterParams3 = filterParams3;
    outFilterParams4 = filterParams4;
    outCtrl = int(extra.x);
}

vertex main0_out main0(main0_in in [[stage_in]], constant int2& uZBufferSize [[buffer(1)]], constant int2& uTextureMetadataSize [[buffer(2)]], constant float2& uTileSize [[buffer(0)]], constant float4x4& uTransform [[buffer(3)]], texture2d<float> uZBuffer [[texture(0)]], texture2d<float> uTextureMetadata [[texture(1)]], sampler uZBufferSmplr [[sampler(0)]], sampler uTextureMetadataSmplr [[sampler(1)]])
{
    main0_out out = {};
    float2 tileOrigin = float2(in.aTileOrigin);
    float2 tileOffset = float2(in.aTileOffset);
    float2 position = (tileOrigin + tileOffset) * uTileSize;
    int4 zValue = int4(uZBuffer.sample(uZBufferSmplr, ((tileOrigin + float2(0.5)) / float2(uZBufferSize)), level(0.0)) * 255.0);
    if (in.aPathIndex < (((zValue.x | (zValue.y << 8)) | (zValue.z << 16)) | (zValue.w << 24)))
    {
        out.gl_Position = float4(0.0);
        return out;
    }
    uint2 maskTileCoord = uint2(in.aMaskTexCoord0.x, in.aMaskTexCoord0.y + (256u * in.aMaskTexCoord0.z));
    float2 maskTexCoord0 = (float2(maskTileCoord) + tileOffset) * uTileSize;
    bool _264 = in.aCtrlBackdrop.y == 0;
    bool _270;
    if (_264)
    {
        _270 = in.aMaskTexCoord0.w != 0u;
    }
    else
    {
        _270 = _264;
    }
    if (_270)
    {
        out.gl_Position = float4(0.0);
        return out;
    }
    float2 param = position;
    int param_1 = in.aColor;
    int2 param_2 = uTextureMetadataSize;
    float2 param_3;
    float4 param_4;
    float4 param_5;
    float4 param_6;
    float4 param_7;
    float4 param_8;
    float4 param_9;
    int param_10;
    computeTileVaryings(param, param_1, uTextureMetadata, uTextureMetadataSmplr, param_2, param_3, param_4, param_5, param_6, param_7, param_8, param_9, param_10);
    out.vColorTexCoord0 = param_3;
    out.vBaseColor = param_4;
    out.vFilterParams0 = param_5;
    out.vFilterParams1 = param_6;
    out.vFilterParams2 = param_7;
    out.vFilterParams3 = param_8;
    out.vFilterParams4 = param_9;
    int ctrl = param_10;
    out.vTileCtrl = float(in.aCtrlBackdrop.x);
    out.vCtrl = float(ctrl);
    out.vMaskTexCoord0 = float3(maskTexCoord0, float(in.aCtrlBackdrop.y));
    out.gl_Position = uTransform * float4(position, 0.0, 1.0);
    return out;
}

