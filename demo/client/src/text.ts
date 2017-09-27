// pathfinder/client/src/text.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {Metrics} from 'opentype.js';
import * as base64js from 'base64-js';
import * as glmatrix from 'gl-matrix';
import * as _ from 'lodash';
import * as opentype from "opentype.js";

import {B_QUAD_SIZE, PathfinderMeshData} from "./meshes";
import { UINT32_SIZE, UINT32_MAX, assert, panic, unwrapNull } from "./utils";

export const BUILTIN_FONT_URI: string = "/otf/demo";

const PARTITION_FONT_ENDPOINT_URI: string = "/partition-font";

export interface ExpandedMeshData {
    meshes: PathfinderMeshData;
}

export interface PartitionResult {
    meshes: PathfinderMeshData,
    time: number,
}

export interface PixelMetrics {
    left: number;
    right: number;
    ascent: number;
    descent: number;
}

opentype.Font.prototype.isSupported = function() {
    return (this as any).supported;
}

opentype.Font.prototype.lineHeight = function() {
    const os2Table = this.tables.os2;
    return os2Table.sTypoAscender - os2Table.sTypoDescender + os2Table.sTypoLineGap;
};

export class PathfinderFont {
    constructor(data: ArrayBuffer) {
        this.data = data;

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

    readonly opentypeFont: opentype.Font;
    readonly data: ArrayBuffer;

    private metricsCache: Metrics[];
}

export class TextRun {
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

    private pixelMetricsForGlyphAt(index: number, pixelsPerUnit: number, hint: Hint):
                                   PixelMetrics {
        const metrics = unwrapNull(this.font.metricsForGlyph(index));
        return calculatePixelMetricsForGlyph(metrics, pixelsPerUnit, hint);
    }

    calculatePixelOriginForGlyphAt(index: number, pixelsPerUnit: number, hint: Hint):
                                   glmatrix.vec2 {
        const textGlyphOrigin = glmatrix.vec2.clone(this.origin);
        textGlyphOrigin[0] += this.advances[index];
        glmatrix.vec2.scale(textGlyphOrigin, textGlyphOrigin, pixelsPerUnit);
        return textGlyphOrigin;
    }

    pixelRectForGlyphAt(index: number, pixelsPerUnit: number, hint: Hint): glmatrix.vec4 {
        const metrics = unwrapNull(this.font.metricsForGlyph(this.glyphIDs[index]));
        const textGlyphOrigin = this.calculatePixelOriginForGlyphAt(index, pixelsPerUnit, hint);
        glmatrix.vec2.round(textGlyphOrigin, textGlyphOrigin);
        return calculatePixelRectForGlyph(metrics, textGlyphOrigin, pixelsPerUnit, hint);
    }

    get measure(): number {
        const lastGlyphID = _.last(this.glyphIDs), lastAdvance = _.last(this.advances);
        if (lastGlyphID == null || lastAdvance == null)
            return 0.0;
        return lastAdvance + this.font.opentypeFont.glyphs.get(lastGlyphID).advanceWidth;
    }

    readonly glyphIDs: number[];
    advances: number[];
    readonly origin: number[];
    private readonly font: PathfinderFont;
}

export class TextFrame {
    constructor(runs: TextRun[], font: PathfinderFont) {
        this.runs = runs;
        this.origin = glmatrix.vec3.create();
        this.font = font;
    }

