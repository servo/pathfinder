// pathfinder/client/src/text-demo.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {Font} from 'opentype.js';
import * as _ from 'lodash';
import * as base64js from 'base64-js';
import * as glmatrix from 'gl-matrix';
import * as opentype from 'opentype.js';

import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from './aa-strategy';
import {DemoAppController} from './app-controller';
import {OrthographicCamera} from "./camera";
import {ECAAMonochromeStrategy, ECAAStrategy} from './ecaa-strategy';
import {createFramebuffer, createFramebufferColorTexture} from './gl-utils';
import {createFramebufferDepthTexture, QUAD_ELEMENTS, setTextureParameters} from './gl-utils';
import {UniformMap} from './gl-utils';
import {PathfinderMeshBuffers, PathfinderMeshData} from './meshes';
import {PathfinderShaderProgram, ShaderMap, ShaderProgramSource} from './shader-loader';
import {BUILTIN_FONT_URI, Hint, PathfinderGlyph, SimpleTextLayout} from "./text";
import {PathfinderError, UINT32_SIZE, assert, expectNotNull, scaleRect, panic} from './utils';
import {unwrapNull} from './utils';
import { MonochromePathfinderView, Timings, TIMINGS } from './view';
import PathfinderBufferTexture from './buffer-texture';
import SSAAStrategy from './ssaa-strategy';

const DEFAULT_TEXT: string =
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

const DEFAULT_FONT: string = 'open-sans';

const B_POSITION_SIZE: number = 8;

const B_PATH_INDEX_SIZE: number = 2;

const ATLAS_SIZE: glmatrix.vec2 = glmatrix.vec2.fromValues(2048, 4096);

const MIN_SCALE: number = 0.02;
const MAX_SCALE: number = 10.0;

declare global {
    interface Window {
        jQuery(element: HTMLElement): JQuerySubset;
    }
}

interface JQuerySubset {
    modal(options?: any): void;
}

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
        lineHeight(): number;
    }
    interface Glyph {
        getIndex(): number;
    }
}

/// The separating axis theorem.
function rectsIntersect(a: glmatrix.vec4, b: glmatrix.vec4): boolean {
    return a[2] > b[0] && a[3] > b[1] && a[0] < b[2] && a[1] < b[3];
}

class TextDemoController extends DemoAppController<TextDemoView> {
    constructor() {
        super();
        this.text = DEFAULT_TEXT;
        this._atlas = new Atlas;
    }

    start() {
        super.start();

        this._fontSize = INITIAL_FONT_SIZE;

        this.hintingSelect = unwrapNull(document.getElementById('pf-hinting-select')) as
            HTMLSelectElement;
        this.hintingSelect.addEventListener('change', () => this.hintingChanged(), false);

        this.editTextModal = unwrapNull(document.getElementById('pf-edit-text-modal'));
        this.editTextArea = unwrapNull(document.getElementById('pf-edit-text-area')) as
            HTMLTextAreaElement;

        const editTextOkButton = unwrapNull(document.getElementById('pf-edit-text-ok-button'));
        editTextOkButton.addEventListener('click', () => this.updateText(), false);

        this.loadInitialFile();
    }

    showTextEditor() {
        this.editTextArea.value = this.text;

        window.jQuery(this.editTextModal).modal();
    }

    private hintingChanged(): void {
        this.view.then(view => view.updateHinting());
    }

    private updateText(): void {
        this.text = this.editTextArea.value;
        this.recreateLayout();

        window.jQuery(this.editTextModal).modal('hide');
    }

    protected createView() {
        return new TextDemoView(this,
                                unwrapNull(this.commonShaderSource),
                                unwrapNull(this.shaderSources));
    }

    protected fileLoaded() {
        this.recreateLayout();
    }

    private recreateLayout() {
        this.layout = new SimpleTextLayout(this.fileData,
                                           this.text,
                                           glyph => new GlyphInstance(glyph));
        this.layout.glyphStorage.partition().then((meshes: PathfinderMeshData) => {
            this.meshes = meshes;
            this.view.then(view => {
                view.attachText();
                view.uploadPathColors(1);
                view.attachMeshes([this.meshes]);
            });
        });
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
        this.view.then(view => view.relayoutText());
    }

    get pixelsPerUnit(): number {
        return this._fontSize / this.layout.glyphStorage.font.unitsPerEm;
    }

    get useHinting(): boolean {
        return this.hintingSelect.selectedIndex !== 0;
    }

