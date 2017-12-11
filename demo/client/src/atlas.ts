// pathfinder/client/src/atlas.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';
import * as _ from 'lodash';

import {setTextureParameters} from './gl-utils';
import {calculatePixelDescent, calculatePixelRectForGlyph, calculatePixelXMin, Hint} from './text';
import {PathfinderFont, UnitMetrics} from './text';
import {unwrapNull} from './utils';
import {RenderContext} from './view';

export const SUBPIXEL_GRANULARITY: number = 4;

export const ATLAS_SIZE: glmatrix.vec2 = glmatrix.vec2.fromValues(2048, 4096);

export class Atlas {
    private _texture: WebGLTexture | null;
    private _usedSize: glmatrix.vec2;

    constructor() {
        this._texture = null;
        this._usedSize = glmatrix.vec2.create();
    }

    layoutGlyphs(glyphs: AtlasGlyph[],
                 font: PathfinderFont,
                 pixelsPerUnit: number,
                 rotationAngle: number,
                 hint: Hint,
                 emboldenAmount: glmatrix.vec2):
                 void {
        let nextOrigin = glmatrix.vec2.fromValues(1.0, 1.0);
        let shelfBottom = 2.0;

        for (const glyph of glyphs) {
            // Place the glyph, and advance the origin.
            const metrics = font.metricsForGlyph(glyph.glyphKey.id);
            if (metrics == null)
                continue;

            const unitMetrics = new UnitMetrics(metrics, rotationAngle, emboldenAmount);
            glyph.setPixelLowerLeft(nextOrigin, unitMetrics, pixelsPerUnit);

            let pixelOrigin = glyph.calculateSubpixelOrigin(pixelsPerUnit);
            nextOrigin[0] = calculatePixelRectForGlyph(unitMetrics,
                                                       pixelOrigin,
                                                       pixelsPerUnit,
                                                       hint)[2] + 1.0;

            // If the glyph overflowed the shelf, make a new one and reposition the glyph.
            if (nextOrigin[0] > ATLAS_SIZE[0]) {
                nextOrigin = glmatrix.vec2.clone([1.0, shelfBottom + 1.0]);
                glyph.setPixelLowerLeft(nextOrigin, unitMetrics, pixelsPerUnit);
                pixelOrigin = glyph.calculateSubpixelOrigin(pixelsPerUnit);
                nextOrigin[0] = calculatePixelRectForGlyph(unitMetrics,
                                                           pixelOrigin,
                                                           pixelsPerUnit,
                                                           hint)[2] + 1.0;
            }

            // Grow the shelf as necessary.
            const glyphBottom = calculatePixelRectForGlyph(unitMetrics,
                                                           pixelOrigin,
                                                           pixelsPerUnit,
                                                           hint)[3];
            shelfBottom = Math.max(shelfBottom, glyphBottom + 1.0);
        }

        // FIXME(pcwalton): Could be more precise if we don't have a full row.
        this._usedSize = glmatrix.vec2.clone([ATLAS_SIZE[0], shelfBottom]);
    }

    ensureTexture(renderContext: RenderContext): WebGLTexture {
        if (this._texture != null)
            return this._texture;

        const gl = renderContext.gl;
        const texture = unwrapNull(gl.createTexture());
        this._texture = texture;
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.texImage2D(gl.TEXTURE_2D,
                      0,
                      gl.RGBA,
                      ATLAS_SIZE[0],
                      ATLAS_SIZE[1],
                      0,
                      gl.RGBA,
                      gl.UNSIGNED_BYTE,
                      null);
        setTextureParameters(gl, gl.NEAREST);

        return texture;
    }

    get usedSize(): glmatrix.vec2 {
        return this._usedSize;
    }
}

export class AtlasGlyph {
    readonly glyphStoreIndex: number;
    readonly glyphKey: GlyphKey;
    readonly origin: glmatrix.vec2;

    constructor(glyphStoreIndex: number, glyphKey: GlyphKey) {
        this.glyphStoreIndex = glyphStoreIndex;
        this.glyphKey = glyphKey;
        this.origin = glmatrix.vec2.create();
    }

    calculateSubpixelOrigin(pixelsPerUnit: number): glmatrix.vec2 {
        const pixelOrigin = glmatrix.vec2.create();
        glmatrix.vec2.scale(pixelOrigin, this.origin, pixelsPerUnit);
        glmatrix.vec2.round(pixelOrigin, pixelOrigin);
        if (this.glyphKey.subpixel != null)
            pixelOrigin[0] += this.glyphKey.subpixel / SUBPIXEL_GRANULARITY;
        return pixelOrigin;
    }

    setPixelLowerLeft(pixelLowerLeft: glmatrix.vec2, metrics: UnitMetrics, pixelsPerUnit: number):
                      void {
        const pixelXMin = calculatePixelXMin(metrics, pixelsPerUnit);
        const pixelDescent = calculatePixelDescent(metrics, pixelsPerUnit);
        const pixelOrigin = glmatrix.vec2.clone([pixelLowerLeft[0] - pixelXMin,
                                                 pixelLowerLeft[1] - pixelDescent]);
        this.setPixelOrigin(pixelOrigin, pixelsPerUnit);
    }

    private setPixelOrigin(pixelOrigin: glmatrix.vec2, pixelsPerUnit: number): void {
        glmatrix.vec2.scale(this.origin, pixelOrigin, 1.0 / pixelsPerUnit);
    }

    get pathID(): number {
        if (this.glyphKey.subpixel == null)
            return this.glyphStoreIndex + 1;
        return this.glyphStoreIndex * SUBPIXEL_GRANULARITY + this.glyphKey.subpixel + 1;
    }
}

export class GlyphKey {
    readonly id: number;
    readonly subpixel: number | null;

    constructor(id: number, subpixel: number | null) {
        this.id = id;
        this.subpixel = subpixel;
    }

    get sortKey(): number {
        return this.subpixel == null ? this.id : this.id * SUBPIXEL_GRANULARITY + this.subpixel;
    }
}
