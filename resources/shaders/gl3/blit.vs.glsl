#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

in vec2 aPosition;

out vec2 vTexCoord;

void main(){
    vTexCoord = aPosition;
    gl_Position = vec4(mix(aPosition, vec2(- 1.0), vec2(1.0)), 0.0, 1.0);
}