    createHint(): Hint {
        return new Hint(this.layout.glyphStorage.font, this.pixelsPerUnit, this.useHinting);
    }

    protected get builtinFileURI(): string {
        return BUILTIN_FONT_URI;
    }

    protected get defaultFile(): string {
        return DEFAULT_FONT;
    }

    private hintingSelect: HTMLSelectElement;

    private editTextModal: HTMLElement;
    private editTextArea: HTMLTextAreaElement;

    private _atlas: Atlas;
    atlasGlyphs: AtlasGlyph[];

    private meshes: PathfinderMeshData;

    private _fontSize: number;

    private text: string;

    layout: SimpleTextLayout<GlyphInstance>;
}

class TextDemoView extends MonochromePathfinderView {
    constructor(appController: TextDemoController,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(commonShaderSource, shaderSources);

        this.appController = appController;

        this.camera = new OrthographicCamera(this.canvas, {
            minScale: MIN_SCALE,
            maxScale: MAX_SCALE,
        });
        this.camera.onPan = () => this.onPan();
        this.camera.onZoom = () => this.onZoom();

        this.canvas.addEventListener('dblclick', () => this.appController.showTextEditor(), false);
    }

    protected initContext() {
        super.initContext();
    }

    protected pathColorsForObject(objectIndex: number): Uint8Array {
        const atlasGlyphs = this.appController.atlasGlyphs;
        const pathCount = atlasGlyphs.length;

        const pathColors = new Uint8Array(4 * (pathCount + 1));

        for (let pathIndex = 0; pathIndex < pathCount; pathIndex++) {
            for (let channel = 0; channel < 3; channel++)
                pathColors[(pathIndex + 1) * 4 + channel] = 0x00; // RGB
            pathColors[(pathIndex + 1) * 4 + 3] = 0xff;           // alpha
        }

        return pathColors;
    }

