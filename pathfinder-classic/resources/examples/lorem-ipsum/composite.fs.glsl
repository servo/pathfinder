#version 330

uniform sampler2DRect uAtlas;
uniform vec3 uForegroundColor;
uniform vec3 uBackgroundColor;

in vec2 vTexCoord;

out vec4 oFragColor;

void main() {
    vec3 value = texture(uAtlas, vTexCoord).rgb;
    vec3 color = mix(uBackgroundColor, uForegroundColor, value);
    oFragColor = vec4(color, 1.0f);
}

