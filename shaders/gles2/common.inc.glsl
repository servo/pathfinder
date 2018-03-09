// pathfinder/shaders/gles2/common.inc.glsl
//
// Copyright (c) 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#version 100

#extension GL_EXT_frag_depth : require
#extension GL_OES_standard_derivatives : require

#define FREETYPE_LCD_FILTER_FACTOR_0    0.337254902
#define FREETYPE_LCD_FILTER_FACTOR_1    0.301960784
#define FREETYPE_LCD_FILTER_FACTOR_2    0.031372549

// These intentionally do not precisely match what Core Graphics does (a Lanczos function), because
// we don't want any ringing artefacts.
#define CG_LCD_FILTER_FACTOR_0          0.286651906
#define CG_LCD_FILTER_FACTOR_1          0.221434336
#define CG_LCD_FILTER_FACTOR_2          0.102074051
#define CG_LCD_FILTER_FACTOR_3          0.033165660

#define MAX_PATHS   65536

#define EPSILON     0.001

precision highp float;

/// Returns true if the given number is close to zero.
bool isNearZero(float x) {
    return abs(x) < EPSILON;
}

/// Computes `ia % ib`.
///
/// This function is not in OpenGL ES 2 but can be polyfilled.
/// See: https://stackoverflow.com/a/36078859
int imod(int ia, int ib) {
    float a = float(ia), b = float(ib);
    float m = a - floor((a + 0.5) / b) * b;
    return int(floor(m + 0.5));
}

float fastSign(float x) {
    return x > 0.0 ? 1.0 : -1.0;
}

float det2(mat2 m) {
    return m[0][0] * m[1][1] - m[0][1] * m[1][0];
}

/// Returns the *2D* result of transforming the given 2D point with the given 4D transformation
/// matrix.
///
/// The z and w coordinates are treated as 0.0 and 1.0, respectively.
vec2 transformVertexPosition(vec2 position, mat4 transform) {
    return (transform * vec4(position, 0.0, 1.0)).xy;
}

/// Returns the 2D result of transforming the given 2D position by the given ST-transform.
///
/// An ST-transform is a combined 2D scale and translation, where the (x, y) coordinates specify
/// the scale and and the (z, w) coordinates specify the translation.
vec2 transformVertexPositionST(vec2 position, vec4 transformST) {
    return position * transformST.xy + transformST.zw;
}

vec2 transformVertexPositionAffine(vec2 position, vec4 transformST, vec2 transformExt) {
    return position * transformST.xy + position.yx * transformExt + transformST.zw;
}

vec2 transformVertexPositionInverseLinear(vec2 position, mat2 transform) {
    position = vec2(det2(mat2(position, transform[1])), det2(mat2(transform[0], position)));
    return position / det2(transform);
}

/// Interpolates the given 2D position in the vertical direction using the given ultra-slight
/// hints.
///
/// Similar in spirit to the `IUP[y]` TrueType command, but minimal.
///
///     pathHints.x: xHeight
///     pathHints.y: hintedXHeight
///     pathHints.z: stemHeight
///     pathHints.w: hintedStemHeight
///
/// TODO(pcwalton): Do something smarter with overshoots and the blue zone.
/// TODO(pcwalton): Support interpolating relative to arbitrary horizontal stems, not just the
/// baseline, x-height, and stem height.
vec2 hintPosition(vec2 position, vec4 pathHints) {
    float y;
    if (position.y >= pathHints.z) {
        y = position.y - pathHints.z + pathHints.w;
    } else if (position.y >= pathHints.x) {
        float t = (position.y - pathHints.x) / (pathHints.z - pathHints.x);
        y = mix(pathHints.y, pathHints.w, t);
    } else if (position.y >= 0.0) {
        y = mix(0.0, pathHints.y, position.y / pathHints.x);
    } else {
        y = position.y;
    }

    return vec2(position.x, y);
}

vec2 quantize(vec2 position) {
    return (floor(position * 20000.0 + 0.5) - 0.5) / 20000.0;
}

/// Converts the given 2D position in clip space to device pixel space (with origin in the lower
/// left).
vec2 convertClipToScreenSpace(vec2 position, ivec2 framebufferSize) {
    return (position + 1.0) * 0.5 * vec2(framebufferSize);
}

/// Converts the given 2D position in device pixel space (with origin in the lower left) to clip
/// space.
vec2 convertScreenToClipSpace(vec2 position, ivec2 framebufferSize) {
    return position / vec2(framebufferSize) * 2.0 - 1.0;
}

/// Packs the given path ID into a floating point value suitable for storage in the depth buffer.
/// This function returns values in clip space (i.e. what `gl_Position` is in).
float convertPathIndexToViewportDepthValue(int pathIndex) {
    return float(pathIndex) / float(MAX_PATHS) * 2.0 - 1.0;
}

