#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform sampler2D uFillTexture;

in vec2 vFillTexCoord;
in float vBackdrop;

out vec4 oFragColor;

void main(){
    oFragColor = vec4(abs(texture(uFillTexture, vFillTexCoord). r + vBackdrop));
}

