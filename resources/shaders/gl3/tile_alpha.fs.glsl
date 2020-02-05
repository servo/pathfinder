#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform sampler2D uStencilTexture;
uniform sampler2D uPaintTexture;
uniform vec2 uPaintTextureSize;

in vec2 vMaskTexCoord;
in vec2 vColorTexCoord;
in float vBackdrop;
in vec4 vColor;

out vec4 oFragColor;

void main(){
    float coverage = abs(texture(uStencilTexture, vMaskTexCoord). r + vBackdrop);
    vec4 color = texture(uPaintTexture, vColorTexCoord);
    oFragColor = vec4(color . rgb, color . a * coverage);
}

