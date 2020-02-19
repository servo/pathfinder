#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform mat4 uTransform;
uniform vec2 uTileSize;
uniform vec2 uStencilTextureSize;

in ivec2 aTilePosition;
in vec2 aColorTexCoord;
in vec2 aMaskTexCoord;

out vec2 vColorTexCoord;
out vec2 vMaskTexCoord;

void main(){
    vec2 position = aTilePosition * uTileSize;

    vMaskTexCoord = aMaskTexCoord;
    vColorTexCoord = aColorTexCoord;
    gl_Position = uTransform * vec4(position, 0.0, 1.0);
}

