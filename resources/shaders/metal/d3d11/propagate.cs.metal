// Automatically generated from files in pathfinder/shaders/. Do not edit!
#pragma clang diagnostic ignored "-Wmissing-prototypes"
#pragma clang diagnostic ignored "-Wunused-variable"

#include <metal_stdlib>
#include <simd/simd.h>
#include <metal_atomic>

using namespace metal;

struct bBackdrops
{
    int iBackdrops[1];
};

struct bDrawMetadata
{
    uint4 iDrawMetadata[1];
};

struct bClipMetadata
{
    uint4 iClipMetadata[1];
};

struct bDrawTiles
{
    uint iDrawTiles[1];
};

struct bClipTiles
{
    uint iClipTiles[1];
};

struct bZBuffer
{
    int iZBuffer[1];
};

struct bAlphaTiles
{
    uint iAlphaTiles[1];
};

struct bFirstTileMap
{
    int iFirstTileMap[1];
};

constant uint3 gl_WorkGroupSize [[maybe_unused]] = uint3(64u, 1u, 1u);

static inline __attribute__((always_inline))
uint calculateTileIndex(thread const uint& bufferOffset, thread const uint4& tileRect, thread const uint2& tileCoord)
{
    return (bufferOffset + (tileCoord.y * (tileRect.z - tileRect.x))) + tileCoord.x;
}

