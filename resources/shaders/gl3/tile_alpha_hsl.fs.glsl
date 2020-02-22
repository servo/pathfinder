#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












#extension GL_GOOGLE_include_directive : enable




precision highp float;

uniform sampler2D uStencilTexture;
uniform sampler2D uPaintTexture;
uniform sampler2D uDest;
uniform ivec3 uBlendHSL;
uniform vec2 uFramebufferSize;

in vec2 vColorTexCoord;
in vec2 vMaskTexCoord;

out vec4 oFragColor;






vec3 convertHSLToRGB(vec3 hsl){
    float a = hsl . y * min(hsl . z, 1.0 - hsl . z);
    vec3 ks = mod(vec3(0.0, 8.0, 4.0)+ vec3(hsl . x * 1.9098593171027443), 12.0);
    return hsl . zzz - clamp(min(ks - vec3(3.0), vec3(9.0)- ks), - 1.0, 1.0)* a;
}


vec3 convertRGBToHSL(vec3 rgb){
    float v = max((rgb . x, rgb . y), rgb . z);
    float c = v - min((rgb . x, rgb . y), rgb . z);
    float l = v - 0.5 * c;

    vec3 tmp = vec3(0.0);
    bvec3 is_v = equal(rgb, vec3(v));
    if(is_v . r)
        tmp = vec3(0.0, rgb . gb);
    else if(is_v . g)
        tmp = vec3(2.0, rgb . br);
    else if(is_v . b)
        tmp = vec3(4.0, rgb . rg);
    float h = 1.0471975511965976 *(tmp . x +(tmp . y - tmp . z)/ c);

    float s = 0.0;
    if(l > 0.0 && l < 1.0)
        s =(v - l)/ min(l, 1.0 - l);

    return vec3(h, s, l);
}

void main(){
    float coverage = texture(uStencilTexture, vMaskTexCoord). r;
    vec4 srcRGBA = texture(uPaintTexture, vColorTexCoord);
    srcRGBA . a *= coverage;

    vec2 destTexCoord = gl_FragCoord . xy / uFramebufferSize;
    vec4 destRGBA = texture(uDest, destTexCoord);

    vec3 destHSL = convertRGBToHSL(destRGBA . rgb);
    vec3 srcHSL = convertRGBToHSL(srcRGBA . rgb);
    bvec3 blendDest = equal(uBlendHSL, ivec3(0));
    vec3 blendedHSL = vec3(blendDest . x ? destHSL . x : srcHSL . x,
                           blendDest . y ? destHSL . y : srcHSL . y,
                           blendDest . z ? destHSL . z : srcHSL . z);
    vec3 blendedRGB = convertHSLToRGB(blendedHSL);


    vec4 color = vec4(srcRGBA . a *(1.0 - destRGBA . a)* srcRGBA . rgb +
                      srcRGBA . a * destRGBA . a * blendedRGB +
                      (1.0 - srcRGBA . a)* destRGBA . a * destRGBA . rgb,
                      1.0);
    oFragColor = color;
}

