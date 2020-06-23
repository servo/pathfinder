#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;





uniform vec2 uFramebufferSize;

in ivec2 aTileOffset;
in int aTileIndex;

out vec2 vTexCoord;

void main(){
    vec2 position = vec2(ivec2(aTileIndex % 256, aTileIndex / 256)+ aTileOffset);
    position *= vec2(16.0, 4.0)/ uFramebufferSize;

    vTexCoord = position;

    if(aTileIndex < 0)
        position = vec2(0.0);




    gl_Position = vec4(mix(vec2(- 1.0), vec2(1.0), position), 0.0, 1.0);
}

