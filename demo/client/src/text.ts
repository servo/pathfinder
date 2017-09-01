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

opentype.Font.prototype.isSupported = function() {
    return (this as any).supported;
}

export class TextLayout<Glyph extends PathfinderGlyph> {
    constructor(fontData: ArrayBuffer,
                text: string,
                createGlyph: (glyph: opentype.Glyph) => Glyph) {
        this.fontData = fontData;
        this.font = opentype.parse(fontData);
        assert(this.font.isSupported(), "The font type is unsupported!");

        // Lay out the text.
        this.lineGlyphs = text.split("\n").map(line => {
            return this.font.stringToGlyphs(line).map(createGlyph);
        });
        this.textGlyphs = _.flatten(this.lineGlyphs);

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

    layoutText() {
        const os2Table = this.font.tables.os2;
        const lineHeight = os2Table.sTypoAscender - os2Table.sTypoDescender +
            os2Table.sTypoLineGap;

        const currentPosition = glmatrix.vec2.create();

        let glyphIndex = 0;
        for (const line of this.lineGlyphs) {
            for (let lineCharIndex = 0; lineCharIndex < line.length; lineCharIndex++) {
                const textGlyph = this.textGlyphs[glyphIndex];
                textGlyph.position = glmatrix.vec2.clone(currentPosition);
                currentPosition[0] += textGlyph.advanceWidth;
                glyphIndex++;
            }

            currentPosition[0] = 0;
            currentPosition[1] -= lineHeight;
        }
    }

    readonly fontData: ArrayBuffer;
    readonly font: Font;
    readonly lineGlyphs: Glyph[][];
    readonly textGlyphs: Glyph[];
    readonly uniqueGlyphs: Glyph[];
}

export abstract class PathfinderGlyph {
    constructor(glyph: opentype.Glyph) {
        this.opentypeGlyph = glyph;
        this._metrics = null;
        this.position = glmatrix.vec2.create();
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

    setPixelPosition(pixelPosition: glmatrix.vec2, pixelsPerUnit: number): void {
        glmatrix.vec2.scale(this.position, pixelPosition, 1.0 / pixelsPerUnit);
    }

    readonly opentypeGlyph: opentype.Glyph;

    private _metrics: Metrics | null;

    /// In font units.
    position: glmatrix.vec2;
}