kernel void main0(constant int& uColumnCount [[buffer(0)]], constant int& uFirstAlphaTileIndex [[buffer(8)]], constant int2& uFramebufferTileSize [[buffer(9)]], const device bBackdrops& _59 [[buffer(1)]], const device bDrawMetadata& _85 [[buffer(2)]], const device bClipMetadata& _126 [[buffer(3)]], device bDrawTiles& _175 [[buffer(4)]], device bClipTiles& _252 [[buffer(5)]], device bZBuffer& _302 [[buffer(6)]], device bAlphaTiles& _310 [[buffer(7)]], device bFirstTileMap& _395 [[buffer(10)]], uint3 gl_GlobalInvocationID [[thread_position_in_grid]])
{
    uint columnIndex = gl_GlobalInvocationID.x;
    if (int(columnIndex) >= uColumnCount)
    {
        return;
    }
    int currentBackdrop = _59.iBackdrops[(columnIndex * 3u) + 0u];
    int tileX = _59.iBackdrops[(columnIndex * 3u) + 1u];
    uint drawPathIndex = uint(_59.iBackdrops[(columnIndex * 3u) + 2u]);
    uint4 drawTileRect = _85.iDrawMetadata[(drawPathIndex * 3u) + 0u];
    uint4 drawOffsets = _85.iDrawMetadata[(drawPathIndex * 3u) + 1u];
    uint2 drawTileSize = drawTileRect.zw - drawTileRect.xy;
    uint drawTileBufferOffset = drawOffsets.x;
    bool zWrite = drawOffsets.z != 0u;
    int clipPathIndex = int(drawOffsets.w);
    uint4 clipTileRect = uint4(0u);
    uint4 clipOffsets = uint4(0u);
    if (clipPathIndex >= 0)
    {
        clipTileRect = _126.iClipMetadata[(clipPathIndex * 2) + 0];
        clipOffsets = _126.iClipMetadata[(clipPathIndex * 2) + 1];
    }
    uint clipTileBufferOffset = clipOffsets.x;
    uint clipBackdropOffset = clipOffsets.y;
    for (uint tileY = 0u; tileY < drawTileSize.y; tileY++)
    {
        uint2 drawTileCoord = uint2(uint(tileX), tileY);
        uint param = drawTileBufferOffset;
        uint4 param_1 = drawTileRect;
        uint2 param_2 = drawTileCoord;
        uint drawTileIndex = calculateTileIndex(param, param_1, param_2);
        int drawAlphaTileIndex = -1;
        int clipAlphaTileIndex = -1;
        int drawFirstFillIndex = int(_175.iDrawTiles[(drawTileIndex * 4u) + 1u]);
        int drawBackdropDelta = int(_175.iDrawTiles[(drawTileIndex * 4u) + 2u]) >> 24;
        uint drawTileWord = _175.iDrawTiles[(drawTileIndex * 4u) + 3u] & 16777215u;
        int drawTileBackdrop = currentBackdrop;
        bool haveDrawAlphaMask = drawFirstFillIndex >= 0;
        bool needNewAlphaTile = haveDrawAlphaMask;
        if (clipPathIndex >= 0)
        {
            uint2 tileCoord = drawTileCoord + drawTileRect.xy;
            if (all(bool4(tileCoord >= clipTileRect.xy, tileCoord < clipTileRect.zw)))
            {
                uint2 clipTileCoord = tileCoord - clipTileRect.xy;
                uint param_3 = clipTileBufferOffset;
                uint4 param_4 = clipTileRect;
                uint2 param_5 = clipTileCoord;
                uint clipTileIndex = calculateTileIndex(param_3, param_4, param_5);
                int thisClipAlphaTileIndex = int(_252.iClipTiles[(clipTileIndex * 4u) + 2u] << uint(8)) >> 8;
                uint clipTileWord = _252.iClipTiles[(clipTileIndex * 4u) + 3u];
                int clipTileBackdrop = int(clipTileWord) >> 24;
                if (thisClipAlphaTileIndex >= 0)
                {
                    if (haveDrawAlphaMask)
                    {
                        clipAlphaTileIndex = thisClipAlphaTileIndex;
                        needNewAlphaTile = true;
                    }
                    else
                    {
                        if (drawTileBackdrop != 0)
                        {
                            drawAlphaTileIndex = thisClipAlphaTileIndex;
                            clipAlphaTileIndex = -1;
                            needNewAlphaTile = false;
                        }
                        else
                        {
                            drawAlphaTileIndex = -1;
                            clipAlphaTileIndex = -1;
                            needNewAlphaTile = false;
                        }
                    }
                }
                else
                {
                    if (clipTileBackdrop == 0)
                    {
                        drawTileBackdrop = 0;
                        needNewAlphaTile = false;
                    }
                }
            }
            else
            {
                drawTileBackdrop = 0;
                needNewAlphaTile = false;
            }
        }
        if (needNewAlphaTile)
        {
            int _305 = atomic_fetch_add_explicit((device atomic_int*)&_302.iZBuffer[4], 1, memory_order_relaxed);
            uint drawBatchAlphaTileIndex = uint(_305);
            _310.iAlphaTiles[(drawBatchAlphaTileIndex * 2u) + 0u] = drawTileIndex;
            _310.iAlphaTiles[(drawBatchAlphaTileIndex * 2u) + 1u] = uint(clipAlphaTileIndex);
            drawAlphaTileIndex = int(drawBatchAlphaTileIndex) + uFirstAlphaTileIndex;
        }
        _175.iDrawTiles[(drawTileIndex * 4u) + 2u] = (uint(drawAlphaTileIndex) & 16777215u) | (uint(drawBackdropDelta) << uint(24));
        _175.iDrawTiles[(drawTileIndex * 4u) + 3u] = drawTileWord | (uint(drawTileBackdrop) << uint(24));
        int2 tileCoord_1 = int2(tileX, int(tileY)) + int2(drawTileRect.xy);
        int tileMapIndex = (tileCoord_1.y * uFramebufferTileSize.x) + tileCoord_1.x;
        if ((zWrite && (drawTileBackdrop != 0)) && (drawAlphaTileIndex < 0))
        {
            int _383 = atomic_fetch_max_explicit((device atomic_int*)&_302.iZBuffer[tileMapIndex + 8], int(drawTileIndex), memory_order_relaxed);
        }
        if ((drawTileBackdrop != 0) || (drawAlphaTileIndex >= 0))
        {
            int _400 = atomic_exchange_explicit((device atomic_int*)&_395.iFirstTileMap[tileMapIndex], int(drawTileIndex), memory_order_relaxed);
            int nextTileIndex = _400;
            _175.iDrawTiles[(drawTileIndex * 4u) + 0u] = uint(nextTileIndex);
        }
        currentBackdrop += drawBackdropDelta;
    }
}

