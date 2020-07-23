// Automatically generated from files in pathfinder/shaders/. Do not edit!
#pragma clang diagnostic ignored "-Wmissing-prototypes"

#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct bFirstTileMap
{
    int iFirstTileMap[1];
};

struct bTiles
{
    uint iTiles[1];
};

constant uint3 gl_WorkGroupSize [[maybe_unused]] = uint3(16u, 4u, 1u);

constant float3 _1149 = {};

// Implementation of the GLSL mod() function, which is slightly different than Metal fmod()
template<typename Tx, typename Ty>
inline Tx mod(Tx x, Ty y)
{
    return x - y * floor(x / y);
}

static inline __attribute__((always_inline))
int2 toImageCoords(thread const int2& coords, thread float2 uFramebufferSize)
{
    return int2(coords.x, int(uFramebufferSize.y - float(coords.y)));
}

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

static inline __attribute__((always_inline))
float sampleMask(thread const float& maskAlpha, thread const texture2d<float> maskTexture, thread const sampler maskTextureSmplr, thread const float2& maskTextureSize, thread const float3& maskTexCoord, thread const int& maskCtrl)
{
    if (maskCtrl == 0)
    {
        return maskAlpha;
    }
    int2 maskTexCoordI = int2(floor(maskTexCoord.xy));
    float4 texel = maskTexture.sample(maskTextureSmplr, ((float2(maskTexCoordI / int2(1, 4)) + float2(0.5)) / maskTextureSize), level(0.0));
    float coverage = texel[maskTexCoordI.y % 4] + maskTexCoord.z;
    if ((maskCtrl & 1) != 0)
    {
        coverage = abs(coverage);
    }
    else
    {
        coverage = 1.0 - abs(1.0 - mod(coverage, 2.0));
    }
    return fast::min(maskAlpha, coverage);
}

static inline __attribute__((always_inline))
float4 filterRadialGradient(thread const float2& colorTexCoord, thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr, thread const float2& colorTextureSize, thread const float2& fragCoord, thread const float2& framebufferSize, thread const float4& filterParams0, thread const float4& filterParams1)
{
    float2 lineFrom = filterParams0.xy;
    float2 lineVector = filterParams0.zw;
    float2 radii = filterParams1.xy;
    float2 uvOrigin = filterParams1.zw;
    float2 dP = colorTexCoord - lineFrom;
    float2 dC = lineVector;
    float dR = radii.y - radii.x;
    float a = dot(dC, dC) - (dR * dR);
    float b = dot(dP, dC) + (radii.x * dR);
    float c = dot(dP, dP) - (radii.x * radii.x);
    float discrim = (b * b) - (a * c);
    float4 color = float4(0.0);
    if (discrim != 0.0)
    {
        float2 ts = float2((float2(1.0, -1.0) * sqrt(discrim)) + float2(b)) / float2(a);
        if (ts.x > ts.y)
        {
            ts = ts.yx;
        }
        float _609;
        if (ts.x >= 0.0)
        {
            _609 = ts.x;
        }
        else
        {
            _609 = ts.y;
        }
        float t = _609;
        color = colorTexture.sample(colorTextureSmplr, (uvOrigin + float2(t, 0.0)), level(0.0));
    }
    return color;
}

