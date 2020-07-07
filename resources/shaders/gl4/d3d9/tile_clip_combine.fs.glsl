#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;





uniform sampler2D uSrc;

in vec2 vTexCoord0;
in float vBackdrop0;
in vec2 vTexCoord1;
in float vBackdrop1;

out vec4 oFragColor;

void main(){
    oFragColor = min(abs(texture(uSrc, vTexCoord0)+ vBackdrop0),
                     abs(texture(uSrc, vTexCoord1)+ vBackdrop1));
}

