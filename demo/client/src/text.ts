// pathfinder/client/src/text.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as base64js from 'base64-js';
import * as glmatrix from 'gl-matrix';
import * as _ from 'lodash';
import * as opentype from "opentype.js";
import {Metrics} from 'opentype.js';

import {B_QUAD_SIZE, parseServerTiming, PathfinderMeshData} from "./meshes";
import {assert, lerp, panic, UINT32_MAX, UINT32_SIZE, unwrapNull} from "./utils";

export const BUILTIN_FONT_URI: string = "/otf/demo";

const SQRT_2: number = Math.sqrt(2.0);

// Should match macOS 10.13 High Sierra.
//
// We multiply by sqrt(2) to compensate for the fact that dilation amounts are relative to the
// pixel square on macOS and relative to the vertex normal in Pathfinder.
const STEM_DARKENING_FACTORS: glmatrix.vec2 = glmatrix.vec2.clone([
    0.0121 * SQRT_2,
    0.0121 * 1.25 * SQRT_2,
]);

// Likewise.
const MAX_STEM_DARKENING_AMOUNT: glmatrix.vec2 = glmatrix.vec2.clone([0.3 * SQRT_2, 0.3 * SQRT_2]);

// This value is a subjective cutoff. Above this ppem value, no stem darkening is performed.
const MAX_STEM_DARKENING_PIXELS_PER_EM: number = 72.0;

const PARTITION_FONT_ENDPOINT_URI: string = "/partition-font";

export interface ExpandedMeshData {
    meshes: PathfinderMeshData;
}

export interface PartitionResult {
    meshes: PathfinderMeshData;
    time: number;
}

export interface PixelMetrics {
    left: number;
    right: number;
    ascent: number;
    descent: number;
}

opentype.Font.prototype.isSupported = function() {
    return (this as any).supported;
};

opentype.Font.prototype.lineHeight = function() {
    const os2Table = this.tables.os2;
    return os2Table.sTypoAscender - os2Table.sTypoDescender + os2Table.sTypoLineGap;
};

export class PathfinderFont {
    readonly opentypeFont: opentype.Font;
    readonly data: ArrayBuffer;
    readonly builtinFontName: string | null;

    private metricsCache: Metrics[];

    constructor(data: ArrayBuffer, builtinFontName: string | null) {
        this.data = data;
        this.builtinFontName = builtinFontName != null ? builtinFontName : null;

        this.opentypeFont = opentype.parse(data);
        if (!this.opentypeFont.isSupported())
            panic("Unsupported font!");

        this.metricsCache = [];
    }

    metricsForGlyph(glyphID: number): Metrics | null {
        if (this.metricsCache[glyphID] == null)
            this.metricsCache[glyphID] = this.opentypeFont.glyphs.get(glyphID).getMetrics();
        return this.metricsCache[glyphID];
    }
}

export class TextRun {
    readonly glyphIDs: number[];
    advances: number[];
    readonly origin: number[];

    private readonly font: PathfinderFont;

    constructor(text: number[] | string, origin: number[], font: PathfinderFont) {
        if (typeof(text) === 'string') {
            this.glyphIDs = font.opentypeFont
                                .stringToGlyphs(text)
                                .map(glyph => (glyph as any).index);
        } else {
            this.glyphIDs = text;
        }

        this.origin = origin;
        this.advances = [];
        this.font = font;
    }

    layout() {
        this.advances = [];
        let currentX = 0;
        for (const glyphID of this.glyphIDs) {
            this.advances.push(currentX);
            currentX += this.font.opentypeFont.glyphs.get(glyphID).advanceWidth;
        }
    }

    calculatePixelOriginForGlyphAt(index: number, pixelsPerUnit: number, hint: Hint):
                                   glmatrix.vec2 {
        const textGlyphOrigin = glmatrix.vec2.clone(this.origin);
        textGlyphOrigin[0] += this.advances[index];
        glmatrix.vec2.scale(textGlyphOrigin, textGlyphOrigin, pixelsPerUnit);
        return textGlyphOrigin;
    }

    pixelRectForGlyphAt(index: number,
                        pixelsPerUnit: number,
                        hint: Hint,
                        stemDarkeningAmount: glmatrix.vec2,
                        subpixelGranularity: number):
                        glmatrix.vec4 {
        const metrics = unwrapNull(this.font.metricsForGlyph(this.glyphIDs[index]));
        const unitMetrics = new UnitMetrics(metrics, stemDarkeningAmount);
        const textGlyphOrigin = this.calculatePixelOriginForGlyphAt(index, pixelsPerUnit, hint);

        textGlyphOrigin[0] *= subpixelGranularity;
        glmatrix.vec2.round(textGlyphOrigin, textGlyphOrigin);
        textGlyphOrigin[0] /= subpixelGranularity;

        return calculatePixelRectForGlyph(unitMetrics, textGlyphOrigin, pixelsPerUnit, hint);
    }

