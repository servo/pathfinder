// Automatically generated from files in pathfinder/shaders/. Do not edit!
#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct bTilePathInfo
{
    uint4 iTilePathInfo[1];
};

struct bTiles
{
    uint iTiles[1];
};

constant uint3 gl_WorkGroupSize [[maybe_unused]] = uint3(64u, 1u, 1u);

kernel void main0(constant int& uTileCount [[buffer(0)]], constant int& uPathCount [[buffer(1)]], const device bTilePathInfo& _64 [[buffer(2)]], device bTiles& _148 [[buffer(3)]], uint3 gl_GlobalInvocationID [[thread_position_in_grid]])
{
    uint tileIndex = gl_GlobalInvocationID.x;
    if (tileIndex >= uint(uTileCount))
    {
        return;
    }
    uint lowPathIndex = 0u;
    uint highPathIndex = uint(uPathCount);
    int iteration = 0;
    for (;;)
    {
        bool _42 = iteration < 1024;
        bool _50;
        if (_42)
        {
            _50 = (lowPathIndex + 1u) < highPathIndex;
        }
        else
        {
            _50 = _42;
        }
        if (_50)
        {
            uint midPathIndex = lowPathIndex + ((highPathIndex - lowPathIndex) / 2u);
            uint midTileIndex = _64.iTilePathInfo[midPathIndex].z;
            if (tileIndex < midTileIndex)
            {
                highPathIndex = midPathIndex;
            }
            else
            {
                lowPathIndex = midPathIndex;
                if (tileIndex == midTileIndex)
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
    uint pathIndex = lowPathIndex;
    uint4 pathInfo = _64.iTilePathInfo[pathIndex];
    int2 packedTileRect = int2(pathInfo.xy);
    int4 tileRect = int4((packedTileRect.x << 16) >> 16, packedTileRect.x >> 16, (packedTileRect.y << 16) >> 16, packedTileRect.y >> 16);
    uint tileOffset = tileIndex - pathInfo.z;
    uint tileWidth = uint(tileRect.z - tileRect.x);
    int2 tileCoords = tileRect.xy + int2(int(tileOffset % tileWidth), int(tileOffset / tileWidth));
    _148.iTiles[(tileIndex * 4u) + 0u] = 4294967295u;
    _148.iTiles[(tileIndex * 4u) + 1u] = 4294967295u;
    _148.iTiles[(tileIndex * 4u) + 2u] = 16777215u;
    _148.iTiles[(tileIndex * 4u) + 3u] = pathInfo.w;
}