static inline __attribute__((always_inline))
float4 filterBlur(thread const float2& colorTexCoord, thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr, thread const float2& colorTextureSize, thread const float4& filterParams0, thread const float4& filterParams1)
{
    float2 srcOffsetScale = filterParams0.xy / colorTextureSize;
    int support = int(filterParams0.z);
    float3 gaussCoeff = filterParams1.xyz;
    float gaussSum = gaussCoeff.x;
    float4 color = colorTexture.sample(colorTextureSmplr, colorTexCoord, level(0.0)) * gaussCoeff.x;
    float2 _653 = gaussCoeff.xy * gaussCoeff.yz;
    gaussCoeff = float3(_653.x, _653.y, gaussCoeff.z);
    for (int i = 1; i <= support; i += 2)
    {
        float gaussPartialSum = gaussCoeff.x;
        float2 _673 = gaussCoeff.xy * gaussCoeff.yz;
        gaussCoeff = float3(_673.x, _673.y, gaussCoeff.z);
        gaussPartialSum += gaussCoeff.x;
        float2 srcOffset = srcOffsetScale * (float(i) + (gaussCoeff.x / gaussPartialSum));
        color += ((colorTexture.sample(colorTextureSmplr, (colorTexCoord - srcOffset), level(0.0)) + colorTexture.sample(colorTextureSmplr, (colorTexCoord + srcOffset), level(0.0))) * gaussPartialSum);
        gaussSum += (2.0 * gaussPartialSum);
        float2 _713 = gaussCoeff.xy * gaussCoeff.yz;
        gaussCoeff = float3(_713.x, _713.y, gaussCoeff.z);
    }
    return color / float4(gaussSum);
}

static inline __attribute__((always_inline))
float filterTextSample1Tap(thread const float& offset, thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr, thread const float2& colorTexCoord)
{
    return colorTexture.sample(colorTextureSmplr, (colorTexCoord + float2(offset, 0.0)), level(0.0)).x;
}

static inline __attribute__((always_inline))
void filterTextSample9Tap(thread float4& outAlphaLeft, thread float& outAlphaCenter, thread float4& outAlphaRight, thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr, thread const float2& colorTexCoord, thread const float4& kernel0, thread const float& onePixel)
{
    bool wide = kernel0.x > 0.0;
    float _292;
    if (wide)
    {
        float param = (-4.0) * onePixel;
        float2 param_1 = colorTexCoord;
        _292 = filterTextSample1Tap(param, colorTexture, colorTextureSmplr, param_1);
    }
    else
    {
        _292 = 0.0;
    }
    float param_2 = (-3.0) * onePixel;
    float2 param_3 = colorTexCoord;
    float param_4 = (-2.0) * onePixel;
    float2 param_5 = colorTexCoord;
    float param_6 = (-1.0) * onePixel;
    float2 param_7 = colorTexCoord;
    outAlphaLeft = float4(_292, filterTextSample1Tap(param_2, colorTexture, colorTextureSmplr, param_3), filterTextSample1Tap(param_4, colorTexture, colorTextureSmplr, param_5), filterTextSample1Tap(param_6, colorTexture, colorTextureSmplr, param_7));
    float param_8 = 0.0;
    float2 param_9 = colorTexCoord;
    outAlphaCenter = filterTextSample1Tap(param_8, colorTexture, colorTextureSmplr, param_9);
    float param_10 = 1.0 * onePixel;
    float2 param_11 = colorTexCoord;
    float param_12 = 2.0 * onePixel;
    float2 param_13 = colorTexCoord;
    float param_14 = 3.0 * onePixel;
    float2 param_15 = colorTexCoord;
    float _352;
    if (wide)
    {
        float param_16 = 4.0 * onePixel;
        float2 param_17 = colorTexCoord;
        _352 = filterTextSample1Tap(param_16, colorTexture, colorTextureSmplr, param_17);
    }
    else
    {
        _352 = 0.0;
    }
    outAlphaRight = float4(filterTextSample1Tap(param_10, colorTexture, colorTextureSmplr, param_11), filterTextSample1Tap(param_12, colorTexture, colorTextureSmplr, param_13), filterTextSample1Tap(param_14, colorTexture, colorTextureSmplr, param_15), _352);
}

static inline __attribute__((always_inline))
float filterTextConvolve7Tap(thread const float4& alpha0, thread const float3& alpha1, thread const float4& kernel0)
{
    return dot(alpha0, kernel0) + dot(alpha1, kernel0.zyx);
}

