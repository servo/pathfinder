#version 330

uniform mat2 uTransform;
uniform vec2 uTranslation;

in vec2 aPosition;
in vec2 aTexCoord;

out vec2 vTexCoord;

void main() {
    vTexCoord = aTexCoord;
    gl_Position = vec4(uTransform * aPosition + uTranslation, 0.0f, 1.0f);
}

