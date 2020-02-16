#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

in vec2 aPosition;
in vec2 aMaskTexCoord;
in int aBackdrop;

out vec2 vMaskTexCoord;
out float vBackdrop;

void main(){
    vec2 position = mix(vec2(- 1.0), vec2(1.0), aPosition);




    vMaskTexCoord = aMaskTexCoord;
    vBackdrop = float(aBackdrop);
    gl_Position = vec4(position, 0.0, 1.0);
}

