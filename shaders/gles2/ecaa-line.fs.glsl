// pathfinder/shaders/gles2/ecaa-line.vs.glsl
//
// Copyright (c) 2017 Mozilla Foundation

precision highp float;

uniform bool uLowerPart;

varying vec4 vEndpoints;

void main() {
    // Unpack.
    vec2 center = gl_FragCoord.xy;
    vec2 p0 = vEndpoints.xy, p1 = vEndpoints.zw;

    bool slopeNegative = p0.y > p1.y;

    // Set up Liang-Barsky clipping.
    vec4 pixelExtents = center.xxyy + vec4(-0.5, 0.5, -0.5, 0.5);
    vec4 p = (p1 - p0).xxyy, q = pixelExtents - p0.xxyy;

    // Use Liang-Barsky to clip to the left and right sides of this pixel.
    vec2 t = clamp(q.xy / p.xy, 0.0, 1.0);
    vec2 spanP0 = p0 + p.yw * t.x, spanP1 = p0 + p.yw * t.y;

    // ...and to the bottom and top.
    if (p.z != 0.0) {
        vec2 tVertical = q.zw / p.zw;
        if (slopeNegative)
            tVertical.xy = tVertical.yx;    // FIXME(pcwalton): Can this be removed?
        t = vec2(max(t.x, tVertical.x), min(t.y, tVertical.y));
    }

    // If the line doesn't pass through this pixel, detect that and bail.
    if (t.x >= t.y) {
        bool fill = uLowerPart ? spanP0.y < pixelExtents.z : spanP0.y > pixelExtents.w;
        gl_FragColor = vec4(fill ? spanP1.x - spanP0.x : 0.0);
        return;
    }

    // Calculate A2.x.
    float a2x;
    if (xor(uLowerPart, slopeNegative)) {
        a2x = spanP0.x;
        t.xy = t.yx;
    } else {
        a2x = spanP1.x;
    }

    // Calculate A3.y.
    float a3y = uLowerPart ? pixelExtents.w : pixelExtents.z;

    // Calculate A0-A5.
    vec2 a0 = p0 + p.yw * t.x;
    vec2 a1 = p0 + p.yw * t.y;
    vec2 a2 = vec2(a2x, a1.y);
    vec2 a3 = vec2(a2x, a3y);
    vec2 a4 = vec2(a0.x, a3y);

    // Calculate area with the shoelace formula.
    float area = det2(a0, a1) + det2(a1, a2) + det2(a2, a3) + det2(a3, a4) + det2(a4, a0); 
    area *= slopeNegative ? 0.5 : -0.5;

    // Done!
    gl_FragColor = vec4(area);
}
