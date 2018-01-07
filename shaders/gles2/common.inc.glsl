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

/// Displaces the given point by the given distance in the direction of the normal angle.
vec2 dilatePosition(vec2 position, float normalAngle, vec2 amount) {
    return position + vec2(cos(normalAngle), -sin(normalAngle)) * amount;
}

vec2 offsetPositionVertically(vec2 position, ivec2 framebufferSize, bool roundUp) {
    position = convertClipToScreenSpace(position, framebufferSize);
    position.y = roundUp ? ceil(position.y + 1.0) : floor(position.y - 1.0);
    return convertScreenToClipSpace(position, framebufferSize);
}

vec2 computeMCAAPosition(vec2 position,
                         vec4 hints,
                         vec4 localTransformST,
                         vec4 globalTransformST,
                         ivec2 framebufferSize) {
    if (position == vec2(0.0))
        return position;

    position = hintPosition(position, hints);
    position = transformVertexPositionST(position, localTransformST);
    position = transformVertexPositionST(position, globalTransformST);
    return convertClipToScreenSpace(position, framebufferSize);
}

vec2 computeMCAASnappedPosition(vec2 position,
                                vec4 hints,
                                vec4 localTransformST,
                                vec4 globalTransformST,
                                ivec2 framebufferSize,
                                float slope,
                                bool snapToPixelGrid) {
    position = hintPosition(position, hints);
    position = transformVertexPositionST(position, localTransformST);
    position = transformVertexPositionST(position, globalTransformST);
    position = convertClipToScreenSpace(position, framebufferSize);

    float xNudge;
    if (snapToPixelGrid) {
        xNudge = fract(position.x);
        if (xNudge < 0.5)
            xNudge = -xNudge;
        else
            xNudge = 1.0 - xNudge;
    } else {
        xNudge = 0.0;
    }

    return position + vec2(xNudge, xNudge * slope);
}

vec2 transformECAAPosition(vec2 position,
                           vec4 localTransformST,
                           vec2 localTransformExt,
                           mat4 globalTransform) {
    position = transformVertexPositionAffine(position, localTransformST, localTransformExt);
    return transformVertexPosition(position, globalTransform);
}

vec2 transformECAAPositionToScreenSpace(vec2 position,
                                        vec4 localTransformST,
                                        vec2 localTransformExt,
                                        mat4 globalTransform,
                                        ivec2 framebufferSize) {
    position = transformECAAPosition(position,
                                     localTransformST,
                                     localTransformExt,
                                     globalTransform);
    return convertClipToScreenSpace(position, framebufferSize);
}

vec2 computeECAAPosition(vec2 position,
                         float normalAngle,
                         vec2 emboldenAmount,
                         vec4 hints,
                         vec4 localTransformST,
                         vec2 localTransformExt,
                         mat4 globalTransform,
                         ivec2 framebufferSize) {
    position = dilatePosition(position, normalAngle, emboldenAmount);
    position = hintPosition(position, hints);
    position = transformECAAPositionToScreenSpace(position,
                                                  localTransformST,
                                                  localTransformExt,
                                                  globalTransform,
                                                  framebufferSize);
    return position;
}

float computeECAAWinding(inout vec2 leftPosition, inout vec2 rightPosition) {
    float winding = sign(leftPosition.x - rightPosition.x);
    if (winding > 0.0) {
        vec2 tmp = leftPosition;
        leftPosition = rightPosition;
        rightPosition = tmp;
    }

    return rightPosition.x - leftPosition.x > EPSILON ? winding : 0.0;
}

vec2 computeECAAQuadPositionFromTransformedPositions(vec2 leftPosition,
                                                     vec2 rightPosition,
                                                     vec2 quadPosition,
                                                     ivec2 framebufferSize,
                                                     vec4 localTransformST,
                                                     vec2 localTransformExt,
                                                     mat4 globalTransform,
                                                     vec4 bounds,
                                                     vec3 leftTopRightEdges) {
    vec2 edgeBL = bounds.xy, edgeTL = bounds.xw, edgeTR = bounds.zw, edgeBR = bounds.zy;
    edgeBL = transformECAAPosition(edgeBL, localTransformST, localTransformExt, globalTransform);
    edgeBR = transformECAAPosition(edgeBR, localTransformST, localTransformExt, globalTransform);
    edgeTL = transformECAAPosition(edgeTL, localTransformST, localTransformExt, globalTransform);
    edgeTR = transformECAAPosition(edgeTR, localTransformST, localTransformExt, globalTransform);

    // Find the bottom of the path, and convert to clip space.
    //
    // FIXME(pcwalton): Speed this up somehow?
    float pathBottomY = max(max(edgeBL.y, edgeBR.y), max(edgeTL.y, edgeTR.y));
    pathBottomY = (pathBottomY + 1.0) * 0.5 * float(framebufferSize.y);

    vec4 extents = vec4(leftTopRightEdges, pathBottomY);
    vec2 position = mix(floor(extents.xy), ceil(extents.zw), quadPosition);
    return convertScreenToClipSpace(position, framebufferSize);
}

