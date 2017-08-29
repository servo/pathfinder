// pathfinder/client/src/text.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as _ from 'lodash';
import * as base64js from 'base64-js';
import * as glmatrix from 'gl-matrix';
import * as opentype from 'opentype.js';

import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from './aa-strategy';
import {ECAAMonochromeStrategy, ECAAStrategy} from './ecaa-strategy';
import {createFramebuffer, createFramebufferColorTexture} from './gl-utils';
import {createFramebufferDepthTexture, QUAD_ELEMENTS, setTextureParameters} from './gl-utils';
import {UniformMap} from './gl-utils';
import {PathfinderMeshBuffers, PathfinderMeshData} from './meshes';
import {PathfinderShaderProgram, ShaderMap, ShaderProgramSource} from './shader-loader';
import {PathfinderError, assert, expectNotNull, UINT32_SIZE, unwrapNull, panic} from './utils';
import {MonochromePathfinderView, Timings} from './view';
import AppController from './app-controller';
import PathfinderBufferTexture from './buffer-texture';
import SSAAStrategy from './ssaa-strategy';

const TEXT: string =
`’Twas brillig, and the slithy toves
Did gyre and gimble in the wabe;
All mimsy were the borogoves,
And the mome raths outgrabe.

“Beware the Jabberwock, my son!
The jaws that bite, the claws that catch!
Beware the Jubjub bird, and shun
The frumious Bandersnatch!”

He took his vorpal sword in hand:
Long time the manxome foe he sought—
So rested he by the Tumtum tree,
And stood awhile in thought.

And as in uffish thought he stood,
The Jabberwock, with eyes of flame,
Came whiffling through the tulgey wood,
And burbled as it came!

One, two! One, two! And through and through
The vorpal blade went snicker-snack!
He left it dead, and with its head
He went galumphing back.

“And hast thou slain the Jabberwock?
Come to my arms, my beamish boy!
O frabjous day! Callooh! Callay!”
He chortled in his joy.

’Twas brillig, and the slithy toves
Did gyre and gimble in the wabe;
All mimsy were the borogoves,
And the mome raths outgrabe.`;

const INITIAL_FONT_SIZE: number = 72.0;

const PARTITION_FONT_ENDPOINT_URL: string = "/partition-font";

const B_POSITION_SIZE: number = 8;

const B_PATH_INDEX_SIZE: number = 2;

const ATLAS_SIZE: glmatrix.vec2 = glmatrix.vec2.fromValues(3072, 3072);

type Matrix4D = Float32Array;

type Rect = glmatrix.vec4;

interface Point2D {
    x: number;
    y: number;
}

type Size2D = glmatrix.vec2;

type ShaderType = number;

// `opentype.js` monkey patches

declare module 'opentype.js' {
    interface Font {
        isSupported(): boolean;
    }
    interface Glyph {
        getIndex(): number;
    }
}

opentype.Font.prototype.isSupported = function() {
    return (this as any).supported;
}

// Various utility functions

function expectNotUndef<T>(value: T | undefined, message: string): T {
    if (value === undefined)
        throw new PathfinderError(message);
    return value;
}

function unwrapUndef<T>(value: T | undefined): T {
    return expectNotUndef(value, "Unexpected `undefined`!");
}

/// The separating axis theorem.
function rectsIntersect(a: glmatrix.vec4, b: glmatrix.vec4): boolean {
    return a[2] > b[0] && a[3] > b[1] && a[0] < b[2] && a[1] < b[3];
}

class TextDemoController extends AppController<TextDemoView> {
    constructor() {
        super();
        this._atlas = new Atlas;
    }

    start() {
        super.start();

        this.fontSize = INITIAL_FONT_SIZE;

        this.fpsLabel = unwrapNull(document.getElementById('pf-fps-label'));

        this.loadFileButton = document.getElementById('pf-load-font-button') as HTMLInputElement;
        this.loadFileButton.addEventListener('change', () => this.loadFile(), false);

        this.aaLevelSelect = document.getElementById('pf-aa-level-select') as HTMLSelectElement;
        this.aaLevelSelect.addEventListener('change', () => this.updateAALevel(), false);
        this.updateAALevel();
    }