static inline __attribute__((always_inline))
float filterTextGammaCorrectChannel(thread const float& bgColor, thread const float& fgColor, thread const texture2d<float> gammaLUT, thread const sampler gammaLUTSmplr)
{
    return gammaLUT.sample(gammaLUTSmplr, float2(fgColor, 1.0 - bgColor), level(0.0)).x;
}

static inline __attribute__((always_inline))
float3 filterTextGammaCorrect(thread const float3& bgColor, thread const float3& fgColor, thread const texture2d<float> gammaLUT, thread const sampler gammaLUTSmplr)
{
    float param = bgColor.x;
    float param_1 = fgColor.x;
    float param_2 = bgColor.y;
    float param_3 = fgColor.y;
    float param_4 = bgColor.z;
    float param_5 = fgColor.z;
    return float3(filterTextGammaCorrectChannel(param, param_1, gammaLUT, gammaLUTSmplr), filterTextGammaCorrectChannel(param_2, param_3, gammaLUT, gammaLUTSmplr), filterTextGammaCorrectChannel(param_4, param_5, gammaLUT, gammaLUTSmplr));
}

static inline __attribute__((always_inline))
float4 filterText(thread const float2& colorTexCoord, thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr, thread const texture2d<float> gammaLUT, thread const sampler gammaLUTSmplr, thread const float2& colorTextureSize, thread const float4& filterParams0, thread const float4& filterParams1, thread const float4& filterParams2)
{
    float4 kernel0 = filterParams0;
    float3 bgColor = filterParams1.xyz;
    float3 fgColor = filterParams2.xyz;
    bool gammaCorrectionEnabled = filterParams2.w != 0.0;
    float3 alpha;
    if (kernel0.w == 0.0)
    {
        alpha = colorTexture.sample(colorTextureSmplr, colorTexCoord, level(0.0)).xxx;
    }
    else
    {
        float2 param_3 = colorTexCoord;
        float4 param_4 = kernel0;
        float param_5 = 1.0 / colorTextureSize.x;
        float4 param;
        float param_1;
        float4 param_2;
        filterTextSample9Tap(param, param_1, param_2, colorTexture, colorTextureSmplr, param_3, param_4, param_5);
        float4 alphaLeft = param;
        float alphaCenter = param_1;
        float4 alphaRight = param_2;
        float4 param_6 = alphaLeft;
        float3 param_7 = float3(alphaCenter, alphaRight.xy);
        float4 param_8 = kernel0;
        float r = filterTextConvolve7Tap(param_6, param_7, param_8);
        float4 param_9 = float4(alphaLeft.yzw, alphaCenter);
        float3 param_10 = alphaRight.xyz;
        float4 param_11 = kernel0;
        float g = filterTextConvolve7Tap(param_9, param_10, param_11);
        float4 param_12 = float4(alphaLeft.zw, alphaCenter, alphaRight.x);
        float3 param_13 = alphaRight.yzw;
        float4 param_14 = kernel0;
        float b = filterTextConvolve7Tap(param_12, param_13, param_14);
        alpha = float3(r, g, b);
    }
    if (gammaCorrectionEnabled)
    {
        float3 param_15 = bgColor;
        float3 param_16 = alpha;
        alpha = filterTextGammaCorrect(param_15, param_16, gammaLUT, gammaLUTSmplr);
    }
    return float4(mix(bgColor, fgColor, alpha), 1.0);
}

static inline __attribute__((always_inline))
float4 filterColorMatrix(thread const float2& colorTexCoord, thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr, thread const float4& filterParams0, thread const float4& filterParams1, thread const float4& filterParams2, thread const float4& filterParams3, thread const float4& filterParams4)
{
    float4 srcColor = colorTexture.sample(colorTextureSmplr, colorTexCoord, level(0.0));
    float4x4 colorMatrix = float4x4(float4(filterParams0), float4(filterParams1), float4(filterParams2), float4(filterParams3));
    return (colorMatrix * srcColor) + filterParams4;
}