    subpixelForGlyphAt(index: number,
                       pixelsPerUnit: number,
                       hint: Hint,
                       subpixelGranularity: number):
                       number {
        const textGlyphOrigin = this.calculatePixelOriginForGlyphAt(index, pixelsPerUnit, hint)[0];
        return Math.abs(Math.round(textGlyphOrigin * subpixelGranularity) % subpixelGranularity);
    }

    get measure(): number {
        const lastGlyphID = _.last(this.glyphIDs), lastAdvance = _.last(this.advances);
        if (lastGlyphID == null || lastAdvance == null)
            return 0.0;
        return lastAdvance + this.font.opentypeFont.glyphs.get(lastGlyphID).advanceWidth;
    }
}

export class TextFrame {
    readonly runs: TextRun[];
    readonly origin: glmatrix.vec3;

    private readonly font: PathfinderFont;

    constructor(runs: TextRun[], font: PathfinderFont) {
        this.runs = runs;
        this.origin = glmatrix.vec3.create();
        this.font = font;
    }

    expandMeshes(meshes: PathfinderMeshData, glyphIDs: number[]): ExpandedMeshData {
        const pathIDs = [];
        for (const textRun of this.runs) {
            for (const glyphID of textRun.glyphIDs) {
                if (glyphID === 0)
                    continue;
                const pathID = _.sortedIndexOf(glyphIDs, glyphID);
                pathIDs.push(pathID + 1);
            }
        }

        return {
            meshes: meshes.expand(pathIDs),
        };
    }

    get bounds(): glmatrix.vec4 {
        if (this.runs.length === 0)
            return glmatrix.vec4.create();

        const upperLeft = glmatrix.vec2.clone(this.runs[0].origin);
        const lowerRight = glmatrix.vec2.clone(_.last(this.runs)!.origin);

        const lowerLeft = glmatrix.vec2.clone([upperLeft[0], lowerRight[1]]);
        const upperRight = glmatrix.vec2.clone([lowerRight[0], upperLeft[1]]);

        const lineHeight = this.font.opentypeFont.lineHeight();
        lowerLeft[1] -= lineHeight;
        upperRight[1] += lineHeight * 2.0;

        upperRight[0] = _.defaultTo<number>(_.max(this.runs.map(run => run.measure)), 0.0);

        return glmatrix.vec4.clone([lowerLeft[0], lowerLeft[1], upperRight[0], upperRight[1]]);
    }

    get totalGlyphCount(): number {
        return _.sumBy(this.runs, run => run.glyphIDs.length);
    }

    get allGlyphIDs(): number[] {
        const glyphIDs = [];
        for (const run of this.runs)
            glyphIDs.push(...run.glyphIDs);
        return glyphIDs;
    }
}

/// Stores one copy of each glyph.
export class GlyphStore {
    readonly font: PathfinderFont;
    readonly glyphIDs: number[];

    constructor(font: PathfinderFont, glyphIDs: number[]) {
        this.font = font;
        this.glyphIDs = glyphIDs;
    }

    partition(): Promise<PartitionResult> {
        // Build the partitioning request to the server.
        let fontFace;
        if (this.font.builtinFontName != null)
            fontFace = { Builtin: this.font.builtinFontName };
        else
            fontFace = { Custom: base64js.fromByteArray(new Uint8Array(this.font.data)) };

        const request = {
            face: fontFace,
            fontIndex: 0,
            glyphs: this.glyphIDs.map(id => ({ id: id, transform: [1, 0, 0, 1, 0, 0] })),
            pointSize: this.font.opentypeFont.unitsPerEm,
        };

        // Make the request.
        let time = 0;
        return window.fetch(PARTITION_FONT_ENDPOINT_URI, {
            body: JSON.stringify(request),
            headers: {'Content-Type': 'application/json'} as any,
            method: 'POST',
        }).then(response => {
            time = parseServerTiming(response.headers);
            return response.arrayBuffer();
        }).then(buffer => {
            return {
                meshes: new PathfinderMeshData(buffer),
                time: time,
            };
        });
    }

    indexOfGlyphWithID(glyphID: number): number | null {
        const index = _.sortedIndexOf(this.glyphIDs, glyphID);
        return index >= 0 ? index : null;
    }
}

export class SimpleTextLayout {
    readonly textFrame: TextFrame;

    constructor(font: PathfinderFont, text: string) {
        const lineHeight = font.opentypeFont.lineHeight();
        const textRuns: TextRun[] = text.split("\n").map((line, lineNumber) => {
            return new TextRun(line, [0.0, -lineHeight * lineNumber], font);
        });
        this.textFrame = new TextFrame(textRuns, font);
    }

