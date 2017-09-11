// pathfinder/client/src/text.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {Font, Metrics} from 'opentype.js';
import * as base64js from 'base64-js';
import * as glmatrix from 'gl-matrix';
import * as _ from 'lodash';
import * as opentype from "opentype.js";

import {B_QUAD_SIZE, PathfinderMeshData} from "./meshes";
import {UINT32_SIZE, UINT32_MAX, assert, panic} from "./utils";

export const BUILTIN_FONT_URI: string = "/otf/demo";

const PARTITION_FONT_ENDPOINT_URI: string = "/partition-font";

export interface ExpandedMeshData {
    meshes: PathfinderMeshData;
}

type CreateGlyphFn<Glyph> = (glyph: opentype.Glyph) => Glyph;

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

export class TextRun<Glyph extends PathfinderGlyph> {
    constructor(text: string | Glyph[],
                origin: number[],
                font: Font,
                createGlyph: CreateGlyphFn<Glyph>) {
        if (typeof(text) === 'string')
            text = font.stringToGlyphs(text).map(createGlyph);

        this.glyphs = text;
        this.origin = origin;
    }

    layout() {
        let currentX = this.origin[0];
        for (const glyph of this.glyphs) {
            glyph.origin = glmatrix.vec2.fromValues(currentX, this.origin[1]);
            currentX += glyph.advanceWidth;
        }
    }

    get measure(): number {
        const lastGlyph = _.last(this.glyphs);
        return lastGlyph == null ? 0.0 : lastGlyph.origin[0] + lastGlyph.advanceWidth;
    }

    readonly glyphs: Glyph[];
    readonly origin: number[];
}

export class TextFrame<Glyph extends PathfinderGlyph> {
    constructor(runs: TextRun<Glyph>[], font: Font) {
        this.runs = runs;
        this.font = font;
    }

    expandMeshes(uniqueGlyphs: Glyph[], meshes: PathfinderMeshData): ExpandedMeshData {
        const bQuads = _.chunk(new Uint32Array(meshes.bQuads), B_QUAD_SIZE / UINT32_SIZE);
        const bVertexPositions = new Float32Array(meshes.bVertexPositions);
        const bVertexPathIDs = new Uint16Array(meshes.bVertexPathIDs);
        const bVertexLoopBlinnData = new Uint32Array(meshes.bVertexLoopBlinnData);

        const expandedBQuads: number[] = [];
        const expandedBVertexPositions: number[] = [];
        const expandedBVertexPathIDs: number[] = [];
        const expandedBVertexLoopBlinnData: number[] = [];
        const expandedCoverInteriorIndices: number[] = [];
        const expandedCoverCurveIndices: number[] = [];

        let textGlyphIndex = 0;
        for (const textRun of this.runs) {
            for (const textGlyph of textRun.glyphs) {
                const uniqueGlyphIndex = _.sortedIndexBy(uniqueGlyphs, textGlyph, 'index');
                if (uniqueGlyphIndex < 0)
                    continue;
                const firstBVertexIndex = _.sortedIndex(bVertexPathIDs, uniqueGlyphIndex + 1);
                if (firstBVertexIndex < 0)
                    continue;

                // Copy over vertices.
                let bVertexIndex = firstBVertexIndex;
                const firstExpandedBVertexIndex = expandedBVertexPathIDs.length;
                while (bVertexIndex < bVertexPathIDs.length &&
                    bVertexPathIDs[bVertexIndex] === uniqueGlyphIndex + 1) {
                    expandedBVertexPositions.push(bVertexPositions[bVertexIndex * 2 + 0],
                                                bVertexPositions[bVertexIndex * 2 + 1]);
                    expandedBVertexPathIDs.push(textGlyphIndex + 1);
                    expandedBVertexLoopBlinnData.push(bVertexLoopBlinnData[bVertexIndex]);
                    bVertexIndex++;
                }

                // Copy over indices.
                copyIndices(expandedCoverInteriorIndices,
                            new Uint32Array(meshes.coverInteriorIndices),
                            firstExpandedBVertexIndex,
                            firstBVertexIndex,
                            bVertexIndex);
                copyIndices(expandedCoverCurveIndices,
                            new Uint32Array(meshes.coverCurveIndices),
                            firstExpandedBVertexIndex,
                            firstBVertexIndex,
                            bVertexIndex);

                // Copy over B-quads.
                let firstBQuadIndex =
                    _.findIndex(bQuads, bQuad => bVertexPathIDs[bQuad[0]] == uniqueGlyphIndex + 1);
                if (firstBQuadIndex < 0)
                    firstBQuadIndex = bQuads.length;
                const indexDelta = firstExpandedBVertexIndex - firstBVertexIndex;
                for (let bQuadIndex = firstBQuadIndex; bQuadIndex < bQuads.length; bQuadIndex++) {
                    const bQuad = bQuads[bQuadIndex];
                    if (bVertexPathIDs[bQuad[0]] !== uniqueGlyphIndex + 1)
                        break;
                    for (let indexIndex = 0; indexIndex < B_QUAD_SIZE / UINT32_SIZE; indexIndex++) {
                        const srcIndex = bQuad[indexIndex];
                        if (srcIndex === UINT32_MAX)
                            expandedBQuads.push(srcIndex);
                        else
                            expandedBQuads.push(srcIndex + indexDelta);
                    }
                }

                textGlyphIndex++;
            }
        }

        return {
            meshes: new PathfinderMeshData({
                bQuads: new Uint32Array(expandedBQuads).buffer as ArrayBuffer,
                bVertexPositions: new Float32Array(expandedBVertexPositions).buffer as ArrayBuffer,
                bVertexPathIDs: new Uint16Array(expandedBVertexPathIDs).buffer as ArrayBuffer,
                bVertexLoopBlinnData: new Uint32Array(expandedBVertexLoopBlinnData).buffer as
                    ArrayBuffer,
                coverInteriorIndices: new Uint32Array(expandedCoverInteriorIndices).buffer as
                    ArrayBuffer,
                coverCurveIndices: new Uint32Array(expandedCoverCurveIndices).buffer as
                    ArrayBuffer,
                edgeUpperCurveIndices: new ArrayBuffer(0),
                edgeUpperLineIndices: new ArrayBuffer(0),
                edgeLowerCurveIndices: new ArrayBuffer(0),
                edgeLowerLineIndices: new ArrayBuffer(0),
            })
        }
    }