    /// Lays out glyphs on the canvas.
    private layoutText() {
        const layout = this.appController.layout;
        layout.layoutRuns();

        let textBounds = layout.textFrame.bounds;
        textBounds = scaleRect(textBounds, this.appController.pixelsPerUnit);
        this.camera.bounds = textBounds;

        const textGlyphs = layout.glyphStorage.allGlyphs;
        const glyphPositions = new Float32Array(textGlyphs.length * 8);
        const glyphIndices = new Uint32Array(textGlyphs.length * 6);

        for (let glyphIndex = 0; glyphIndex < textGlyphs.length; glyphIndex++) {
            const textGlyph = textGlyphs[glyphIndex];
            const rect = textGlyph.pixelRect(this.appController.createHint(),
                                             this.appController.pixelsPerUnit);
            glyphPositions.set([
                rect[0], rect[3],
                rect[2], rect[3],
                rect[0], rect[1],
                rect[2], rect[1],
            ], glyphIndex * 8);

            for (let glyphIndexIndex = 0;
                 glyphIndexIndex < QUAD_ELEMENTS.length;
                 glyphIndexIndex++) {
                glyphIndices[glyphIndexIndex + glyphIndex * 6] =
                    QUAD_ELEMENTS[glyphIndexIndex] + 4 * glyphIndex;
            }
        }

        this.glyphPositionsBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.glyphPositionsBuffer);
        this.gl.bufferData(this.gl.ARRAY_BUFFER, glyphPositions, this.gl.STATIC_DRAW);
        this.glyphElementsBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.glyphElementsBuffer);
        this.gl.bufferData(this.gl.ELEMENT_ARRAY_BUFFER, glyphIndices, this.gl.STATIC_DRAW);
    }

    private buildAtlasGlyphs() {
        const textGlyphs = this.appController.layout.glyphStorage.allGlyphs;
        const pixelsPerUnit = this.appController.pixelsPerUnit;

        const hint = this.appController.createHint();

        // Only build glyphs in view.
        const translation = this.camera.translation;
        const canvasRect = glmatrix.vec4.fromValues(-translation[0],
                                                    -translation[1],
                                                    -translation[0] + this.canvas.width,
                                                    -translation[1] + this.canvas.height);

        let atlasGlyphs =
            textGlyphs.filter(glyph => {
                        return rectsIntersect(glyph.pixelRect(hint, pixelsPerUnit), canvasRect);
                      }).map(textGlyph => new AtlasGlyph(textGlyph.opentypeGlyph));
        atlasGlyphs.sort((a, b) => a.index - b.index);
        atlasGlyphs = _.sortedUniqBy(atlasGlyphs, glyph => glyph.index);
        this.appController.atlasGlyphs = atlasGlyphs;

        this.appController.atlas.layoutGlyphs(atlasGlyphs, hint, pixelsPerUnit);

        const uniqueGlyphs = this.appController.layout.glyphStorage.uniqueGlyphs;
        const uniqueGlyphIndices = uniqueGlyphs.map(glyph => glyph.index);
        uniqueGlyphIndices.sort((a, b) => a - b);

        this.uploadPathTransforms(1);

        // TODO(pcwalton): Regenerate the IBOs to include only the glyphs we care about.
        const pathHints = new Float32Array((uniqueGlyphs.length + 1) * 4);

        for (let glyphIndex = 0; glyphIndex < atlasGlyphs.length; glyphIndex++) {
            const glyph = atlasGlyphs[glyphIndex];

            let pathID = _.sortedIndexOf(uniqueGlyphIndices, glyph.index);
            assert(pathID >= 0, "No path ID!");
            pathID++;

            pathHints[pathID * 4 + 0] = hint.xHeight;
            pathHints[pathID * 4 + 1] = hint.hintedXHeight;
        }

        const pathHintsBufferTexture = new PathfinderBufferTexture(this.gl, 'uPathHints');
        pathHintsBufferTexture.upload(this.gl, pathHints);
        this.pathHintsBufferTexture = pathHintsBufferTexture;
    }

    protected pathTransformsForObject(objectIndex: number): Float32Array {
        const atlasGlyphs = this.appController.atlasGlyphs;
        const pixelsPerUnit = this.appController.pixelsPerUnit;

        const uniqueGlyphs = this.appController.layout.glyphStorage.uniqueGlyphs;
        const uniqueGlyphIndices = uniqueGlyphs.map(glyph => glyph.index);
        uniqueGlyphIndices.sort((a, b) => a - b);

        const transforms = new Float32Array((uniqueGlyphs.length + 1) * 4);

        for (let glyphIndex = 0; glyphIndex < atlasGlyphs.length; glyphIndex++) {
            const glyph = atlasGlyphs[glyphIndex];

            let pathID = _.sortedIndexOf(uniqueGlyphIndices, glyph.index);
            assert(pathID >= 0, "No path ID!");
            pathID++;

            const atlasOrigin = glyph.pixelOrigin(pixelsPerUnit);
            const metrics = glyph.metrics;
            transforms[pathID * 4 + 0] = pixelsPerUnit;
            transforms[pathID * 4 + 1] = pixelsPerUnit;
            transforms[pathID * 4 + 2] = atlasOrigin[0];
            transforms[pathID * 4 + 3] = atlasOrigin[1];
        }

        return transforms;
    }

    private createAtlasFramebuffer() {
        const atlasColorTexture = this.appController.atlas.ensureTexture(this.gl);
        this.atlasDepthTexture = createFramebufferDepthTexture(this.gl, ATLAS_SIZE);
        this.atlasFramebuffer = createFramebuffer(this.gl,
                                                    this.drawBuffersExt,
                                                    [atlasColorTexture],
                                                    this.atlasDepthTexture);

        // Allow the antialiasing strategy to set up framebuffers as necessary.
        if (this.antialiasingStrategy != null)
            this.antialiasingStrategy.setFramebufferSize(this);
    }

    private setGlyphTexCoords() {
        const textGlyphs = this.appController.layout.glyphStorage.allGlyphs;
        const atlasGlyphs = this.appController.atlasGlyphs;

        const hint = this.appController.createHint();

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
            const atlasGlyphRect = atlasGlyph.pixelRect(hint, this.appController.pixelsPerUnit);
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
        this.relayoutText();
        this.camera.zoomToFit();
    }

    relayoutText() {
        this.layoutText();
        this.rebuildAtlasIfNecessary();
    }

    private rebuildAtlasIfNecessary() {
        this.buildAtlasGlyphs();
        this.setGlyphTexCoords();
        this.setDirty();
    }

    protected onPan() {
        this.setDirty();
        this.rebuildAtlasIfNecessary();
    }

    protected onZoom() {
        this.appController.fontSize = this.camera.scale * INITIAL_FONT_SIZE;
        this.setDirty();
        this.rebuildAtlasIfNecessary();
    }

    updateHinting(): void {
        this.setDirty();
        this.rebuildAtlasIfNecessary();
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
        this.gl.disable(this.gl.SCISSOR_TEST);
        this.gl.blendEquation(this.gl.FUNC_ADD);
        this.gl.blendFuncSeparate(this.gl.SRC_ALPHA, this.gl.ONE_MINUS_SRC_ALPHA,
                                  this.gl.ONE, this.gl.ONE);
        this.gl.enable(this.gl.BLEND);

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
                                [this.camera.translation[0], this.camera.translation[1], 0.0]);

        // Blit.
        this.gl.uniformMatrix4fv(blitProgram.uniforms.uTransform, false, transform);
        this.gl.activeTexture(this.gl.TEXTURE0);
        this.gl.bindTexture(this.gl.TEXTURE_2D, this.appController.atlas.ensureTexture(this.gl));
        this.gl.uniform1i(blitProgram.uniforms.uSource, 0);
        this.setIdentityTexScaleUniform(blitProgram.uniforms);
        this.gl.drawElements(this.gl.TRIANGLES,
                             this.appController.layout.glyphStorage.allGlyphs.length * 6,
                             this.gl.UNSIGNED_INT,
                             0);
    }

    protected clearForDirectRendering(): void {
        this.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        this.gl.clearDepth(0.0);
        this.gl.depthMask(true);
        this.gl.clear(this.gl.COLOR_BUFFER_BIT | this.gl.DEPTH_BUFFER_BIT);
    }

    readonly bgColor: glmatrix.vec4 = glmatrix.vec4.fromValues(1.0, 1.0, 1.0, 0.0);
    readonly fgColor: glmatrix.vec4 = glmatrix.vec4.fromValues(0.0, 0.0, 0.0, 1.0);

    get destFramebuffer(): WebGLFramebuffer {
        return this.atlasFramebuffer;
    }

    get destAllocatedSize(): glmatrix.vec2 {
        return ATLAS_SIZE;
    }

    get destUsedSize(): glmatrix.vec2 {
        return this.appController.atlas.usedSize;
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName,
                               aaLevel: number,
                               subpixelAA: boolean):
                               AntialiasingStrategy {
        return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel, subpixelAA);
    }

    protected newTimingsReceived() {
        this.appController.newTimingsReceived(this.lastTimings);
    }

    protected get worldTransform(): glmatrix.mat4 {
        const transform = glmatrix.mat4.create();
        glmatrix.mat4.translate(transform, transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform, transform, [2.0 / ATLAS_SIZE[0], 2.0 / ATLAS_SIZE[1], 1.0]);
        return transform;
    }

    protected get directCurveProgramName(): keyof ShaderMap<void> {
        return 'directCurve';
    }

    protected get directInteriorProgramName(): keyof ShaderMap<void> {
        return 'directInterior';
    }

    protected depthFunction: number = this.gl.GREATER;

    atlasFramebuffer: WebGLFramebuffer;
    atlasDepthTexture: WebGLTexture;

    glyphPositionsBuffer: WebGLBuffer;
    glyphTexCoordsBuffer: WebGLBuffer;
    glyphElementsBuffer: WebGLBuffer;

    appController: TextDemoController;

    camera: OrthographicCamera;
}

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
    ecaa: typeof ECAAStrategy;
}

