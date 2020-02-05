#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform sampler2D uPaintTexture;
uniform vec2 uPaintTextureSize;

in vec2 vColorTexCoord;

out vec4 oFragColor;

void main(){
    oFragColor = texture(uPaintTexture, vColorTexCoord);
}

