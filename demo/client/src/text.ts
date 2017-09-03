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

import {PathfinderMeshData} from "./meshes";
import {assert, panic} from "./utils";

export const BUILTIN_FONT_URI: string = "/otf/demo";

const PARTITION_FONT_ENDPOINT_URI: string = "/partition-font";

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

export class GlyphStorage<Glyph extends PathfinderGlyph> {
    constructor(fontData: ArrayBuffer,
                textGlyphs: Glyph[] | string,
                createGlyph: CreateGlyphFn<Glyph>,
                font?: Font) {
        if (font == null) {
            font = opentype.parse(fontData);
            assert(font.isSupported(), "The font type is unsupported!");
        }

        if (typeof(textGlyphs) === 'string')
            textGlyphs = font.stringToGlyphs(textGlyphs).map(createGlyph);

        this.fontData = fontData;
        this.textGlyphs = textGlyphs;
        this.font = font;

        // Determine all glyphs potentially needed.
        this.uniqueGlyphs = this.textGlyphs.map(textGlyph => textGlyph);
        this.uniqueGlyphs.sort((a, b) => a.index - b.index);
        this.uniqueGlyphs = _.sortedUniqBy(this.uniqueGlyphs, glyph => glyph.index);
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

    readonly fontData: ArrayBuffer;
    readonly font: Font;
    readonly textGlyphs: Glyph[];
    readonly uniqueGlyphs: Glyph[];
}

export class TextLayout<Glyph extends PathfinderGlyph> {
    constructor(fontData: ArrayBuffer, text: string, createGlyph: CreateGlyphFn<Glyph>) {
        const font = opentype.parse(fontData);
        assert(font.isSupported(), "The font type is unsupported!");

        this.lineGlyphs = text.split("\n").map(line => font.stringToGlyphs(line).map(createGlyph));

        const textGlyphs = _.flatten(this.lineGlyphs);
        this.glyphStorage = new GlyphStorage(fontData, textGlyphs, createGlyph, font);
    }

    layoutText() {
        const os2Table = this.glyphStorage.font.tables.os2;
        const lineHeight = os2Table.sTypoAscender - os2Table.sTypoDescender +
            os2Table.sTypoLineGap;

        const currentPosition = glmatrix.vec2.create();

        let glyphIndex = 0;
        for (const line of this.lineGlyphs) {
            for (let lineCharIndex = 0; lineCharIndex < line.length; lineCharIndex++) {
                const textGlyph = this.glyphStorage.textGlyphs[glyphIndex];
                textGlyph.origin = glmatrix.vec2.clone(currentPosition);
                currentPosition[0] += textGlyph.advanceWidth;
                glyphIndex++;
            }

            currentPosition[0] = 0;
            currentPosition[1] -= lineHeight;
        }
    }

    readonly lineGlyphs: Glyph[][];
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
        const pixelMetrics = this.pixelMetrics(pixelsPerUnit);
        const pixelOrigin = glmatrix.vec2.fromValues(pixelLowerLeft[0],
                                                     pixelLowerLeft[1] + pixelMetrics.descent);
        this.setPixelOrigin(pixelOrigin, pixelsPerUnit);
    }

    protected pixelMetrics(pixelsPerUnit: number): PixelMetrics {
        const metrics = this.metrics;
        return {
            left: Math.floor(metrics.xMin * pixelsPerUnit),
            right: Math.ceil(metrics.xMax * pixelsPerUnit),
            ascent: Math.ceil(metrics.yMax * pixelsPerUnit),
            descent: Math.ceil(-metrics.yMin * pixelsPerUnit),
        };
    }

    pixelRect(pixelsPerUnit: number): glmatrix.vec4 {
        const pixelMetrics = this.pixelMetrics(pixelsPerUnit);
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