class Atlas {
    constructor() {
        this._texture = null;
        this._usedSize = glmatrix.vec2.create();
    }

    layoutGlyphs(glyphs: AtlasGlyph[], hint: Hint, pixelsPerUnit: number) {
        let nextOrigin = glmatrix.vec2.fromValues(1.0, 1.0);
        let shelfBottom = 2.0;

        for (const glyph of glyphs) {
            // Place the glyph, and advance the origin.
            glyph.setPixelLowerLeft(nextOrigin, pixelsPerUnit);
            nextOrigin[0] = glyph.pixelRect(hint, pixelsPerUnit)[2] + 1.0;

            // If the glyph overflowed the shelf, make a new one and reposition the glyph.
            if (nextOrigin[0] > ATLAS_SIZE[0]) {
                nextOrigin = glmatrix.vec2.fromValues(1.0, shelfBottom + 1.0);
                glyph.setPixelLowerLeft(nextOrigin, pixelsPerUnit);
                nextOrigin[0] = glyph.pixelRect(hint, pixelsPerUnit)[2] + 1.0;
            }

            // Grow the shelf as necessary.
            shelfBottom = Math.max(shelfBottom, glyph.pixelRect(hint, pixelsPerUnit)[3] + 1.0);
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

class AtlasGlyph extends PathfinderGlyph {
    constructor(glyph: opentype.Glyph) {
        super(glyph);
    }
}

class GlyphInstance extends PathfinderGlyph {
    constructor(glyph: opentype.Glyph) {
        super(glyph);
    }
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