    get bounds(): glmatrix.vec4 {
        if (this.runs.length === 0)
            return glmatrix.vec4.create();

        const upperLeft = glmatrix.vec2.clone(this.runs[0].origin);
        const lowerRight = glmatrix.vec2.clone(_.last(this.runs)!.origin);

        const lineHeight = this.font.lineHeight();
        upperLeft[1] += lineHeight * 2.0;
        lowerRight[1] -= lineHeight;

        lowerRight[0] = _.defaultTo<number>(_.max(this.runs.map(run => run.measure)), 0.0);

        return glmatrix.vec4.fromValues(upperLeft[0], upperLeft[1], lowerRight[0], lowerRight[1]);
    }

    get allGlyphs(): Glyph[] {
        return _.flatMap(this.runs, run => run.glyphs);
    }

    readonly runs: TextRun<Glyph>[];
    readonly origin: glmatrix.vec3;

    private readonly font: Font;

}

export class GlyphStorage<Glyph extends PathfinderGlyph> {
    constructor(fontData: ArrayBuffer,
                textFrames: TextFrame<Glyph>[],
                createGlyph: CreateGlyphFn<Glyph>,
                font?: Font) {
        if (font == null) {
            font = opentype.parse(fontData);
            assert(font.isSupported(), "The font type is unsupported!");
        }

        this.fontData = fontData;
        this.textFrames = textFrames;
        this.font = font;

        // Determine all glyphs potentially needed.
        this.uniqueGlyphs = this.allGlyphs;
        this.uniqueGlyphs.sort((a, b) => a.index - b.index);
        this.uniqueGlyphs = _.sortedUniqBy(this.uniqueGlyphs, glyph => glyph.index);
    }

    expandMeshes(meshes: PathfinderMeshData): ExpandedMeshData[] {
        return this.textFrames.map(textFrame => textFrame.expandMeshes(this.uniqueGlyphs, meshes));
    }

    partition(): Promise<PathfinderMeshData> {
        // Build the partitioning request to the server.
        //
        // FIXME(pcwalton): If this is a builtin font, don't resend it to the server!
        const request = {
            face: {
                Custom: base64js.fromByteArray(new Uint8Array(this.fontData)),
            },
            fontIndex: 0,
            glyphs: this.uniqueGlyphs.map(glyph => {
                const metrics = glyph.metrics;
                return {
                    id: glyph.index,
                    transform: [1, 0, 0, 1, 0, 0],
                };
            }),
            pointSize: this.font.unitsPerEm,
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
            return new PathfinderMeshData(response.Ok.pathData);
        });
    }

    layoutRuns() {
        for (const textFrame of this.textFrames)
            textFrame.runs.forEach(textRun => textRun.layout());
    }

    get allGlyphs(): Glyph[] {
        return _.flatMap(this.textFrames, textRun => textRun.allGlyphs);
    }

    readonly fontData: ArrayBuffer;
    readonly font: Font;
    readonly textFrames: TextFrame<Glyph>[];
    readonly uniqueGlyphs: Glyph[];
}

