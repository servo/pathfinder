#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;





uniform vec4 uRect;
uniform vec2 uFramebufferSize;

in ivec2 aPosition;

void main(){
    vec2 position = mix(uRect . xy, uRect . zw, vec2(aPosition))/ uFramebufferSize * 2.0 - 1.0;
    gl_Position = vec4(position . x, - position . y, 0.0, 1.0);
}

