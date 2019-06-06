#version {{version}}











precision highp float;

uniform vec2 uFramebufferSize;

in vec2 aPosition;

void main(){
    vec2 position = aPosition / uFramebufferSize * 2.0 - 1.0;
    gl_Position = vec4(position . x, - position . y, 0.0, 1.0);
}

