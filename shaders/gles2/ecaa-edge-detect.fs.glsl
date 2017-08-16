// pathfinder/shaders/gles2/ecaa-edge-detect.fs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

uniform ivec2 uFramebufferSize;
uniform sampler2D uColor;
uniform sampler2D uDepth;

varying vec2 vTexCoord;

vec3 checkFG(vec3 fgPosition, vec3 queryPosition) {
    return queryPosition.z > fgPosition.z ? queryPosition : fgPosition;
}

void main() {
    // Unpack.
    vec2 position = vTexCoord;

    // Compute positions.
    vec2 onePixel = 1.0 / vec2(uFramebufferSize);
    vec2 positionL = position + vec2(-onePixel.x, 0.0);
    vec2 positionR = position + vec2( onePixel.x, 0.0);
    vec2 positionB = position + vec2(0.0, -onePixel.x);
    vec2 positionT = position + vec2(0.0,  onePixel.x);

    // Determine the topmost path.
    float centerDepth = texture2D(uDepth, position).r;
    vec4 neighborDepths = vec4(texture2D(uDepth, positionL).r,
                               texture2D(uDepth, positionR).r,
                               texture2D(uDepth, positionT).r,
                               texture2D(uDepth, positionB).r);

    // Determine the position of the foreground color.
    vec3 fgPosition = vec3(position, centerDepth);
    fgPosition = checkFG(fgPosition, vec3(positionL, neighborDepths.x));
    fgPosition = checkFG(fgPosition, vec3(positionR, neighborDepths.y));
    fgPosition = checkFG(fgPosition, vec3(positionT, neighborDepths.z));
    fgPosition = checkFG(fgPosition, vec3(positionB, neighborDepths.w));

    // Determine the position of the background color.
    vec2 bgPosition;
    if (fgPosition.z != centerDepth)
        bgPosition = fgPosition.xy;
    else if (fgPosition.z != neighborDepths.x)
        bgPosition = positionL;
    else if (fgPosition.z != neighborDepths.y)
        bgPosition = positionR;
    else if (fgPosition.z != neighborDepths.z)
        bgPosition = positionT;
    else
        bgPosition = positionB;

    // Determine the foreground and background colors.
    vec4 fgColor = texture2D(uColor, fgPosition.st);
    vec4 bgColor = texture2D(uColor, bgPosition);

    // Determine the depth.
    //
    // If all colors are the same, avoid touching this pixel in any further passes.
    float outDepth = fgColor == bgColor ? 0.0 : fgPosition.z;

    // Output results.
    gl_FragData[0] = fgColor;
    gl_FragData[1] = bgColor;
    gl_FragDepthEXT = outDepth;
}
