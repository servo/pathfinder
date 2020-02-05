#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform mat4 uTransform;
uniform vec2 uTileSize;

in uvec2 aTessCoord;
in ivec2 aTileOrigin;
in vec2 aColorTexCoord;

out vec2 vColorTexCoord;

void main(){
    vec2 position = vec2(aTileOrigin + ivec2(aTessCoord))* uTileSize;
    vColorTexCoord = aColorTexCoord;
    gl_Position = uTransform * vec4(position, 0.0, 1.0);
}

