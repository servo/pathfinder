#version {{version}}











precision highp float;

uniform mat4 uTransform;
uniform int uGridlineCount;

in vec2 aPosition;

out vec2 vTexCoord;

void main(){
    vTexCoord = aPosition * float(uGridlineCount);
    gl_Position = uTransform * vec4(aPosition . x, 0.0, aPosition . y, 1.0);
}

