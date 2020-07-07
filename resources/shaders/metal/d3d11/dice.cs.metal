// Automatically generated from files in pathfinder/shaders/. Do not edit!
#pragma clang diagnostic ignored "-Wmissing-prototypes"
#pragma clang diagnostic ignored "-Wunused-variable"

#include <metal_stdlib>
#include <simd/simd.h>
#include <metal_atomic>

using namespace metal;

struct bMicrolines
{
    uint4 iMicrolines[1];
};

struct bPoints
{
    float2 iPoints[1];
};

struct bDiceMetadata
{
    uint4 iDiceMetadata[1];
};

struct bInputIndices
{
    uint2 iInputIndices[1];
};

struct bComputeIndirectParams
{
    uint iComputeIndirectParams[1];
};

constant uint3 gl_WorkGroupSize [[maybe_unused]] = uint3(64u, 1u, 1u);

static inline __attribute__((always_inline))
float2 getPoint(thread const uint& pointIndex, thread float2x2 uTransform, const device bPoints& v_194, thread float2 uTranslation)
{
    return (uTransform * v_194.iPoints[pointIndex]) + uTranslation;
}

static inline __attribute__((always_inline))
float2 sampleCurve(thread const float4& baseline, thread const float4& ctrl, thread const float& t)
{
    float2 p0 = baseline.xy;
    float2 p1 = ctrl.xy;
    float2 p2 = ctrl.zw;
    float2 p3 = baseline.zw;
    float2 p0p1 = mix(p0, p1, float2(t));
    float2 p1p2 = mix(p1, p2, float2(t));
    float2 p2p3 = mix(p2, p3, float2(t));
    float2 p0p1p2 = mix(p0p1, p1p2, float2(t));
    float2 p1p2p3 = mix(p1p2, p2p3, float2(t));
    return mix(p0p1p2, p1p2p3, float2(t));
}

static inline __attribute__((always_inline))
float2 sampleLine(thread const float4& line, thread const float& t)
{
    return mix(line.xy, line.zw, float2(t));
}

static inline __attribute__((always_inline))
void emitMicroline(thread const float4& microlineSegment, thread const uint& pathIndex, thread const uint& outputMicrolineIndex, thread int uMaxMicrolineCount, device bMicrolines& v_76)
{
    if (outputMicrolineIndex >= uint(uMaxMicrolineCount))
    {
        return;
    }
    int4 microlineSubpixels = int4(round(fast::clamp(microlineSegment, float4(-32768.0), float4(32767.0)) * 256.0));
    int4 microlinePixels = int4(floor(float4(microlineSubpixels) / float4(256.0)));
    int4 microlineFractPixels = microlineSubpixels - (microlinePixels * int4(256));
    v_76.iMicrolines[outputMicrolineIndex] = uint4((uint(microlinePixels.x) & 65535u) | (uint(microlinePixels.y) << uint(16)), (uint(microlinePixels.z) & 65535u) | (uint(microlinePixels.w) << uint(16)), ((uint(microlineFractPixels.x) | (uint(microlineFractPixels.y) << uint(8))) | (uint(microlineFractPixels.z) << uint(16))) | (uint(microlineFractPixels.w) << uint(24)), pathIndex);
}

