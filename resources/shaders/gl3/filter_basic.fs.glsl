#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!















#extension GL_GOOGLE_include_directive : enable

precision highp float;

uniform sampler2D uSource;
uniform vec2 uSourceSize;

in vec2 vTexCoord;

out vec4 oFragColor;

void main(){
    oFragColor = texture(uSource, vTexCoord);
}

