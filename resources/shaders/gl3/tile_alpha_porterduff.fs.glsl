#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!














#extension GL_GOOGLE_include_directive : enable






precision highp float;

uniform int uDestFactor;
uniform int uSrcFactor;

out vec4 oFragColor;












uniform sampler2D uStencilTexture;
uniform sampler2D uPaintTexture;
uniform sampler2D uDest;
uniform vec2 uFramebufferSize;

in vec2 vColorTexCoord;
in vec2 vMaskTexCoord;


vec4 sampleSrcColor(){
    float coverage = texture(uStencilTexture, vMaskTexCoord). r;
    vec4 srcRGBA = texture(uPaintTexture, vColorTexCoord);
    return vec4(srcRGBA . rgb, srcRGBA . a * coverage);
}

vec4 sampleDestColor(){
    vec2 destTexCoord = gl_FragCoord . xy / uFramebufferSize;
    return texture(uDest, destTexCoord);
}


vec4 blendColors(vec4 destRGBA, vec4 srcRGBA, vec3 blendedRGB){
    return vec4(srcRGBA . a *(1.0 - destRGBA . a)* srcRGBA . rgb +
                srcRGBA . a * destRGBA . a * blendedRGB +
                (1.0 - srcRGBA . a)* destRGBA . a * destRGBA . rgb,
                1.0);
}

vec3 select3(bvec3 cond, vec3 a, vec3 b){
    return vec3(cond . x ? a . x : b . x, cond . y ? a . y : b . y, cond . z ? a . z : b . z);
}


vec4 getFactor(int factor, vec4 destRGBA, vec4 srcRGBA){
    if(factor == 0)
        return vec4(0.0);
    if(factor == 1)
        return vec4(destRGBA . a);
    if(factor == 2)
        return vec4(srcRGBA . a);
    return vec4(1.0 - destRGBA . a);
}

void main(){
    vec4 srcRGBA = sampleSrcColor();
    vec4 destRGBA = sampleDestColor();

    vec4 destFactor = getFactor(uDestFactor, destRGBA, srcRGBA);
    vec4 srcFactor = getFactor(uSrcFactor, destRGBA, srcRGBA);

    vec4 blended = destFactor * destRGBA * vec4(destRGBA . aaa, 1.0)+
        srcFactor * srcRGBA * vec4(srcRGBA . aaa, 1.0);
    oFragColor = blended;
}