    expandMeshes(meshes: PathfinderMeshData, glyphIDs: number[]): ExpandedMeshData {
        const pathIDs = [];
        for (const textRun of this.runs) {
            for (let glyphIndex = 0; glyphIndex < textRun.glyphIDs.length; glyphIndex++) {
                const glyphID = textRun.glyphIDs[glyphIndex];
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

    readonly runs: TextRun[];
    readonly origin: glmatrix.vec3;

    private readonly font: PathfinderFont;
}

/// Stores one copy of each glyph.
export class GlyphStore {
    constructor(font: PathfinderFont, glyphIDs: number[]) {
        this.font = font;
        this.glyphIDs = glyphIDs;
    }

    partition(): Promise<PartitionResult> {
        // Build the partitioning request to the server.
        //
        // FIXME(pcwalton): If this is a builtin font, don't resend it to the server!
        const request = {
            face: {
                Custom: base64js.fromByteArray(new Uint8Array(this.font.data)),
            },
            fontIndex: 0,
            glyphs: this.glyphIDs.map(id => ({ id: id, transform: [1, 0, 0, 1, 0, 0] })),
            pointSize: this.font.opentypeFont.unitsPerEm,
        };

        // Make the request.
        return window.fetch(PARTITION_FONT_ENDPOINT_URI, {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify(request),
        }).then(response => response.text()).then(responseText => {
            const response = JSON.parse(responseText);
            if (!('Ok' in response))
                panic("Failed to partition the font!");
            return {
                meshes: new PathfinderMeshData(response.Ok.pathData),
                time: response.Ok.time,
            };
        });
    }

    indexOfGlyphWithID(glyphID: number): number | null {
        const index = _.sortedIndexOf(this.glyphIDs, glyphID);
        return index >= 0 ? index : null;
    }

    readonly font: PathfinderFont;
    readonly glyphIDs: number[];
}

export class SimpleTextLayout {
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

    readonly textFrame: TextFrame;
}

export class Hint {
    constructor(font: PathfinderFont, pixelsPerUnit: number, useHinting: boolean) {
        this.useHinting = useHinting;

        const os2Table = font.opentypeFont.tables.os2;
        this.xHeight = os2Table.sxHeight != null ? os2Table.sxHeight : 0;

        if (!useHinting) {
            this.hintedXHeight = this.xHeight;
        } else {
            this.hintedXHeight = Math.ceil(Math.ceil(this.xHeight * pixelsPerUnit) /
                                           pixelsPerUnit);
        }
    }

    hintPosition(position: glmatrix.vec2): glmatrix.vec2 {
        if (!this.useHinting)
            return position;

        if (position[1] < 0.0)
            return position;

        if (position[1] >= this.hintedXHeight) {
            return glmatrix.vec2.fromValues(position[0],
                                            position[1] - this.xHeight + this.hintedXHeight);
        }

        return glmatrix.vec2.fromValues(position[0],
                                        position[1] / this.xHeight * this.hintedXHeight);
    }

    readonly xHeight: number;
    readonly hintedXHeight: number;
    private useHinting: boolean;
}

export function calculatePixelXMin(metrics: Metrics, pixelsPerUnit: number): number {
    return Math.floor(metrics.xMin * pixelsPerUnit);
}

export function calculatePixelDescent(metrics: Metrics, pixelsPerUnit: number): number {
    return Math.ceil(-metrics.yMin * pixelsPerUnit);
}

function calculatePixelMetricsForGlyph(metrics: Metrics, pixelsPerUnit: number, hint: Hint):
                                       PixelMetrics {
    const top = hint.hintPosition(glmatrix.vec2.fromValues(0, metrics.yMax))[1];
    return {
        left: calculatePixelXMin(metrics, pixelsPerUnit),
        right: Math.ceil(metrics.xMax * pixelsPerUnit),
        ascent: Math.ceil(top * pixelsPerUnit),
        descent: calculatePixelDescent(metrics, pixelsPerUnit),
    };
}

export function calculatePixelRectForGlyph(metrics: Metrics,
                                           pixelOrigin: glmatrix.vec2,
                                           pixelsPerUnit: number,
                                           hint: Hint):
                                           glmatrix.vec4 {
        const pixelMetrics = calculatePixelMetricsForGlyph(metrics, pixelsPerUnit, hint);
        return glmatrix.vec4.clone([pixelOrigin[0] + pixelMetrics.left,
                                    pixelOrigin[1] - pixelMetrics.descent,
                                    pixelOrigin[0] + pixelMetrics.right,
                                    pixelOrigin[1] + pixelMetrics.ascent]);
    }
