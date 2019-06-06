#version {{version}}











precision highp float;

in vec2 aPosition;

out vec2 vTexCoord;

void main(){
    vTexCoord = aPosition;
    gl_Position = vec4(aPosition * 2.0 - 1.0, 0.0, 1.0);
}

