#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;





uniform sampler2D uSrc;

in vec2 vTexCoord;
in float vBackdrop;

out vec4 oFragColor;

void main(){
    oFragColor = clamp(abs(texture(uSrc, vTexCoord)+ vBackdrop), 0.0, 1.0);
}

