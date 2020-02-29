#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform mat4 uTransform;
uniform vec2 uTileSize;

in ivec2 aTilePosition;
in vec2 aColorTexCoord;
in vec2 aMaskTexCoord;
in float aOpacity;

out vec2 vColorTexCoord;
out vec2 vMaskTexCoord;
out float vOpacity;

void main(){
    vec2 position = vec2(aTilePosition)* uTileSize;

    vMaskTexCoord = aMaskTexCoord;
    vColorTexCoord = aColorTexCoord;
    vOpacity = aOpacity;
    gl_Position = uTransform * vec4(position, 0.0, 1.0);
}

