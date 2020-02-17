#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform sampler2D uFillTexture;

in vec2 vFillTexCoord;
in float vBackdrop;

out vec4 oFragColor;

void main(){
    float alpha = texture(uFillTexture, vFillTexCoord). r + vBackdrop;
    oFragColor = vec4(1.0 - abs(1.0 - mod(alpha, 2.0)));
}

