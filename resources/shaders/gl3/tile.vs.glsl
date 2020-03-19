#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform mat4 uTransform;
uniform vec2 uTileSize;

in ivec2 aTilePosition;
in vec2 aColorTexCoord0;
in vec2 aColorTexCoord1;
in vec2 aMaskTexCoord0;
in vec2 aMaskTexCoord1;
in ivec2 aMaskBackdrop;

out vec3 vMaskTexCoord0;
out vec3 vMaskTexCoord1;
out vec2 vColorTexCoord0;
out vec2 vColorTexCoord1;

void main(){
    vec2 position = vec2(aTilePosition)* uTileSize;
    vColorTexCoord0 = aColorTexCoord0;
    vColorTexCoord1 = aColorTexCoord1;
    vMaskTexCoord0 = vec3(aMaskTexCoord0, float(aMaskBackdrop . x));
    vMaskTexCoord1 = vec3(aMaskTexCoord1, float(aMaskBackdrop . y));
    gl_Position = uTransform * vec4(position, 0.0, 1.0);
}