export class SimpleTextLayout<Glyph extends PathfinderGlyph> {
    constructor(fontData: ArrayBuffer, text: string, createGlyph: CreateGlyphFn<Glyph>) {
        const font = opentype.parse(fontData);
        assert(font.isSupported(), "The font type is unsupported!");

        const lineHeight = font.lineHeight();
        const textRuns: TextRun<Glyph>[] = text.split("\n").map((line, lineNumber) => {
            return new TextRun<Glyph>(line, [0.0, -lineHeight * lineNumber], font, createGlyph);
        });
        this.textFrame = new TextFrame(textRuns, font);

        this.glyphStorage = new GlyphStorage(fontData, [this.textFrame], createGlyph, font);
    }

    layoutRuns() {
        this.textFrame.runs.forEach(textRun => textRun.layout());
    }

    get allGlyphs(): Glyph[] {
        return this.textFrame.allGlyphs;
    }

    readonly textFrame: TextFrame<Glyph>;
    readonly glyphStorage: GlyphStorage<Glyph>;
}

export abstract class PathfinderGlyph {
    constructor(glyph: opentype.Glyph) {
        this.opentypeGlyph = glyph;
        this._metrics = null;
        this.origin = glmatrix.vec2.create();
    }

    get index(): number {
        return (this.opentypeGlyph as any).index;
    }

    get metrics(): opentype.Metrics {
        if (this._metrics == null)
            this._metrics = this.opentypeGlyph.getMetrics();
        return this._metrics;
    }

    get advanceWidth(): number {
        return this.opentypeGlyph.advanceWidth;
    }

    pixelOrigin(pixelsPerUnit: number): glmatrix.vec2 {
        const origin = glmatrix.vec2.create();
        glmatrix.vec2.scale(origin, this.origin, pixelsPerUnit);
        return origin;
    }

    setPixelOrigin(pixelOrigin: glmatrix.vec2, pixelsPerUnit: number): void {
        glmatrix.vec2.scale(this.origin, pixelOrigin, 1.0 / pixelsPerUnit);
    }

    setPixelLowerLeft(pixelLowerLeft: glmatrix.vec2, pixelsPerUnit: number): void {
        const pixelDescent = this.calculatePixelDescent(pixelsPerUnit);
        const pixelOrigin = glmatrix.vec2.fromValues(pixelLowerLeft[0],
                                                     pixelLowerLeft[1] + pixelDescent);
        this.setPixelOrigin(pixelOrigin, pixelsPerUnit);
    }

    private calculatePixelDescent(pixelsPerUnit: number): number {
        return Math.ceil(-this.metrics.yMin * pixelsPerUnit);
    }

    protected pixelMetrics(hint: Hint, pixelsPerUnit: number): PixelMetrics {
        const metrics = this.metrics;
        const top = hint.hintPosition(glmatrix.vec2.fromValues(0, metrics.yMax))[1];
        return {
            left: Math.floor(metrics.xMin * pixelsPerUnit),
            right: Math.ceil(metrics.xMax * pixelsPerUnit),
            ascent: Math.ceil(top * pixelsPerUnit),
            descent: this.calculatePixelDescent(pixelsPerUnit),
        };
    }

    pixelRect(hint: Hint, pixelsPerUnit: number): glmatrix.vec4 {
        const pixelMetrics = this.pixelMetrics(hint, pixelsPerUnit);
        const textGlyphOrigin = glmatrix.vec2.clone(this.origin);
        glmatrix.vec2.scale(textGlyphOrigin, textGlyphOrigin, pixelsPerUnit);
        glmatrix.vec2.round(textGlyphOrigin, textGlyphOrigin);

        return glmatrix.vec4.fromValues(textGlyphOrigin[0],
                                        textGlyphOrigin[1] - pixelMetrics.descent,
                                        textGlyphOrigin[0] + pixelMetrics.right,
                                        textGlyphOrigin[1] + pixelMetrics.ascent);

    }

    readonly opentypeGlyph: opentype.Glyph;

    private _metrics: Metrics | null;

    /// In font units, relative to (0, 0).
    origin: glmatrix.vec2;
}

export class Hint {
    constructor(font: Font, pixelsPerUnit: number, useHinting: boolean) {
        this.useHinting = useHinting;

        const os2Table = font.tables.os2;
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

function copyIndices(destIndices: number[],
                     srcIndices: Uint32Array,
                     firstExpandedIndex: number,
                     firstIndex: number,
                     lastIndex: number) {
    // FIXME(pcwalton): Use binary search instead of linear search.
    const indexDelta = firstExpandedIndex - firstIndex;
    let indexIndex = _.findIndex(srcIndices,
                                 srcIndex => srcIndex >= firstIndex && srcIndex < lastIndex);
    if (indexIndex < 0)
        return;
    while (indexIndex < srcIndices.length) {
        const index = srcIndices[indexIndex];
        if (index < firstIndex || index >= lastIndex)
            break;
        destIndices.push(index + indexDelta);
        indexIndex++;
    }
}
