// pathfinder/shaders/fill_compute.inc.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

vec4 accumulateCoverageForFillList(int fillIndex, ivec2 tileSubCoord) {
    vec2 tileFragCoord = vec2(tileSubCoord) + vec2(0.5);
    vec4 coverages = vec4(0.0);
    int iteration = 0;
    do {
        uint fillFrom = iFills[fillIndex * 3 + 0], fillTo = iFills[fillIndex * 3 + 1];
        vec4 lineSegment = vec4(fillFrom & 0xffff, fillFrom >> 16,
                                fillTo   & 0xffff, fillTo   >> 16) / 256.0;
        lineSegment -= tileFragCoord.xyxy;
        coverages += computeCoverage(lineSegment.xy, lineSegment.zw, uAreaLUT);
        fillIndex = int(iFills[fillIndex * 3 + 2]);
        iteration++;
    } while (fillIndex >= 0 && iteration < 1024);
    return coverages;
}
