#version {{version}}











precision highp float;

uniform vec2 uFramebufferSize;
uniform vec2 uTileSize;

in ivec2 aTessCoord;
in uint aFromPx;
in uint aToPx;
in vec2 aFromSubpx;
in vec2 aToSubpx;
in uint aTileIndex;

out vec2 vFrom;
out vec2 vTo;

vec2 computeTileOffset(uint tileIndex, float stencilTextureWidth){
    uint tilesPerRow = uint(stencilTextureWidth / uTileSize . x);
    uvec2 tileOffset = uvec2(aTileIndex % tilesPerRow, aTileIndex / tilesPerRow);
    return vec2(tileOffset)* uTileSize;
}

void main(){
    vec2 tileOrigin = computeTileOffset(aTileIndex, uFramebufferSize . x);

    vec2 from = vec2(aFromPx & 15u, aFromPx >> 4u)+ aFromSubpx;
    vec2 to = vec2(aToPx & 15u, aToPx >> 4u)+ aToSubpx;

    vec2 position;
    if(aTessCoord . x == 0)
        position . x = floor(min(from . x, to . x));
    else
        position . x = ceil(max(from . x, to . x));
    if(aTessCoord . y == 0)
        position . y = floor(min(from . y, to . y));
    else
        position . y = uTileSize . y;

    vFrom = from - position;
    vTo = to - position;

    gl_Position = vec4((tileOrigin + position)/ uFramebufferSize * 2.0 - 1.0, 0.0, 1.0);
}

