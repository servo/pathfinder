#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform sampler2D uMaskTexture;

in vec2 vMaskTexCoord;
in float vBackdrop;

out vec4 oFragColor;

void main(){
    oFragColor = vec4(abs(texture(uMaskTexture, vMaskTexCoord). r + vBackdrop));
}