// FIXME(pcwalton): Clean up this signature somehow?
bool computeECAAQuadPosition(out vec2 outPosition,
                             out float outWinding,
                             inout vec2 leftPosition,
                             inout vec2 rightPosition,
                             vec2 quadPosition,
                             ivec2 framebufferSize,
                             vec4 localTransformST,
                             vec2 localTransformExt,
                             mat4 globalTransform,
                             vec4 hints,
                             vec4 bounds,
                             vec2 normalAngles,
                             vec2 emboldenAmount) {
    leftPosition = computeECAAPosition(leftPosition,
                                       normalAngles.x,
                                       emboldenAmount,
                                       hints,
                                       localTransformST,
                                       localTransformExt,
                                       globalTransform,
                                       framebufferSize);
    rightPosition = computeECAAPosition(rightPosition,
                                        normalAngles.y,
                                        emboldenAmount,
                                        hints,
                                        localTransformST,
                                        localTransformExt,
                                        globalTransform,
                                        framebufferSize);

    float winding = computeECAAWinding(leftPosition, rightPosition);
    outWinding = winding;
    if (winding == 0.0) {
        outPosition = vec2(0.0);
        return false;
    }

    vec3 leftTopRightEdges = vec3(leftPosition.x,
                                  min(leftPosition.y, rightPosition.y),
                                  rightPosition.x);
    outPosition = computeECAAQuadPositionFromTransformedPositions(leftPosition,
                                                                  rightPosition,
                                                                  quadPosition,
                                                                  framebufferSize,
                                                                  localTransformST,
                                                                  localTransformExt,
                                                                  globalTransform,
                                                                  bounds,
                                                                  leftTopRightEdges);
    return true;
}

bool splitCurveAndComputeECAAWinding(out float outWinding,
                                     out vec3 outLeftTopRightEdges,
                                     inout vec2 leftPosition,
                                     inout vec2 rightPosition,
                                     vec2 controlPointPosition,
                                     int passIndex) {
    // Split at the X inflection point if necessary.
    float num = leftPosition.x - controlPointPosition.x;
    float denom = leftPosition.x - 2.0 * controlPointPosition.x + rightPosition.x;
    float inflectionT = num / denom;
    if (inflectionT > EPSILON && inflectionT < 1.0 - EPSILON) {
        vec2 newCP0 = mix(leftPosition, controlPointPosition, inflectionT);
        vec2 newCP1 = mix(controlPointPosition, rightPosition, inflectionT);
        vec2 inflectionPoint = mix(newCP0, newCP1, inflectionT);
        if (passIndex == 0) {
            controlPointPosition = newCP0;
            rightPosition = inflectionPoint;
        } else {
            controlPointPosition = newCP1;
            leftPosition = inflectionPoint;
        }
    } else if (passIndex != 0) {
        return false;
    }

    float winding = computeECAAWinding(leftPosition, rightPosition);
    outWinding = winding;
    if (winding == 0.0)
        return false;

    outLeftTopRightEdges = vec3(min(leftPosition.x, controlPointPosition.x),
                                min(min(leftPosition.y, controlPointPosition.y), rightPosition.y),
                                max(rightPosition.x, controlPointPosition.x));
    return true;
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

/// Returns true if the line runs through this pixel or false otherwise.
///
/// The line must run left-to-right and must already be clipped to the left and right sides of the
/// pixel, which implies that `dP.x` must be within the range [0.0, 1.0].
///
/// * `p0X` is the start point of the line.
/// * `dPX` is the vector from the start point to the endpoint of the line.
/// * `pixelCenterY` is the Y coordinate of the center of the pixel in window coordinates (i.e.
///   `gl_FragCoord.y`).
bool isPartiallyCovered(vec2 p0X, vec2 dPX, float pixelCenterY) {
    float pixelTop;
    vec2 dP = clipLineToPixelRow(p0X, dPX, pixelCenterY, pixelTop).zw;
    return !isNearZero(dP.x) || !isNearZero(dP.y);
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
