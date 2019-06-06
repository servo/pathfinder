#version {{version}}











precision highp float;

uniform vec2 uFramebufferSize;
uniform vec2 uTextureSize;

in vec2 aPosition;
in vec2 aTexCoord;

out vec2 vTexCoord;

void main(){
    vTexCoord = aTexCoord / uTextureSize;
    vec2 position = aPosition / uFramebufferSize * 2.0 - 1.0;
    gl_Position = vec4(position . x, - position . y, 0.0, 1.0);
}

