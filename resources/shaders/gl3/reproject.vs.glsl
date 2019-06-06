#version {{version}}











precision highp float;

uniform mat4 uNewTransform;

in vec2 aPosition;

out vec2 vTexCoord;

void main(){
    vTexCoord = aPosition;
    gl_Position = uNewTransform * vec4(aPosition, 0.0, 1.0);
}