/// Displaces the given point by the given distance in the direction of the normal angle.
vec2 dilatePosition(vec2 position, float normalAngle, vec2 amount) {
    return position + vec2(cos(normalAngle), -sin(normalAngle)) * amount;
}

/// Returns true if the slope of the line along the given vector is negative.
bool slopeIsNegative(vec2 dp) {
    return dp.y < 0.0;
}

/// Uses Liang-Barsky to clip the line to the left and right of the pixel square.
///
/// Returns vec4(P0', dP').
vec4 clipLineToPixelColumn(vec2 p0, vec2 dP, float pixelCenterX) {
    vec2 pixelColumnBounds = vec2(-0.5, 0.5) + pixelCenterX;
    vec2 qX = pixelColumnBounds - p0.xx;
    vec2 tX = clamp(qX / dP.xx, 0.0, 1.0);
    return vec4(p0 + dP * tX.x, dP * (tX.y - tX.x));
}

/// Uses Liang-Barsky to clip the line to the top and bottom of the pixel square.
///
/// Returns vec4(P0'', dP''). In the case of horizontal lines, this can yield -Infinity or
/// Infinity.
vec4 clipLineToPixelRow(vec2 p0, vec2 dP, float pixelCenterY, out float outPixelTop) {
    vec2 pixelRowBounds = vec2(-0.5, 0.5) + pixelCenterY;
    outPixelTop = pixelRowBounds.y;
    vec2 qY = pixelRowBounds - p0.yy;
    vec2 tY = clamp((slopeIsNegative(dP) ? qY.yx : qY.xy) / dP.yy, 0.0, 1.0);
    return vec4(p0 + dP * tY.x, dP * (tY.y - tY.x));
}

/// Computes the area of the polygon covering the pixel with the given boundaries.
///
/// The line must run left-to-right and must already be clipped to the left and right sides of the
/// pixel, which implies that `dP.x` must be within the range [0.0, 1.0].
///
/// * `p0X` is the start point of the line.
/// * `dPX` is the vector from the start point to the endpoint of the line.
/// * `pixelCenterY` is the Y coordinate of the center of the pixel in window coordinates (i.e.
///   `gl_FragCoord.y`).
/// * `winding` is the winding number (1 or -1).
float computeCoverage(vec2 p0X, vec2 dPX, float pixelCenterY, float winding) {
    // Clip to the pixel row.
    float pixelTop;
    vec4 p0DPY = clipLineToPixelRow(p0X, dPX, pixelCenterY, pixelTop);
    vec2 p0 = p0DPY.xy, dP = p0DPY.zw;
    vec2 p1 = p0 + dP;

    // If the line doesn't pass through this pixel, detect that and bail.
    //
    // This should be worth a branch because it's very common for fragment blocks to all hit this
    // path.
    //
    // The variable is required to work around a bug in the macOS Nvidia drivers.
    // Without moving the condition in a variable, the early return is ignored. See #51.
    bool lineDoesNotPassThroughPixel = isNearZero(dP.x) && isNearZero(dP.y);
    if (lineDoesNotPassThroughPixel)
        return p0X.y < pixelTop ? winding * dPX.x : 0.0;

    // Calculate points A0-A2.
    float a2x;
    vec2 a0, a1;
    if (slopeIsNegative(dP)) {
        a2x = p0X.x + dPX.x;
        a0 = p0;
        a1 = p1;
    } else {
        a2x = p0X.x;
        a0 = p1;
        a1 = p0;
    }

    // Calculate area with the shoelace formula.
    // This is conceptually the sum of 5 determinants for points A0-A5, where A2-A5 are:
    //
    //      A2 = (a2.x, a1.y)
    //      A3 = (a2.x, top)
    //      A4 = (a0.x, top)
    //
    // The formula is optimized. See: http://geomalgorithms.com/a01-_area.html
    float area = a0.x * (a0.y + a1.y - 2.0 * pixelTop) +
                 a1.x * (a1.y - a0.y) +
                 2.0 * a2x * (pixelTop - a1.y);
    return abs(area) * winding * 0.5;
}

/// Solves the equation:
///
///    x = p0x + t^2 * (p0x - 2*p1x + p2x) + t*(2*p1x - 2*p0x)
///
/// We use the Citardauq Formula to avoid floating point precision issues.
vec2 solveCurveT(float p0x, float p1x, float p2x, vec2 x) {
    float a = p0x - 2.0 * p1x + p2x;
    float b = 2.0 * p1x - 2.0 * p0x;
    vec2 c = p0x - x;
    return 2.0 * c / (-b - sqrt(b * b - 4.0 * a * c));
}