static inline __attribute__((always_inline))
float4 sampleColor(thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr, thread const float2& colorTexCoord)
{
    return colorTexture.sample(colorTextureSmplr, colorTexCoord, level(0.0));
}

static inline __attribute__((always_inline))
float4 filterNone(thread const float2& colorTexCoord, thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr)
{
    float2 param = colorTexCoord;
    return sampleColor(colorTexture, colorTextureSmplr, param);
}

static inline __attribute__((always_inline))
float4 filterColor(thread const float2& colorTexCoord, thread const texture2d<float> colorTexture, thread const sampler colorTextureSmplr, thread const texture2d<float> gammaLUT, thread const sampler gammaLUTSmplr, thread const float2& colorTextureSize, thread const float2& fragCoord, thread const float2& framebufferSize, thread const float4& filterParams0, thread const float4& filterParams1, thread const float4& filterParams2, thread const float4& filterParams3, thread const float4& filterParams4, thread const int& colorFilter)
{
    switch (colorFilter)
    {
        case 1:
        {
            float2 param = colorTexCoord;
            float2 param_1 = colorTextureSize;
            float2 param_2 = fragCoord;
            float2 param_3 = framebufferSize;
            float4 param_4 = filterParams0;
            float4 param_5 = filterParams1;
            return filterRadialGradient(param, colorTexture, colorTextureSmplr, param_1, param_2, param_3, param_4, param_5);
        }
        case 3:
        {
            float2 param_6 = colorTexCoord;
            float2 param_7 = colorTextureSize;
            float4 param_8 = filterParams0;
            float4 param_9 = filterParams1;
            return filterBlur(param_6, colorTexture, colorTextureSmplr, param_7, param_8, param_9);
        }
        case 2:
        {
            float2 param_10 = colorTexCoord;
            float2 param_11 = colorTextureSize;
            float4 param_12 = filterParams0;
            float4 param_13 = filterParams1;
            float4 param_14 = filterParams2;
            return filterText(param_10, colorTexture, colorTextureSmplr, gammaLUT, gammaLUTSmplr, param_11, param_12, param_13, param_14);
        }
        case 4:
        {
            float2 param_15 = colorTexCoord;
            float4 param_16 = filterParams0;
            float4 param_17 = filterParams1;
            float4 param_18 = filterParams2;
            float4 param_19 = filterParams3;
            float4 param_20 = filterParams4;
            return filterColorMatrix(param_15, colorTexture, colorTextureSmplr, param_16, param_17, param_18, param_19, param_20);
        }
    }
    float2 param_21 = colorTexCoord;
    return filterNone(param_21, colorTexture, colorTextureSmplr);
}

static inline __attribute__((always_inline))
float4 combineColor0(thread const float4& destColor, thread const float4& srcColor, thread const int& op)
{
    switch (op)
    {
        case 1:
        {
            return float4(srcColor.xyz, srcColor.w * destColor.w);
        }
        case 2:
        {
            return float4(destColor.xyz, srcColor.w * destColor.w);
        }
    }
    return destColor;
}

static inline __attribute__((always_inline))
float3 compositeScreen(thread const float3& destColor, thread const float3& srcColor)
{
    return (destColor + srcColor) - (destColor * srcColor);
}

static inline __attribute__((always_inline))
float3 compositeSelect(thread const bool3& cond, thread const float3& ifTrue, thread const float3& ifFalse)
{
    float _833;
    if (cond.x)
    {
        _833 = ifTrue.x;
    }
    else
    {
        _833 = ifFalse.x;
    }
    float _844;
    if (cond.y)
    {
        _844 = ifTrue.y;
    }
    else
    {
        _844 = ifFalse.y;
    }
    float _855;
    if (cond.z)
    {
        _855 = ifTrue.z;
    }
    else
    {
        _855 = ifFalse.z;
    }
    return float3(_833, _844, _855);
}