    protected createView(canvas: HTMLCanvasElement,
                         commonShaderSource: string,
                         shaderSources: ShaderMap<ShaderProgramSource>) {
        return new TextDemoView(this, canvas, commonShaderSource, shaderSources);
    }

    private updateAALevel() {
        const selectedOption = this.aaLevelSelect.selectedOptions[0];
        const aaType = unwrapUndef(selectedOption.dataset.pfType) as
            keyof AntialiasingStrategyTable;
        const aaLevel = parseInt(unwrapUndef(selectedOption.dataset.pfLevel));
        this.view.then(view => view.setAntialiasingOptions(aaType, aaLevel));
    }

    protected fileLoaded() {
        this.font = opentype.parse(this.fileData);
        if (!this.font.isSupported())
            throw new PathfinderError("The font type is unsupported.");

        // Lay out the text.
        this.lineGlyphs = TEXT.split("\n").map(line => {
            return this.font.stringToGlyphs(line).map(glyph => new TextGlyph(glyph));
        });
        this.textGlyphs = _.flatten(this.lineGlyphs);

        // Determine all glyphs potentially needed.
        this.uniqueGlyphs = this.textGlyphs.map(textGlyph => textGlyph);
        this.uniqueGlyphs.sort((a, b) => a.index - b.index);
        this.uniqueGlyphs = _.sortedUniqBy(this.uniqueGlyphs, glyph => glyph.index);

        // Build the partitioning request to the server.
        const request = {
            otf: base64js.fromByteArray(new Uint8Array(this.fileData)),
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
        window.fetch(PARTITION_FONT_ENDPOINT_URL, {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify(request),
        }).then(response => response.text()).then(responseText => {
            const response = JSON.parse(responseText);
            if (!('Ok' in response))
                panic("Failed to partition the font!");
            const meshes = response.Ok.pathData;
            this.meshes = new PathfinderMeshData(meshes);
            this.meshesReceived();
        });
    }

    private meshesReceived() {
        this.view.then(view => {
            view.attachText();
            view.uploadPathData(this.uniqueGlyphs.length);
            view.attachMeshes(this.meshes);
        })
    }

    updateTimings(newTimes: Timings) {
        this.fpsLabel.innerHTML =
            `${newTimes.atlasRendering} ms atlas, ${newTimes.compositing} ms compositing`;
    }

    get atlas(): Atlas {
        return this._atlas;
    }

    /// The font size in pixels per em.
    get fontSize(): number {
        return this._fontSize;
    }

    /// The font size in pixels per em.
    set fontSize(newFontSize: number) {
        this._fontSize = newFontSize;
        this.view.then(view => view.attachText());
    }

    private aaLevelSelect: HTMLSelectElement;
    private fpsLabel: HTMLElement;

    font: opentype.Font;
    lineGlyphs: TextGlyph[][];
    textGlyphs: TextGlyph[];
    uniqueGlyphs: PathfinderGlyph[];

    private _atlas: Atlas;
    atlasGlyphs: AtlasGlyph[];

    private meshes: PathfinderMeshData;

    private _fontSize: number;
}

class TextDemoView extends MonochromePathfinderView {
    constructor(appController: TextDemoController,
                canvas: HTMLCanvasElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(canvas, commonShaderSource, shaderSources);

        this.appController = appController;
    }

    protected initContext() {
        super.initContext();
    }

    uploadPathData(pathCount: number) {
        const pathColors = new Uint8Array(4 * (pathCount + 1));
        for (let pathIndex = 0; pathIndex < pathCount; pathIndex++) {
            for (let channel = 0; channel < 3; channel++)
                pathColors[(pathIndex + 1) * 4 + channel] = 0x00; // RGB
            pathColors[(pathIndex + 1) * 4 + 3] = 0xff;           // alpha
        }

        this.pathColorsBufferTexture.upload(this.gl, pathColors);
    }

