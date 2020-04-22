#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;
precision highp sampler2D;

uniform sampler2D uSrc;

in vec2 vTexCoord;
in float vBackdrop;

out vec4 oFragColor;

void main(){
    float alpha = clamp(abs(texture(uSrc, vTexCoord). r + vBackdrop), 0.0, 1.0);
    oFragColor = vec4(alpha, 0.0, 0.0, 1.0);
}

