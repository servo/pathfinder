#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform mat4 uTransform;
uniform vec2 uTileSize;

in uvec2 aTessCoord;
in ivec2 aTileOrigin;
in vec4 aColorTexMatrix;
in vec2 aColorTexOffset;

out vec2 vColorTexCoord;

void main(){
    vec2 tileOffset = vec2(aTessCoord)* uTileSize;
    vec2 position = aTileOrigin * uTileSize + tileOffset;
    vColorTexCoord = mat2(aColorTexMatrix)* tileOffset + aColorTexOffset;
    gl_Position = uTransform * vec4(position, 0.0, 1.0);
}