static inline __attribute__((always_inline))
float3 compositeHardLight(thread const float3& destColor, thread const float3& srcColor)
{
    float3 param = destColor;
    float3 param_1 = (float3(2.0) * srcColor) - float3(1.0);
    bool3 param_2 = srcColor <= float3(0.5);
    float3 param_3 = (destColor * float3(2.0)) * srcColor;
    float3 param_4 = compositeScreen(param, param_1);
    return compositeSelect(param_2, param_3, param_4);
}

static inline __attribute__((always_inline))
float3 compositeColorDodge(thread const float3& destColor, thread const float3& srcColor)
{
    bool3 destZero = destColor == float3(0.0);
    bool3 srcOne = srcColor == float3(1.0);
    bool3 param = srcOne;
    float3 param_1 = float3(1.0);
    float3 param_2 = destColor / (float3(1.0) - srcColor);
    bool3 param_3 = destZero;
    float3 param_4 = float3(0.0);
    float3 param_5 = compositeSelect(param, param_1, param_2);
    return compositeSelect(param_3, param_4, param_5);
}

static inline __attribute__((always_inline))
float3 compositeSoftLight(thread const float3& destColor, thread const float3& srcColor)
{
    bool3 param = destColor <= float3(0.25);
    float3 param_1 = ((((float3(16.0) * destColor) - float3(12.0)) * destColor) + float3(4.0)) * destColor;
    float3 param_2 = sqrt(destColor);
    float3 darkenedDestColor = compositeSelect(param, param_1, param_2);
    bool3 param_3 = srcColor <= float3(0.5);
    float3 param_4 = destColor * (float3(1.0) - destColor);
    float3 param_5 = darkenedDestColor - destColor;
    float3 factor = compositeSelect(param_3, param_4, param_5);
    return destColor + (((srcColor * 2.0) - float3(1.0)) * factor);
}

static inline __attribute__((always_inline))
float compositeDivide(thread const float& num, thread const float& denom)
{
    float _869;
    if (denom != 0.0)
    {
        _869 = num / denom;
    }
    else
    {
        _869 = 0.0;
    }
    return _869;
}

static inline __attribute__((always_inline))
float3 compositeRGBToHSL(thread const float3& rgb)
{
    float v = fast::max(fast::max(rgb.x, rgb.y), rgb.z);
    float xMin = fast::min(fast::min(rgb.x, rgb.y), rgb.z);
    float c = v - xMin;
    float l = mix(xMin, v, 0.5);
    float3 _975;
    if (rgb.x == v)
    {
        _975 = float3(0.0, rgb.yz);
    }
    else
    {
        float3 _988;
        if (rgb.y == v)
        {
            _988 = float3(2.0, rgb.zx);
        }
        else
        {
            _988 = float3(4.0, rgb.xy);
        }
        _975 = _988;
    }
    float3 terms = _975;
    float param = ((terms.x * c) + terms.y) - terms.z;
    float param_1 = c;
    float h = 1.0471975803375244140625 * compositeDivide(param, param_1);
    float param_2 = c;
    float param_3 = v;
    float s = compositeDivide(param_2, param_3);
    return float3(h, s, l);
}

static inline __attribute__((always_inline))
float3 compositeHSL(thread const float3& destColor, thread const float3& srcColor, thread const int& op)
{
    switch (op)
    {
        case 12:
        {
            return float3(srcColor.x, destColor.y, destColor.z);
        }
        case 13:
        {
            return float3(destColor.x, srcColor.y, destColor.z);
        }
        case 14:
        {
            return float3(srcColor.x, srcColor.y, destColor.z);
        }
        default:
        {
            return float3(destColor.x, destColor.y, srcColor.z);
        }
    }
}