    layoutRuns() {
        this.textFrame.runs.forEach(textRun => textRun.layout());
    }
}

export class Hint {
    readonly xHeight: number;
    readonly hintedXHeight: number;
    readonly stemHeight: number;
    readonly hintedStemHeight: number;

    private useHinting: boolean;

    constructor(font: PathfinderFont, pixelsPerUnit: number, useHinting: boolean) {
        this.useHinting = useHinting;

        const os2Table = font.opentypeFont.tables.os2;
        this.xHeight = os2Table.sxHeight != null ? os2Table.sxHeight : 0;
        this.stemHeight = os2Table.sCapHeight != null ? os2Table.sCapHeight : 0;

        if (!useHinting) {
            this.hintedXHeight = this.xHeight;
            this.hintedStemHeight = this.stemHeight;
        } else {
            this.hintedXHeight = Math.round(Math.round(this.xHeight * pixelsPerUnit) /
                                            pixelsPerUnit);
            this.hintedStemHeight = Math.round(Math.round(this.stemHeight * pixelsPerUnit) /
                                               pixelsPerUnit);
        }
    }

    /// NB: This must match `hintPosition()` in `common.inc.glsl`.
    hintPosition(position: glmatrix.vec2): glmatrix.vec2 {
        if (!this.useHinting)
            return position;

        if (position[1] >= this.stemHeight) {
            const y = position[1] - this.stemHeight + this.hintedStemHeight;
            return glmatrix.vec2.clone([position[0], y]);
        }

        if (position[1] >= this.xHeight) {
            const y = lerp(this.hintedXHeight, this.hintedStemHeight,
                           (position[1] - this.xHeight) / (this.stemHeight - this.xHeight));
            return glmatrix.vec2.clone([position[0], y]);
        }

        if (position[1] >= 0.0) {
            const y = lerp(0.0, this.hintedXHeight, position[1] / this.xHeight);
            return glmatrix.vec2.clone([position[0], y]);
        }

        return position;
    }
}

export class UnitMetrics {
    left: number;
    right: number;
    ascent: number;
    descent: number;

    constructor(metrics: Metrics, stemDarkeningAmount: glmatrix.vec2) {
        this.left = metrics.xMin;
        this.right = metrics.xMax + stemDarkeningAmount[0] * 2;
        this.ascent = metrics.yMax + stemDarkeningAmount[1] * 2;
        this.descent = metrics.yMin;
    }
}

export function calculatePixelXMin(metrics: UnitMetrics, pixelsPerUnit: number): number {
    return Math.floor(metrics.left * pixelsPerUnit);
}

export function calculatePixelDescent(metrics: UnitMetrics, pixelsPerUnit: number): number {
    return Math.ceil(-metrics.descent * pixelsPerUnit);
}

function calculateSubpixelMetricsForGlyph(metrics: UnitMetrics, pixelsPerUnit: number, hint: Hint):
                                          PixelMetrics {
    const ascent = hint.hintPosition(glmatrix.vec2.fromValues(0, metrics.ascent))[1];
    return {
        ascent: ascent * pixelsPerUnit,
        descent: metrics.descent * pixelsPerUnit,
        left: metrics.left * pixelsPerUnit,
        right: metrics.right * pixelsPerUnit,
    };
}

export function calculatePixelRectForGlyph(metrics: UnitMetrics,
                                           subpixelOrigin: glmatrix.vec2,
                                           pixelsPerUnit: number,
                                           hint: Hint):
                                           glmatrix.vec4 {
    const pixelMetrics = calculateSubpixelMetricsForGlyph(metrics, pixelsPerUnit, hint);
    return glmatrix.vec4.clone([Math.floor(subpixelOrigin[0] + pixelMetrics.left),
                                Math.floor(subpixelOrigin[1] + pixelMetrics.descent),
                                Math.ceil(subpixelOrigin[0] + pixelMetrics.right),
                                Math.ceil(subpixelOrigin[1] + pixelMetrics.ascent)]);
}

export function computeStemDarkeningAmount(pixelsPerEm: number, pixelsPerUnit: number):
                                           glmatrix.vec2 {
    const amount = glmatrix.vec2.create();
    if (pixelsPerEm > MAX_STEM_DARKENING_PIXELS_PER_EM)
        return amount;

    glmatrix.vec2.scale(amount, STEM_DARKENING_FACTORS, pixelsPerEm);
    glmatrix.vec2.min(amount, amount, MAX_STEM_DARKENING_AMOUNT);
    glmatrix.vec2.scale(amount, amount, 1.0 / pixelsPerUnit);
    return amount;
}
