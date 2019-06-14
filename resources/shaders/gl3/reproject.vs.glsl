#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform mat4 uNewTransform;

in ivec2 aPosition;

out vec2 vTexCoord;

void main(){
    vTexCoord = vec2(aPosition);
    gl_Position = uNewTransform * vec4(ivec4(aPosition, 0, 1));
}