static inline __attribute__((always_inline))
float3 compositeHSLToRGB(thread const float3& hsl)
{
    float a = hsl.y * fast::min(hsl.z, 1.0 - hsl.z);
    float3 ks = mod(float3(0.0, 8.0, 4.0) + float3(hsl.x * 1.90985929965972900390625), float3(12.0));
    return hsl.zzz - (fast::clamp(fast::min(ks - float3(3.0), float3(9.0) - ks), float3(-1.0), float3(1.0)) * a);
}

static inline __attribute__((always_inline))
float3 compositeRGB(thread const float3& destColor, thread const float3& srcColor, thread const int& op)
{
    switch (op)
    {
        case 1:
        {
            return destColor * srcColor;
        }
        case 2:
        {
            float3 param = destColor;
            float3 param_1 = srcColor;
            return compositeScreen(param, param_1);
        }
        case 3:
        {
            float3 param_2 = srcColor;
            float3 param_3 = destColor;
            return compositeHardLight(param_2, param_3);
        }
        case 4:
        {
            return fast::min(destColor, srcColor);
        }
        case 5:
        {
            return fast::max(destColor, srcColor);
        }
        case 6:
        {
            float3 param_4 = destColor;
            float3 param_5 = srcColor;
            return compositeColorDodge(param_4, param_5);
        }
        case 7:
        {
            float3 param_6 = float3(1.0) - destColor;
            float3 param_7 = float3(1.0) - srcColor;
            return float3(1.0) - compositeColorDodge(param_6, param_7);
        }
        case 8:
        {
            float3 param_8 = destColor;
            float3 param_9 = srcColor;
            return compositeHardLight(param_8, param_9);
        }
        case 9:
        {
            float3 param_10 = destColor;
            float3 param_11 = srcColor;
            return compositeSoftLight(param_10, param_11);
        }
        case 10:
        {
            return abs(destColor - srcColor);
        }
        case 11:
        {
            return (destColor + srcColor) - ((float3(2.0) * destColor) * srcColor);
        }
        case 12:
        case 13:
        case 14:
        case 15:
        {
            float3 param_12 = destColor;
            float3 param_13 = srcColor;
            float3 param_14 = compositeRGBToHSL(param_12);
            float3 param_15 = compositeRGBToHSL(param_13);
            int param_16 = op;
            float3 param_17 = compositeHSL(param_14, param_15, param_16);
            return compositeHSLToRGB(param_17);
        }
    }
    return srcColor;
}

static inline __attribute__((always_inline))
float4 composite(thread const float4& srcColor, thread const texture2d<float> destTexture, thread const sampler destTextureSmplr, thread const float2& destTextureSize, thread const float2& fragCoord, thread const int& op)
{
    if (op == 0)
    {
        return srcColor;
    }
    float2 destTexCoord = fragCoord / destTextureSize;
    float4 destColor = destTexture.sample(destTextureSmplr, destTexCoord, level(0.0));
    float3 param = destColor.xyz;
    float3 param_1 = srcColor.xyz;
    int param_2 = op;
    float3 blendedRGB = compositeRGB(param, param_1, param_2);
    return float4(((srcColor.xyz * (srcColor.w * (1.0 - destColor.w))) + (blendedRGB * (srcColor.w * destColor.w))) + (destColor.xyz * (1.0 - srcColor.w)), 1.0);
}