/// Applies a slight horizontal blur to reduce color fringing on LCD screens
/// when performing subpixel AA.
///
/// The algorithm should be identical to that of FreeType:
/// https://www.freetype.org/freetype2/docs/reference/ft2-lcd_filtering.html
float freetypeLCDFilter(float shadeL2, float shadeL1, float shade0, float shadeR1, float shadeR2) {
    return FREETYPE_LCD_FILTER_FACTOR_2 * shadeL2 +
        FREETYPE_LCD_FILTER_FACTOR_1 * shadeL1 +
        FREETYPE_LCD_FILTER_FACTOR_0 * shade0 +
        FREETYPE_LCD_FILTER_FACTOR_1 * shadeR1 +
        FREETYPE_LCD_FILTER_FACTOR_2 * shadeR2;
}

float sample1Tap(sampler2D source, vec2 center, float offset) {
    return texture2D(source, vec2(center.x + offset, center.y)).r;
}

void sample9Tap(out vec4 outShadesL,
                out float outShadeC,
                out vec4 outShadesR,
                sampler2D source,
                vec2 center,
                float onePixel,
                vec4 kernel) {
    outShadesL = vec4(kernel.x > 0.0 ? sample1Tap(source, center, -4.0 * onePixel) : 0.0,
                      sample1Tap(source, center, -3.0 * onePixel),
                      sample1Tap(source, center, -2.0 * onePixel),
                      sample1Tap(source, center, -1.0 * onePixel));
    outShadeC = sample1Tap(source, center, 0.0);
    outShadesR = vec4(sample1Tap(source, center, 1.0 * onePixel),
                      sample1Tap(source, center, 2.0 * onePixel),
                      sample1Tap(source, center, 3.0 * onePixel),
                      kernel.x > 0.0 ? sample1Tap(source, center, 4.0 * onePixel) : 0.0);
}

float convolve7Tap(vec4 shades0, vec3 shades1, vec4 kernel) {
    return dot(shades0, kernel) + dot(shades1, kernel.zyx);
}

float gammaCorrectChannel(float fgColor, float bgColor, sampler2D gammaLUT) {
    return texture2D(gammaLUT, vec2(fgColor, 1.0 - bgColor)).r;
}

// `fgColor` is in linear space.
vec3 gammaCorrect(vec3 fgColor, vec3 bgColor, sampler2D gammaLUT) {
    return vec3(gammaCorrectChannel(fgColor.r, bgColor.r, gammaLUT),
                gammaCorrectChannel(fgColor.g, bgColor.g, gammaLUT),
                gammaCorrectChannel(fgColor.b, bgColor.b, gammaLUT));
}

vec4 fetchFloat4Data(sampler2D dataTexture, int index, ivec2 dimensions) {
    ivec2 pixelCoord = ivec2(imod(index, dimensions.x), index / dimensions.x);
    return texture2D(dataTexture, (vec2(pixelCoord) + 0.5) / vec2(dimensions));
}

vec2 fetchFloat2Data(sampler2D dataTexture, int index, ivec2 dimensions) {
    int texelIndex = index / 2;
    vec4 texel = fetchFloat4Data(dataTexture, texelIndex, dimensions);
    return texelIndex * 2 == index ? texel.xy : texel.zw;
}

vec4 fetchPathAffineTransform(out vec2 outPathTransformExt,
                              sampler2D pathTransformSTTexture,
                              ivec2 pathTransformSTDimensions,
                              sampler2D pathTransformExtTexture,
                              ivec2 pathTransformExtDimensions,
                              int pathID) {
    outPathTransformExt = fetchFloat2Data(pathTransformExtTexture,
                                          pathID,
                                          pathTransformExtDimensions);
    return fetchFloat4Data(pathTransformSTTexture, pathID, pathTransformSTDimensions);
}

// Are we inside the convex hull of the curve? (This will always be false if this is a line.)
bool insideCurve(vec3 uv) {
    return uv.z != 0.0 && uv.x > 0.0 && uv.x < 1.0 && uv.y > 0.0 && uv.y < 1.0;
}

float signedDistanceToCurve(vec2 uv, vec2 dUVDX, vec2 dUVDY, bool inCurve) {
    // u^2 - v for curves inside uv square; u - v otherwise.
    float g = uv.x;
    vec2 dG = vec2(dUVDX.x, dUVDY.x);
    if (inCurve) {
        g *= uv.x;
        dG *= 2.0 * uv.x;
    }
    g -= uv.y;
    dG -= vec2(dUVDX.y, dUVDY.y);
    return g / length(dG);
}

// Cubic approximation to the square area coverage, accurate to about 4%.
float estimateArea(float dist) {
    if (dist >= 0.707107)
        return 0.5;
    // Catch NaNs here.
    if (!(dist > -0.707107))
        return -0.5;
    return 1.14191 * dist - 0.83570 * dist * dist * dist;
}
