#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;





uniform vec2 uFramebufferSize;
uniform sampler2D uSrc;

out vec4 oFragColor;

void main(){
    vec2 texCoord = gl_FragCoord . xy / uFramebufferSize;
    oFragColor = texture(uSrc, texCoord);
}