kernel void main0(constant int& uMaxMicrolineCount [[buffer(0)]], constant int& uLastBatchSegmentIndex [[buffer(5)]], constant int& uPathCount [[buffer(6)]], constant float2x2& uTransform [[buffer(2)]], constant float2& uTranslation [[buffer(4)]], device bMicrolines& v_76 [[buffer(1)]], const device bPoints& v_194 [[buffer(3)]], const device bDiceMetadata& _253 [[buffer(7)]], const device bInputIndices& _300 [[buffer(8)]], device bComputeIndirectParams& _439 [[buffer(9)]], uint3 gl_GlobalInvocationID [[thread_position_in_grid]])
{
    uint batchSegmentIndex = gl_GlobalInvocationID.x;
    if (batchSegmentIndex >= uint(uLastBatchSegmentIndex))
    {
        return;
    }
    uint lowPathIndex = 0u;
    uint highPathIndex = uint(uPathCount);
    int iteration = 0;
    for (;;)
    {
        bool _234 = iteration < 1024;
        bool _241;
        if (_234)
        {
            _241 = (lowPathIndex + 1u) < highPathIndex;
        }
        else
        {
            _241 = _234;
        }
        if (_241)
        {
            uint midPathIndex = lowPathIndex + ((highPathIndex - lowPathIndex) / 2u);
            uint midBatchSegmentIndex = _253.iDiceMetadata[midPathIndex].z;
            if (batchSegmentIndex < midBatchSegmentIndex)
            {
                highPathIndex = midPathIndex;
            }
            else
            {
                lowPathIndex = midPathIndex;
                if (batchSegmentIndex == midBatchSegmentIndex)
                {
                    break;
                }
            }
            iteration++;
            continue;
        }
        else
        {
            break;
        }
    }
    uint batchPathIndex = lowPathIndex;
    uint4 diceMetadata = _253.iDiceMetadata[batchPathIndex];
    uint firstGlobalSegmentIndexInPath = diceMetadata.y;
    uint firstBatchSegmentIndexInPath = diceMetadata.z;
    uint globalSegmentIndex = (batchSegmentIndex - firstBatchSegmentIndexInPath) + firstGlobalSegmentIndexInPath;
    uint2 inputIndices = _300.iInputIndices[globalSegmentIndex];
    uint fromPointIndex = inputIndices.x;
    uint flagsPathIndex = inputIndices.y;
    uint toPointIndex = fromPointIndex;
    if ((flagsPathIndex & 1073741824u) != 0u)
    {
        toPointIndex += 3u;
    }
    else
    {
        if ((flagsPathIndex & 2147483648u) != 0u)
        {
            toPointIndex += 2u;
        }
        else
        {
            toPointIndex++;
        }
    }
    uint param = fromPointIndex;
    uint param_1 = toPointIndex;
    float4 baseline = float4(getPoint(param, uTransform, v_194, uTranslation), getPoint(param_1, uTransform, v_194, uTranslation));
    float4 ctrl = float4(0.0);
    bool isCurve = (flagsPathIndex & 3221225472u) != 0u;
    float segmentCountF;
    if (isCurve)
    {
        uint param_2 = fromPointIndex + 1u;
        float2 ctrl0 = getPoint(param_2, uTransform, v_194, uTranslation);
        if ((flagsPathIndex & 2147483648u) != 0u)
        {
            float2 ctrl0_2 = ctrl0 * float2(2.0);
            ctrl = (baseline + (ctrl0 * float2(2.0)).xyxy) * float4(0.3333333432674407958984375);
        }
        else
        {
            uint param_3 = fromPointIndex + 2u;
            ctrl = float4(ctrl0, getPoint(param_3, uTransform, v_194, uTranslation));
        }
        float2 bound = float2(6.0) * fast::max(abs((ctrl.zw - (ctrl.xy * 2.0)) + baseline.xy), abs((baseline.zw - (ctrl.zw * 2.0)) + ctrl.xy));
        segmentCountF = sqrt(length(bound) / 2.0);
    }
    else
    {
        segmentCountF = length(baseline.zw - baseline.xy) / 16.0;
    }
    int segmentCount = max(int(ceil(segmentCountF)), 1);
    uint _444 = atomic_fetch_add_explicit((device atomic_uint*)&_439.iComputeIndirectParams[3], uint(segmentCount), memory_order_relaxed);
    uint firstOutputMicrolineIndex = _444;
    float prevT = 0.0;
    float2 prevPoint = baseline.xy;
    float2 nextPoint;
    for (int segmentIndex = 0; segmentIndex < segmentCount; segmentIndex++)
    {
        float nextT = float(segmentIndex + 1) / float(segmentCount);
        if (isCurve)
        {
            float4 param_4 = baseline;
            float4 param_5 = ctrl;
            float param_6 = nextT;
            nextPoint = sampleCurve(param_4, param_5, param_6);
        }
        else
        {
            float4 param_7 = baseline;
            float param_8 = nextT;
            nextPoint = sampleLine(param_7, param_8);
        }
        float4 param_9 = float4(prevPoint, nextPoint);
        uint param_10 = batchPathIndex;
        uint param_11 = firstOutputMicrolineIndex + uint(segmentIndex);
        emitMicroline(param_9, param_10, param_11, uMaxMicrolineCount, v_76);
        prevT = nextT;
        prevPoint = nextPoint;
    }
}

