#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!














#extension GL_GOOGLE_include_directive : enable






precision highp float;

uniform int uBlendMode;

out vec4 oFragColor;












uniform sampler2D uStencilTexture;
uniform sampler2D uPaintTexture;
uniform sampler2D uDest;
uniform vec2 uFramebufferSize;

in vec2 vColorTexCoord;
in vec2 vMaskTexCoord;
in float vOpacity;


vec4 sampleSrcColor(){
    float coverage = texture(uStencilTexture, vMaskTexCoord). r;
    vec4 srcRGBA = texture(uPaintTexture, vColorTexCoord);
    return vec4(srcRGBA . rgb, srcRGBA . a * coverage * vOpacity);
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


void main(){
    vec4 srcRGBA = sampleSrcColor();
    vec4 destRGBA = sampleDestColor();

    bool reversed = uBlendMode == 3;
    vec3 src = reversed ? srcRGBA . rgb : destRGBA . rgb;
    vec3 dest = reversed ? destRGBA . rgb : srcRGBA . rgb;

    vec3 multiply = src * dest;
    vec3 blended;
    if(uBlendMode == 0){
        blended = multiply;
    } else {
        vec3 screen = dest + src - multiply;
        if(uBlendMode == 1)
            blended = screen;
        else
            blended = select3(lessThanEqual(src, vec3(0.5)), multiply, screen * 2.0 - 1.0);
    }

    oFragColor = blendColors(destRGBA, srcRGBA, blended);
}

