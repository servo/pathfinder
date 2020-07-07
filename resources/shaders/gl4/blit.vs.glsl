#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;





uniform vec4 uDestRect;
uniform vec2 uFramebufferSize;

in ivec2 aPosition;

out vec2 vTexCoord;

void main(){
    vec2 position = mix(uDestRect . xy, uDestRect . zw, vec2(aPosition))/ uFramebufferSize;
    vec2 texCoord = vec2(aPosition);
    vTexCoord = texCoord;
    gl_Position = vec4(mix(vec2(- 1.0), vec2(1.0), position), 0.0, 1.0);
}

