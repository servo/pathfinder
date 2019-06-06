#version {{version}}











#extension GL_GOOGLE_include_directive : enable

precision highp float;












uniform vec2 uFramebufferSize;
uniform vec2 uTileSize;
uniform vec2 uStencilTextureSize;
uniform vec2 uViewBoxOrigin;

in vec2 aTessCoord;
in uvec3 aTileOrigin;
in int aBackdrop;
in uint aTileIndex;

out vec2 vTexCoord;
out float vBackdrop;
out vec4 vColor;

vec4 getColor();

vec2 computeTileOffset(uint tileIndex, float stencilTextureWidth){
    uint tilesPerRow = uint(stencilTextureWidth / uTileSize . x);
    uvec2 tileOffset = uvec2(tileIndex % tilesPerRow, tileIndex / tilesPerRow);
    return vec2(tileOffset)* uTileSize;
}

void computeVaryings(){
    vec2 origin = vec2(aTileOrigin . xy)+ vec2(aTileOrigin . z & 15u, aTileOrigin . z >> 4u)* 256.0;
    vec2 pixelPosition =(origin + aTessCoord)* uTileSize + uViewBoxOrigin;
    vec2 position =(pixelPosition / uFramebufferSize * 2.0 - 1.0)* vec2(1.0, - 1.0);
    vec2 maskTexCoordOrigin = computeTileOffset(aTileIndex, uStencilTextureSize . x);
    vec2 maskTexCoord = maskTexCoordOrigin + aTessCoord * uTileSize;

    vTexCoord = maskTexCoord / uStencilTextureSize;
    vBackdrop = float(aBackdrop);
    vColor = getColor();
    gl_Position = vec4(position, 0.0, 1.0);
}













uniform sampler2D uPaintTexture;
uniform vec2 uPaintTextureSize;

in vec2 aColorTexCoord;

vec4 getColor(){
    return texture(uPaintTexture, aColorTexCoord);
}


void main(){
    computeVaryings();
}

