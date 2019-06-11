#version {{version}}











precision highp float;

in ivec2 aPosition;

out vec2 vTexCoord;

void main(){
    vTexCoord = vec2(aPosition);
    gl_Position = vec4(vec2(aPosition)* 2.0 - 1.0, 0.0, 1.0);
}

