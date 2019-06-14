#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct spvDescriptorSetBuffer0
{
    texture2d<float> uAreaLUT [[id(0)]];
    sampler uAreaLUTSmplr [[id(1)]];
};

struct main0_out
{
    float4 oFragColor [[color(0)]];
};

struct main0_in
{
    float2 vFrom [[user(locn0)]];
    float2 vTo [[user(locn1)]];
};

fragment main0_out main0(main0_in in [[stage_in]], constant spvDescriptorSetBuffer0& spvDescriptorSet0 [[buffer(0)]])
{
    main0_out out = {};
    float2 from = in.vFrom;
    float2 to = in.vTo;

    bool2 _29 = bool2(from.x < to.x);
    float2 left = float2(_29.x ? from.x : to.x, _29.y ? from.y : to.y);
    bool2 _39 = bool2(from.x < to.x);
    float2 right = float2(_39.x ? to.x : from.x, _39.y ? to.y : from.y);
    float2 window = fast::clamp(float2(from.x, to.x), float2(-0.5), float2(0.5));
    float offset = mix(window.x, window.y, 0.5) - left.x;
    float t = offset / (right.x - left.x);
    float y = mix(left.y, right.y, t);
    float d = (right.y - left.y) / (right.x - left.x);
    float dX = window.x - window.y;
    out.oFragColor = float4(spvDescriptorSet0.uAreaLUT.sample(spvDescriptorSet0.uAreaLUTSmplr, (float2(y + 8.0, abs(d * dX)) / float2(16.0))).x * dX);

    /*
    float2 window = fast::clamp(float2(from.x, to.x), float2(-0.5), float2(0.5));

    float2 a = from.y + (window - from.x) * (to.y - from.y) / (to.x - from.x) + 0.5;
    float ymin = fast::min(fast::min(a.x, a.y), 1) - 1e-6;
    float ymax = fast::max(a.x, a.y);
    float b = fast::min(ymax, 1.0);
    float c = fast::max(b, 0.0);
    float d = fast::max(ymin, 0.0);
    float tex = (b - 0.5 * c * c - ymin + 0.5 * d * d) / (ymax - ymin);
    float dX = window.x - window.y;
    out.oFragColor = float4(tex * dX);
    */

    return out;
}

