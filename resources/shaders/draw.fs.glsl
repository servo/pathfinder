// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#version 330

// The size of the atlas in pixels.
uniform uvec2 uAtlasSize;

// The starting point of the segment.
flat in vec2 vP0;
// The endpoint of this segment.
flat in vec2 vP1;
// 1.0 if this segment runs left to right; -1.0 otherwise.
flat in float vDirection;
// The slope of this line.
flat in float vSlope;
// Minimum and maximum vertical extents, unrounded.
flat in vec2 vYMinMax;

out vec4 oFragColor;

void main() {
    // Compute the X boundaries of this pixel.
    float xMin = floor(gl_FragCoord.x);
    float xMax = xMin + 1.0f;

    // Compute the horizontal span that the line segment covers across this pixel.
    float dX = min(xMax, vP1.x) - max(xMin, vP0.x);

    // Compute the Y-intercepts of the portion of the line crossing this pixel.
    float yMin = clamp(vP0.y + (xMin - vP0.x) * vSlope, vYMinMax.x, vYMinMax.y);
    float yMax = clamp(yMin + vSlope, vYMinMax.x, vYMinMax.y);
    if (yMin > yMax) {
        float tmp = yMin;
        yMin = yMax;
        yMax = tmp;
    }

    // Round the Y-intercepts out to the nearest pixel.
    int yMinI = int(floor(yMin)), yMaxI = int(ceil(yMax));

    // Determine which vertical pixel we're looking at.
    int yI = int(floor(gl_FragCoord.y));

    // Compute trapezoidal area coverage.
    //
    // It may be helpful to follow along with this explanation, keeping in mind that we compute
    // downward coverage rather than rightward coverage:
    //
    //    http://nothings.org/gamedev/rasterize/
    //
    // Note that the algorithm above computes total area coverage for each pixel, while here we
    // compute *delta* coverage: that is, the *difference* in the area covered between this pixel
    // and the pixel above it. In general, this means that, in contrast to the stb_truetype
    // algorithm, we have to specially handle the first fully covered pixel, in order to account
    // for the residual area difference between that pixel and the one above it.
    float coverage = 0.0f;
    if (yMaxI <= yMinI + 1) {
        // The line touches only one pixel (case 1). Compute the area of that trapezoid (or the
        // residual area for the pixel right after that trapezoid).
        float trapArea = 0.5f * (yMin + yMax) - float(yMinI);
        if (yI == yMinI)
            coverage = 1.0f - trapArea;
        else if (yI == yMinI + 1)
            coverage = trapArea;
    } else {
        // The line touches multiple pixels (case 2). There are several subcases to handle here.

        // Compute the area of the topmost triangle.
        float yMinF = fract(yMin);
        float dXdY = 1.0f / (yMax - yMin);
        float triAreaMin = 0.5f * dXdY * (1.0f - yMinF) * (1.0f - yMinF);

        if (yI == yMinI) {
            // We're looking at the pixel that triangle covers, so we're done.
            coverage = triAreaMin;
        } else {
            // Compute the area of the bottommost triangle.
            float yMaxF = yMax - ceil(yMax) + 1.0f;
            float triAreaMax = 0.5f * dXdY * yMaxF * yMaxF;

            bool lineTouchesThreePixels = yMaxI == yMinI + 2;
            if (lineTouchesThreePixels && yI == yMinI + 1) {
                // The line touches exactly 3 pixels, and we're looking at the middle one.
                coverage = 1.0f - triAreaMin - triAreaMax;
            } else if (!lineTouchesThreePixels && yI < yMaxI) {
                // The line touches more than 3 pixels. Compute the area of the first trapezoid.
                float trapAreaMin = dXdY * (1.5f - yMinF);
                if (yI == yMinI + 1) {
                    // We're looking at that trapezoid, so we're done.
                    coverage = trapAreaMin - triAreaMin;
                } else if (yI == yMaxI - 1) {
                    // We're looking at the last trapezoid. Compute its area.
                    float trapAreaMax = trapAreaMin + float(yMaxI - yMinI - 3) * dXdY;
                    coverage = 1.0f - trapAreaMax - triAreaMax;
                } else if (yI > yMinI + 1 && yI < yMaxI - 1) {
                    // We're looking at one of the pixels in between the two trapezoids. The delta
                    // coverage in this case is simply the inverse slope.
                    coverage = dXdY;
                }
            } else if (yI == yMaxI) {
                // We're looking at the final pixel in the column.
                coverage = triAreaMax;
            }
        }
    }

    oFragColor = vec4(dX * vDirection * coverage, 1.0f, 1.0f, 1.0f);
}