    /// Lays out glyphs on the canvas.
    private layoutGlyphs() {
        const lineGlyphs = this.appController.lineGlyphs;
        const textGlyphs = this.appController.textGlyphs;

        const font = this.appController.font;
        this.pixelsPerUnit = this.appController.fontSize / font.unitsPerEm;

        const glyphPositions = new Float32Array(textGlyphs.length * 8);
        const glyphIndices = new Uint32Array(textGlyphs.length * 6);

        const os2Table = font.tables.os2;
        const lineHeight = (os2Table.sTypoAscender - os2Table.sTypoDescender +
                            os2Table.sTypoLineGap) * this.pixelsPerUnit;

        const currentPosition = glmatrix.vec2.create();

        let glyphIndex = 0;
        for (const line of lineGlyphs) {
            for (let lineCharIndex = 0; lineCharIndex < line.length; lineCharIndex++) {
                const textGlyph = textGlyphs[glyphIndex];
                const glyphMetrics = textGlyph.metrics;

                // Determine the atlas size.
                const atlasSize = glmatrix.vec2.fromValues(glyphMetrics.xMax - glyphMetrics.xMin,
                                                           glyphMetrics.yMax - glyphMetrics.yMin);
                glmatrix.vec2.scale(atlasSize, atlasSize, this.pixelsPerUnit);
                glmatrix.vec2.ceil(atlasSize, atlasSize);

                // Set positions.
                const textGlyphBL = glmatrix.vec2.create(), textGlyphTR = glmatrix.vec2.create();
                const offset = glmatrix.vec2.fromValues(glyphMetrics.leftSideBearing,
                                                        glyphMetrics.yMin);
                glmatrix.vec2.scale(offset, offset, this.pixelsPerUnit);
                glmatrix.vec2.add(textGlyphBL, currentPosition, offset);
                glmatrix.vec2.round(textGlyphBL, textGlyphBL);
                glmatrix.vec2.add(textGlyphTR, textGlyphBL, atlasSize);

                glyphPositions.set([
                    textGlyphBL[0], textGlyphTR[1],
                    textGlyphTR[0], textGlyphTR[1],
                    textGlyphBL[0], textGlyphBL[1],
                    textGlyphTR[0], textGlyphBL[1],
                ], glyphIndex * 8);

                textGlyph.canvasRect = glmatrix.vec4.fromValues(textGlyphBL[0], textGlyphBL[1],
                                                                textGlyphTR[0], textGlyphTR[1]);

                // Set indices.
                glyphIndices.set(Array.from(QUAD_ELEMENTS).map(index => index + 4 * glyphIndex),
                                 glyphIndex * 6);

                // Advance.
                currentPosition[0] += textGlyph.advanceWidth * this.pixelsPerUnit;

                glyphIndex++;
            }

            currentPosition[0] = 0;
            currentPosition[1] -= lineHeight;
        }

        this.glyphPositionsBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.glyphPositionsBuffer);
        this.gl.bufferData(this.gl.ARRAY_BUFFER, glyphPositions, this.gl.STATIC_DRAW);
        this.glyphElementsBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.glyphElementsBuffer);
        this.gl.bufferData(this.gl.ELEMENT_ARRAY_BUFFER, glyphIndices, this.gl.STATIC_DRAW);
    }

    private buildAtlasGlyphs() {
        const textGlyphs = this.appController.textGlyphs;

        // Only build glyphs in view.
        const canvasRect = glmatrix.vec4.fromValues(-this.translation[0],
                                                    -this.translation[1],
                                                    -this.translation[0] + this.canvas.width,
                                                    -this.translation[1] + this.canvas.height);

        let atlasGlyphs =
            textGlyphs.filter(textGlyph => rectsIntersect(textGlyph.canvasRect, canvasRect))
                      .map(textGlyph => new AtlasGlyph(textGlyph));
        atlasGlyphs.sort((a, b) => a.index - b.index);
        atlasGlyphs = _.sortedUniqBy(atlasGlyphs, glyph => glyph.index);
        this.appController.atlasGlyphs = atlasGlyphs;

        const fontSize = this.appController.fontSize;
        const unitsPerEm = this.appController.font.unitsPerEm;

        this.appController.atlas.layoutGlyphs(atlasGlyphs, fontSize, unitsPerEm);

        const uniqueGlyphIndices = this.appController.uniqueGlyphs.map(glyph => glyph.index);
        uniqueGlyphIndices.sort((a, b) => a - b);

        // TODO(pcwalton): Regenerate the IBOs to include only the glyphs we care about.
        const transforms = new Float32Array((this.appController.uniqueGlyphs.length + 1) * 4);
        for (let glyphIndex = 0; glyphIndex < atlasGlyphs.length; glyphIndex++) {
            const glyph = atlasGlyphs[glyphIndex];

            let pathID = _.sortedIndexOf(uniqueGlyphIndices, glyph.index);
            assert(pathID >= 0, "No path ID!");
            pathID++;

            const atlasLocation = glyph.atlasRect;
            const metrics = glyph.metrics;
            const left = metrics.xMin * this.pixelsPerUnit;
            const bottom = metrics.yMin * this.pixelsPerUnit;

            transforms[pathID * 4 + 0] = this.pixelsPerUnit;
            transforms[pathID * 4 + 1] = this.pixelsPerUnit;
            transforms[pathID * 4 + 2] = atlasLocation[0] - left;
            transforms[pathID * 4 + 3] = atlasLocation[1] - bottom;
        }

        this.pathTransformBufferTexture.upload(this.gl, transforms);
    }

    private createAtlasFramebuffer() {
        const atlasColorTexture = this.appController.atlas.ensureTexture(this.gl);
        this.atlasDepthTexture = createFramebufferDepthTexture(this.gl, ATLAS_SIZE);
        this.atlasFramebuffer = createFramebuffer(this.gl,
                                                    this.drawBuffersExt,
                                                    [atlasColorTexture],
                                                    this.atlasDepthTexture);

        // Allow the antialiasing strategy to set up framebuffers as necessary.
        this.antialiasingStrategy.setFramebufferSize(this);
    }

    private setGlyphTexCoords() {
        const textGlyphs = this.appController.textGlyphs;
        const atlasGlyphs = this.appController.atlasGlyphs;

        const atlasGlyphIndices = atlasGlyphs.map(atlasGlyph => atlasGlyph.index);

        const glyphTexCoords = new Float32Array(textGlyphs.length * 8);

        const currentPosition = glmatrix.vec2.create();

        for (let textGlyphIndex = 0; textGlyphIndex < textGlyphs.length; textGlyphIndex++) {
            const textGlyph = textGlyphs[textGlyphIndex];
            const textGlyphMetrics = textGlyph.metrics;

            let atlasGlyphIndex = _.sortedIndexOf(atlasGlyphIndices, textGlyph.index);
            if (atlasGlyphIndex < 0)
                continue;

            // Set texture coordinates.
            const atlasGlyph = atlasGlyphs[atlasGlyphIndex];
            const atlasGlyphRect = atlasGlyph.atlasRect;
            const atlasGlyphBL = atlasGlyphRect.slice(0, 2) as glmatrix.vec2;
            const atlasGlyphTR = atlasGlyphRect.slice(2, 4) as glmatrix.vec2;
            glmatrix.vec2.div(atlasGlyphBL, atlasGlyphBL, ATLAS_SIZE);
            glmatrix.vec2.div(atlasGlyphTR, atlasGlyphTR, ATLAS_SIZE);

            glyphTexCoords.set([
                atlasGlyphBL[0], atlasGlyphTR[1],
                atlasGlyphTR[0], atlasGlyphTR[1],
                atlasGlyphBL[0], atlasGlyphBL[1],
                atlasGlyphTR[0], atlasGlyphBL[1],
            ], textGlyphIndex * 8);
        }

        this.glyphTexCoordsBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.glyphTexCoordsBuffer);
        this.gl.bufferData(this.gl.ARRAY_BUFFER, glyphTexCoords, this.gl.STATIC_DRAW);
    }

    attachText() {
        if (this.atlasFramebuffer == null)
            this.createAtlasFramebuffer();
        this.layoutGlyphs();

        this.rebuildAtlasIfNecessary();
    }

    private rebuildAtlasIfNecessary() {
        this.buildAtlasGlyphs();
        this.setGlyphTexCoords();
        this.setDirty();
    }

    protected panned() {
        this.rebuildAtlasIfNecessary();
    }

    protected resized(initialSize: boolean) {
        if (!initialSize)
            this.antialiasingStrategy.init(this);
        this.setDirty();
    }

    private setIdentityTexScaleUniform(uniforms: UniformMap) {
        this.gl.uniform2f(uniforms.uTexScale, 1.0, 1.0);
    }

    protected get usedSizeFactor(): glmatrix.vec2 {
        const usedSize = glmatrix.vec2.create();
        glmatrix.vec2.div(usedSize, this.appController.atlas.usedSize, ATLAS_SIZE);
        return usedSize;
    }

    protected compositeIfNecessary() {
        // Set up composite state.
        this.gl.bindFramebuffer(this.gl.FRAMEBUFFER, null);
        this.gl.viewport(0, 0, this.canvas.width, this.canvas.height);
        this.gl.disable(this.gl.DEPTH_TEST);
        this.gl.disable(this.gl.BLEND);
        this.gl.disable(this.gl.SCISSOR_TEST);

        // Clear.
        this.gl.clearColor(1.0, 1.0, 1.0, 1.0);
        this.gl.clear(this.gl.COLOR_BUFFER_BIT);

        // Set up the composite VAO.
        const blitProgram = this.shaderPrograms.blit;
        const attributes = blitProgram.attributes;
        this.gl.useProgram(blitProgram.program);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.glyphPositionsBuffer);
        this.gl.vertexAttribPointer(attributes.aPosition, 2, this.gl.FLOAT, false, 0, 0);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.glyphTexCoordsBuffer);
        this.gl.vertexAttribPointer(attributes.aTexCoord, 2, this.gl.FLOAT, false, 0, 0);
        this.gl.enableVertexAttribArray(attributes.aPosition);
        this.gl.enableVertexAttribArray(attributes.aTexCoord);
        this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.glyphElementsBuffer);

        // Create the transform.
        const transform = glmatrix.mat4.create();
        glmatrix.mat4.fromTranslation(transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform,
                            transform,
                            [2.0 / this.canvas.width, 2.0 / this.canvas.height, 1.0]);
        glmatrix.mat4.translate(transform,
                                transform,
                                [this.translation[0], this.translation[1], 0.0]);

        // Blit.
        this.gl.uniformMatrix4fv(blitProgram.uniforms.uTransform, false, transform);
        this.gl.activeTexture(this.gl.TEXTURE0);
        this.gl.bindTexture(this.gl.TEXTURE_2D, this.appController.atlas.ensureTexture(this.gl));
        this.gl.uniform1i(blitProgram.uniforms.uSource, 0);
        this.setIdentityTexScaleUniform(blitProgram.uniforms);
        this.gl.drawElements(this.gl.TRIANGLES,
                             this.appController.textGlyphs.length * 6,
                             this.gl.UNSIGNED_INT,
                             0);
    }

    get bgColor(): glmatrix.vec4 {
        return glmatrix.vec4.fromValues(1.0, 1.0, 1.0, 1.0);
    }

    get fgColor(): glmatrix.vec4 {
        return glmatrix.vec4.fromValues(0.0, 0.0, 0.0, 1.0);
    }

    get destFramebuffer(): WebGLFramebuffer {
        return this.atlasFramebuffer;
    }

    get destAllocatedSize(): glmatrix.vec2 {
        return ATLAS_SIZE;
    }

    get destUsedSize(): glmatrix.vec2 {
        return this.appController.atlas.usedSize;
    }

    protected get scale(): number {
        return this.appController.fontSize;
    }

    protected set scale(newScale: number) {
        this.appController.fontSize = newScale;
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName, aaLevel: number):
                               AntialiasingStrategy {
        return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel);
    }

    protected updateTimings(timings: Timings) {
        this.appController.updateTimings(timings);
    }

    protected get worldTransform() {
        return glmatrix.mat4.create();
    }

    atlasFramebuffer: WebGLFramebuffer;
    atlasDepthTexture: WebGLTexture;

    private pixelsPerUnit: number;

    glyphPositionsBuffer: WebGLBuffer;
    glyphTexCoordsBuffer: WebGLBuffer;
    glyphElementsBuffer: WebGLBuffer;

    appController: TextDemoController;
}

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
    ecaa: typeof ECAAStrategy;
}

