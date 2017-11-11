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

#define LCD_FILTER_FACTOR_0     (86.0 / 255.0)
#define LCD_FILTER_FACTOR_1     (77.0 / 255.0)
#define LCD_FILTER_FACTOR_2     (8.0  / 255.0)

#define MAX_PATHS   65536

#define EPSILON     0.001

precision highp float;

/// Computes `ia % ib`.
///
/// This function is not in OpenGL ES 2 but can be polyfilled.
/// See: https://stackoverflow.com/a/36078859
int imod(int ia, int ib) {
    float a = float(ia), b = float(ib);
    float m = a - floor((a + 0.5) / b) * b;
    return int(floor(m + 0.5));
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
vec2 transformVertexPositionST(vec2 position, vec4 stTransform) {
    return position * stTransform.xy + stTransform.zw;
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

/// Packs the given path ID into a floating point value suitable for storage in the depth buffer.
///
/// This function returns values in window space (i.e. what `gl_FragDepth`/`gl_FragDepthEXT` is
/// in).
float convertPathIndexToWindowDepthValue(int pathIndex) {
    return float(pathIndex) / float(MAX_PATHS);
}

int convertWindowDepthValueToPathIndex(float depthValue) {
    float pathIndex = floor(depthValue * float(MAX_PATHS));
    return int(pathIndex);
}

vec2 dilatePosition(vec2 position, float normalAngle, vec2 amount) {
    return position + vec2(cos(normalAngle), -sin(normalAngle)) * amount;
}

bool computeMCAAQuadPosition(out vec2 outPosition,
                             inout vec2 leftPosition,
                             inout vec2 rightPosition,
                             vec2 quadPosition,
                             ivec2 framebufferSize,
                             vec4 localTransformST,
                             vec4 globalTransformST,
                             vec4 hints) {
    leftPosition = hintPosition(leftPosition, hints);
    rightPosition = hintPosition(rightPosition, hints);

    leftPosition = transformVertexPositionST(leftPosition, localTransformST);
    rightPosition = transformVertexPositionST(rightPosition, localTransformST);

    leftPosition = transformVertexPositionST(leftPosition, globalTransformST);
    rightPosition = transformVertexPositionST(rightPosition, globalTransformST);

    leftPosition = convertClipToScreenSpace(leftPosition, framebufferSize);
    rightPosition = convertClipToScreenSpace(rightPosition, framebufferSize);

    if (abs(leftPosition.x - rightPosition.x) <= EPSILON) {
        outPosition = vec2(0.0);
        return false;
    }

    vec2 verticalExtents = vec2(min(leftPosition.y, rightPosition.y),
                                max(leftPosition.y, rightPosition.y));

    vec4 roundedExtents = vec4(floor(vec2(leftPosition.x, verticalExtents.x)),
                               ceil(vec2(rightPosition.x, verticalExtents.y)));

    vec2 position = mix(roundedExtents.xy, roundedExtents.zw, quadPosition);
    outPosition = convertScreenToClipSpace(position, framebufferSize);
    return true;
}

// FIXME(pcwalton): Clean up this signature somehow?
bool computeECAAQuadPosition(out vec2 outPosition,
                             out float outWinding,
                             inout vec2 leftPosition,
                             inout vec2 rightPosition,
                             vec2 quadPosition,
                             ivec2 framebufferSize,
                             vec4 localTransformST,
                             vec4 globalTransformST,
                             vec4 hints,
                             vec4 bounds,
                             float leftNormalAngle,
                             float rightNormalAngle,
                             vec2 emboldenAmount) {
    leftPosition = dilatePosition(leftPosition, leftNormalAngle, emboldenAmount);
    rightPosition = dilatePosition(rightPosition, rightNormalAngle, emboldenAmount);

    leftPosition = hintPosition(leftPosition, hints);
    rightPosition = hintPosition(rightPosition, hints);
    vec2 edgePosition = hintPosition(bounds.zw, hints);

    leftPosition = transformVertexPositionST(leftPosition, localTransformST);
    rightPosition = transformVertexPositionST(rightPosition, localTransformST);
    edgePosition = transformVertexPositionST(edgePosition, localTransformST);

    leftPosition = transformVertexPositionST(leftPosition, globalTransformST);
    rightPosition = transformVertexPositionST(rightPosition, globalTransformST);
    edgePosition = transformVertexPositionST(edgePosition, globalTransformST);

    leftPosition = convertClipToScreenSpace(leftPosition, framebufferSize);
    rightPosition = convertClipToScreenSpace(rightPosition, framebufferSize);
    edgePosition = convertClipToScreenSpace(edgePosition, framebufferSize);

    float winding = sign(leftPosition.x - rightPosition.x);
    if (winding > 0.0) {
        vec2 tmp = leftPosition;
        leftPosition = rightPosition;
        rightPosition = tmp;
    }
    outWinding = winding;

    if (rightPosition.x - leftPosition.x <= EPSILON) {
        outPosition = vec2(0.0);
        return false;
    }

    vec4 roundedExtents = vec4(floor(leftPosition.x),
                               floor(min(leftPosition.y, rightPosition.y)),
                               ceil(rightPosition.x),
                               ceil(edgePosition.y));

    vec2 position = mix(roundedExtents.xy, roundedExtents.zw, quadPosition);
    outPosition = convertScreenToClipSpace(position, framebufferSize);
    return true;
}

bool slopeIsNegative(vec2 dp) {
    return dp.y < -0.001;
}

bool slopeIsZero(vec2 dp) {
    return abs(dp.y) < 0.001;
}

vec2 clipToPixelBounds(vec2 p0,
                       vec2 dp,
                       vec2 center,
                       out vec4 outQ,
                       out vec4 outPixelExtents,
                       out vec2 outSpanP0,
                       out vec2 outSpanP1) {
    // Determine the bounds of this pixel.
    vec4 pixelExtents = center.xxyy + vec4(-0.5, 0.5, -0.5, 0.5);

    // Set up Liang-Barsky clipping.
    vec4 q = pixelExtents - p0.xxyy;

    // Use Liang-Barsky to clip to the left and right sides of this pixel.
    vec2 t = clamp(q.xy / dp.xx, 0.0, 1.0);
    vec2 spanP0 = p0 + dp * t.x, spanP1 = p0 + dp * t.y;

    // Likewise, clip to the to the bottom and top.
    if (!slopeIsZero(dp)) {
        vec2 tVertical = (slopeIsNegative(dp) ? q.wz : q.zw) / dp.yy;
        t = vec2(max(t.x, tVertical.x), min(t.y, tVertical.y));
    }

    outQ = q;
    outPixelExtents = pixelExtents;
    outSpanP0 = spanP0;
    outSpanP1 = spanP1;
    return t;
}

bool lineDoesNotPassThroughPixel(vec2 p0, vec2 dp, vec2 t, vec4 pixelExtents) {
    return t.x >= t.y || (slopeIsZero(dp) && (p0.y < pixelExtents.z || p0.y > pixelExtents.w));
}

// Computes the area of the polygon covering the pixel with the given boundaries.
//
// * `p0` is the start point of the line.
// * `dp` is the vector from the start point to the endpoint of the line.
// * `center` is the center of the pixel in window coordinates (i.e. `gl_FragCoord.xy`).
// * `winding` is the winding number (1 or -1).
float computeCoverage(vec2 p0, vec2 dp, vec2 center, float winding) {
    // Clip to the pixel bounds.
    vec4 q, pixelExtents;
    vec2 spanP0, spanP1;
    vec2 t = clipToPixelBounds(p0, dp, center, q, pixelExtents, spanP0, spanP1);

    // If the line doesn't pass through this pixel, detect that and bail.
    //
    // This should be worth a branch because it's very common for fragment blocks to all hit this
    // path.
    if (lineDoesNotPassThroughPixel(p0, dp, t, pixelExtents))
        return spanP0.y < pixelExtents.z ? winding * (spanP1.x - spanP0.x) : 0.0;

    // Calculate point A2, and swap the two clipped endpoints around if necessary.
    float a2x;
    if (slopeIsNegative(dp)) {
        a2x = spanP1.x;
    } else {
        a2x = spanP0.x;
        t.xy = t.yx;
    }

    // Calculate points A0-A3.
    vec2 a0 = p0 + dp * t.x, a1 = p0 + dp * t.y;
    float a3y = pixelExtents.w;

    // Calculate area with the shoelace formula.
    // This is conceptually the sum of 5 determinants for points A0-A5, where A2-A5 are:
    //
    //      A2 = (a2.x, a1.y)
    //      A3 = (a2.x, a3.y)
    //      A4 = (a0.x, a3.y)
    //
    // The formula is optimized. See: http://geomalgorithms.com/a01-_area.html
    float area = a0.x * (a0.y + a1.y - 2.0 * a3y) +
                 a1.x * (a1.y - a0.y) +
                 2.0 * a2x * (a3y - a1.y);
    return abs(area) * winding * 0.5;
}

// * `p0` is the start point of the line.
// * `dp` is the vector from the start point to the endpoint of the line.
// * `center` is the center of the pixel in window coordinates (i.e. `gl_FragCoord.xy`).
// * `winding` is the winding number (1 or -1).
bool isPartiallyCovered(vec2 p0, vec2 dp, vec2 center, float winding) {
    // Clip to the pixel bounds.
    vec4 q, pixelExtents;
    vec2 spanP0, spanP1;
    vec2 t = clipToPixelBounds(p0, dp, center, q, pixelExtents, spanP0, spanP1);
    return !lineDoesNotPassThroughPixel(p0, dp, t, pixelExtents);
}

// Solve the equation:
//
//    x = p0x + t^2 * (p0x - 2*p1x + p2x) + t*(2*p1x - 2*p0x)
//
// We use the Citardauq Formula to avoid floating point precision issues.
vec2 solveCurveT(float p0x, float p1x, float p2x, vec2 x) {
    float a = p0x - 2.0 * p1x + p2x;
    float b = 2.0 * p1x - 2.0 * p0x;
    vec2 c = p0x - x;
    return 2.0 * c / (-b - sqrt(b * b - 4.0 * a * c));
}

// https://www.freetype.org/freetype2/docs/reference/ft2-lcd_filtering.html
float lcdFilter(float shadeL2, float shadeL1, float shade0, float shadeR1, float shadeR2) {
    return LCD_FILTER_FACTOR_2 * shadeL2 +
        LCD_FILTER_FACTOR_1 * shadeL1 +
        LCD_FILTER_FACTOR_0 * shade0 +
        LCD_FILTER_FACTOR_1 * shadeR1 +
        LCD_FILTER_FACTOR_2 * shadeR2;
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

int unpackUInt16(vec2 packedValue) {
    ivec2 valueBytes = ivec2(floor(packedValue * 255.0));
    return valueBytes.y * 256 + valueBytes.x;
}

vec4 fetchFloat4Data(sampler2D dataTexture, int index, ivec2 dimensions) {
    ivec2 pixelCoord = ivec2(imod(index, dimensions.x), index / dimensions.x);
    return texture2D(dataTexture, (vec2(pixelCoord) + 0.5) / vec2(dimensions));
}

vec2 packPathID(int pathID) {
    return vec2(imod(pathID, 256), pathID / 256) / 255.0;
}

int unpackPathID(vec2 packedPathID) {
    return unpackUInt16(packedPathID);
}
