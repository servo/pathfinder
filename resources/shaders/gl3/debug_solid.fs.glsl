#version {{version}}











precision highp float;

uniform vec4 uColor;

out vec4 oFragColor;

void main(){
    oFragColor = vec4(uColor . rgb, 1.0)* uColor . a;
}

