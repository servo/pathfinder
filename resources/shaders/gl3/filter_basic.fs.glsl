#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!















#extension GL_GOOGLE_include_directive : enable

precision highp float;

uniform sampler2D uSource;

in vec2 vTexCoord;

out vec4 oFragColor;

void main(){
    vec4 color = texture(uSource, vTexCoord);
    oFragColor = vec4(color . rgb * color . a, color . a);
}

