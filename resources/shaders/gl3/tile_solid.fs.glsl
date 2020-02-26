#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform sampler2D uPaintTexture;

in vec2 vColorTexCoord;

out vec4 oFragColor;

void main(){
    vec4 color = texture(uPaintTexture, vColorTexCoord);
    oFragColor = vec4(color . rgb * color . a, color . a);
}