class PathfinderGlyph {
    constructor(glyph: opentype.Glyph | PathfinderGlyph) {
        this.glyph = glyph instanceof PathfinderGlyph ? glyph.glyph : glyph;
        this._metrics = null;
    }

    get index(): number {
        return (this.glyph as any).index;
    }

    get metrics(): opentype.Metrics {
        if (this._metrics == null)
            this._metrics = this.glyph.getMetrics();
        return this._metrics;
    }

    get advanceWidth(): number {
        return this.glyph.advanceWidth;
    }

    private glyph: opentype.Glyph;
    private _metrics: opentype.Metrics | null;
}

class TextGlyph extends PathfinderGlyph {
    constructor(glyph: opentype.Glyph | PathfinderGlyph) {
        super(glyph);
        this._canvasRect = glmatrix.vec4.create();
    }

    get canvasRect() {
        return this._canvasRect;
    }

    set canvasRect(rect: Rect) {
        this._canvasRect = rect;
    }

    private _canvasRect: Rect;
}

class AtlasGlyph extends PathfinderGlyph {
    constructor(glyph: opentype.Glyph | PathfinderGlyph) {
        super(glyph);
        this._atlasRect = glmatrix.vec4.create();
    }

    get atlasRect() {
        return this._atlasRect;
    }

