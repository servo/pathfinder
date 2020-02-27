#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!




















#extension GL_GOOGLE_include_directive : enable

precision highp float;

uniform sampler2D uSrc;
uniform vec2 uSrcOffsetScale;
uniform vec3 uInitialGaussCoeff;
uniform int uSupport;

in vec2 vTexCoord;

out vec4 oFragColor;

void main(){

    vec3 gaussCoeff = uInitialGaussCoeff;
    float gaussSum = gaussCoeff . x;
    vec4 color = texture(uSrc, vTexCoord)* gaussCoeff . x;
    gaussCoeff . xy *= gaussCoeff . yz;









    for(int i = 1;i <= uSupport;i += 2){
        float gaussPartialSum = gaussCoeff . x;
        gaussCoeff . xy *= gaussCoeff . yz;
        gaussPartialSum += gaussCoeff . x;

        vec2 srcOffset = uSrcOffsetScale *(float(i)+ gaussCoeff . x / gaussPartialSum);
        color +=(texture(uSrc, vTexCoord - srcOffset)+ texture(uSrc, vTexCoord + srcOffset))*
            gaussPartialSum;

        gaussSum += 2.0 * gaussPartialSum;
        gaussCoeff . xy *= gaussCoeff . yz;
    }


    color /= gaussSum;
    color . rgb *= color . a;
    oFragColor = color;
}