static inline __attribute__((always_inline))
float4 calculateColor(thread const float2& fragCoord, thread const texture2d<float> colorTexture0, thread const sampler colorTexture0Smplr, thread const texture2d<float> maskTexture0, thread const sampler maskTexture0Smplr, thread const texture2d<float> destTexture, thread const sampler destTextureSmplr, thread const texture2d<float> gammaLUT, thread const sampler gammaLUTSmplr, thread const float2& colorTextureSize0, thread const float2& maskTextureSize0, thread const float4& filterParams0, thread const float4& filterParams1, thread const float4& filterParams2, thread const float4& filterParams3, thread const float4& filterParams4, thread const float2& framebufferSize, thread const int& ctrl, thread const float3& maskTexCoord0, thread const float2& colorTexCoord0, thread const float4& baseColor, thread const int& tileCtrl)
{
    int maskCtrl0 = (tileCtrl >> 0) & 3;
    float maskAlpha = 1.0;
    float param = maskAlpha;
    float2 param_1 = maskTextureSize0;
    float3 param_2 = maskTexCoord0;
    int param_3 = maskCtrl0;
    maskAlpha = sampleMask(param, maskTexture0, maskTexture0Smplr, param_1, param_2, param_3);
    float4 color = baseColor;
    int color0Combine = (ctrl >> 8) & 3;
    if (color0Combine != 0)
    {
        int color0Filter = (ctrl >> 4) & 15;
        float2 param_4 = colorTexCoord0;
        float2 param_5 = colorTextureSize0;
        float2 param_6 = fragCoord;
        float2 param_7 = framebufferSize;
        float4 param_8 = filterParams0;
        float4 param_9 = filterParams1;
        float4 param_10 = filterParams2;
        float4 param_11 = filterParams3;
        float4 param_12 = filterParams4;
        int param_13 = color0Filter;
        float4 color0 = filterColor(param_4, colorTexture0, colorTexture0Smplr, gammaLUT, gammaLUTSmplr, param_5, param_6, param_7, param_8, param_9, param_10, param_11, param_12, param_13);
        float4 param_14 = color;
        float4 param_15 = color0;
        int param_16 = color0Combine;
        color = combineColor0(param_14, param_15, param_16);
    }
    color.w *= maskAlpha;
    int compositeOp = (ctrl >> 10) & 15;
    float4 param_17 = color;
    float2 param_18 = framebufferSize;
    float2 param_19 = fragCoord;
    int param_20 = compositeOp;
    color = composite(param_17, destTexture, destTextureSmplr, param_18, param_19, param_20);
    float3 _1435 = color.xyz * color.w;
    color = float4(_1435.x, _1435.y, _1435.z, color.w);
    return color;
}