    set atlasRect(rect: Rect) {
        this._atlasRect = rect;
    }

    get atlasSize(): Size2D {
        let atlasSize = glmatrix.vec2.create();
        glmatrix.vec2.sub(atlasSize,
                          this._atlasRect.slice(2, 4) as glmatrix.vec2,
                          this._atlasRect.slice(0, 2) as glmatrix.vec2);
        return atlasSize;
    }

    private _atlasRect: Rect;
}

class Atlas {
    constructor() {
        this._texture = null;
        this._usedSize = glmatrix.vec2.create();
    }

    layoutGlyphs(glyphs: AtlasGlyph[], fontSize: number, unitsPerEm: number) {
        const pixelsPerUnit = fontSize / unitsPerEm;

        let nextOrigin = glmatrix.vec2.create();
        let shelfBottom = 0.0;

        for (const glyph of glyphs) {
            const metrics = glyph.metrics;

            const glyphSize = glmatrix.vec2.fromValues(metrics.xMax - metrics.xMin,
                                                       metrics.yMax - metrics.yMin);
            glmatrix.vec2.scale(glyphSize, glyphSize, pixelsPerUnit);
            glmatrix.vec2.ceil(glyphSize, glyphSize);

            // Make a new shelf if necessary.
            const initialGlyphRight = nextOrigin[0] + glyphSize[0] + 2;
            if (initialGlyphRight > ATLAS_SIZE[0])
                nextOrigin = glmatrix.vec2.fromValues(0.0, shelfBottom);

            const glyphRect = glmatrix.vec4.fromValues(nextOrigin[0] + 1,
                                                       nextOrigin[1] + 1,
                                                       nextOrigin[0] + glyphSize[0] + 1,
                                                       nextOrigin[1] + glyphSize[1] + 1);

            glyph.atlasRect = glyphRect;

            nextOrigin[0] = glyphRect[2] + 1;
            shelfBottom = Math.max(shelfBottom, glyphRect[3]);
        }

        // FIXME(pcwalton): Could be more precise if we don't have a full row.
        this._usedSize = glmatrix.vec2.fromValues(ATLAS_SIZE[0], shelfBottom);
    }

    ensureTexture(gl: WebGLRenderingContext): WebGLTexture {
        if (this._texture != null)
            return this._texture;

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

    private _texture: WebGLTexture | null;
    private _usedSize: Size2D;
}

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
    ecaa: ECAAMonochromeStrategy,
};

function main() {
    const controller = new TextDemoController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
