#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform vec2 uTileSize;
uniform vec2 uStencilTextureSize;
uniform sampler2D uStencilTexture;

in vec2 vTexCoord;
in float vBackdrop;
in vec4 vColor;

out vec4 oFragColor;

float avoidTileEdges(float edgeCoord, float texCoord, float tileSize){
    if(edgeCoord < 0.5)
        return texCoord - edgeCoord + 0.5;
    else if(edgeCoord > tileSize - 0.5)
        return texCoord - edgeCoord + 15.5;
    return texCoord;
}

void main(){
    vec2 texCoord = vTexCoord;
    vec2 edgeCoord = mod(texCoord, uTileSize);
    texCoord . x = avoidTileEdges(edgeCoord . x, texCoord . x, uTileSize . x);
    texCoord . y = avoidTileEdges(edgeCoord . y, texCoord . y, uTileSize . y);
    texCoord /= uStencilTextureSize;

    float coverage = abs(texture(uStencilTexture, texCoord). r + vBackdrop);
    oFragColor = vec4(vColor . rgb, vColor . a * coverage);
}

