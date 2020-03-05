#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform sampler2D uColorTexture;

in vec2 vColorTexCoord;

out vec4 oFragColor;

void main(){
    vec4 color = texture(uColorTexture, vColorTexCoord);
    oFragColor = vec4(color . rgb * color . a, color . a);
}