kernel void main0(constant int2& uFramebufferTileSize [[buffer(3)]], constant int& uLoadAction [[buffer(4)]], constant int2& uTextureMetadataSize [[buffer(7)]], constant float2& uFramebufferSize [[buffer(0)]], constant float2& uTileSize [[buffer(1)]], constant float4& uClearColor [[buffer(5)]], constant float2& uColorTextureSize0 [[buffer(8)]], constant float2& uMaskTextureSize0 [[buffer(9)]], const device bFirstTileMap& _1599 [[buffer(2)]], const device bTiles& _1690 [[buffer(6)]], texture2d<float, access::read_write> uDestImage [[texture(0)]], texture2d<float> uTextureMetadata [[texture(1)]], texture2d<float> uColorTexture0 [[texture(2)]], texture2d<float> uMaskTexture0 [[texture(3)]], texture2d<float> uGammaLUT [[texture(4)]], sampler uTextureMetadataSmplr [[sampler(0)]], sampler uColorTexture0Smplr [[sampler(1)]], sampler uMaskTexture0Smplr [[sampler(2)]], sampler uGammaLUTSmplr [[sampler(3)]], uint3 gl_WorkGroupID [[threadgroup_position_in_grid]], uint3 gl_LocalInvocationID [[thread_position_in_threadgroup]])
{
    int2 tileCoord = int2(gl_WorkGroupID.xy);
    int2 firstTileSubCoord = int2(gl_LocalInvocationID.xy) * int2(1, 4);
    int2 firstFragCoord = (tileCoord * int2(uTileSize)) + firstTileSubCoord;
    int tileIndex = _1599.iFirstTileMap[tileCoord.x + (uFramebufferTileSize.x * tileCoord.y)];
    if ((tileIndex < 0) && (uLoadAction != 0))
    {
        return;
    }
    float4x4 destColors;
    for (int subY = 0; subY < 4; subY++)
    {
        if (uLoadAction == 0)
        {
            destColors[subY] = uClearColor;
        }
        else
        {
            int2 param = firstFragCoord + int2(0, subY);
            int2 imageCoords = toImageCoords(param, uFramebufferSize);
            destColors[subY] = uDestImage.read(uint2(imageCoords));
        }
    }
    int backdrop;
    uint2 maskTileCoord;
    float2 param_4;
    float4 param_5;
    float4 param_6;
    float4 param_7;
    float4 param_8;
    float4 param_9;
    float4 param_10;
    int param_11;
    while (tileIndex >= 0)
    {
        for (int subY_1 = 0; subY_1 < 4; subY_1++)
        {
            int2 tileSubCoord = firstTileSubCoord + int2(0, subY_1);
            float2 fragCoord = float2(firstFragCoord + int2(0, subY_1)) + float2(0.5);
            int alphaTileIndex = int(_1690.iTiles[(tileIndex * 4) + 2] << uint(8)) >> 8;
            uint tileControlWord = _1690.iTiles[(tileIndex * 4) + 3];
            uint colorEntry = tileControlWord & 65535u;
            int tileCtrl = int((tileControlWord >> uint(16)) & 255u);
            if (alphaTileIndex >= 0)
            {
                backdrop = 0;
                maskTileCoord = uint2(uint(alphaTileIndex & 255), uint(alphaTileIndex >> 8)) * uint2(uTileSize);
            }
            else
            {
                backdrop = int(tileControlWord) >> 24;
                maskTileCoord = uint2(0u);
                tileCtrl &= (-4);
            }
            float3 maskTexCoord0 = float3(float2(int2(maskTileCoord) + tileSubCoord), float(backdrop));
            float2 param_1 = fragCoord;
            int param_2 = int(colorEntry);
            int2 param_3 = uTextureMetadataSize;
            computeTileVaryings(param_1, param_2, uTextureMetadata, uTextureMetadataSmplr, param_3, param_4, param_5, param_6, param_7, param_8, param_9, param_10, param_11);
            float2 colorTexCoord0 = param_4;
            float4 baseColor = param_5;
            float4 filterParams0 = param_6;
            float4 filterParams1 = param_7;
            float4 filterParams2 = param_8;
            float4 filterParams3 = param_9;
            float4 filterParams4 = param_10;
            int ctrl = param_11;
            float2 param_12 = fragCoord;
            float2 param_13 = uColorTextureSize0;
            float2 param_14 = uMaskTextureSize0;
            float4 param_15 = filterParams0;
            float4 param_16 = filterParams1;
            float4 param_17 = filterParams2;
            float4 param_18 = filterParams3;
            float4 param_19 = filterParams4;
            float2 param_20 = uFramebufferSize;
            int param_21 = ctrl;
            float3 param_22 = maskTexCoord0;
            float2 param_23 = colorTexCoord0;
            float4 param_24 = baseColor;
            int param_25 = tileCtrl;
            float4 srcColor = calculateColor(param_12, uColorTexture0, uColorTexture0Smplr, uMaskTexture0, uMaskTexture0Smplr, uColorTexture0, uColorTexture0Smplr, uGammaLUT, uGammaLUTSmplr, param_13, param_14, param_15, param_16, param_17, param_18, param_19, param_20, param_21, param_22, param_23, param_24, param_25);
            destColors[subY_1] = (destColors[subY_1] * (1.0 - srcColor.w)) + srcColor;
        }
        tileIndex = int(_1690.iTiles[(tileIndex * 4) + 0]);
    }
    for (int subY_2 = 0; subY_2 < 4; subY_2++)
    {
        int2 param_26 = firstFragCoord + int2(0, subY_2);
        uDestImage.write(destColors[subY_2], uint2(toImageCoords(param_26, uFramebufferSize)));
    }
}

