// pathfinder/client/src/text-demo.ts
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
import * as opentype from 'opentype.js';

import {Metrics} from 'opentype.js';
import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from "./aa-strategy";
import {StemDarkeningMode, SubpixelAAType} from './aa-strategy';
import {DemoAppController} from './app-controller';
import PathfinderBufferTexture from './buffer-texture';
import {OrthographicCamera} from "./camera";
import {createFramebuffer, createFramebufferColorTexture} from './gl-utils';
import {createFramebufferDepthTexture, QUAD_ELEMENTS, setTextureParameters} from './gl-utils';
import {UniformMap} from './gl-utils';
import {PathfinderMeshBuffers, PathfinderMeshData} from './meshes';
import {Renderer} from './renderer';
import {PathfinderShaderProgram, ShaderMap, ShaderProgramSource} from './shader-loader';
import SSAAStrategy from './ssaa-strategy';
import {calculatePixelDescent, calculatePixelRectForGlyph, PathfinderFont} from "./text";
import {BUILTIN_FONT_URI, calculatePixelXMin, computeStemDarkeningAmount} from "./text";
import {GlyphStore, Hint, SimpleTextLayout, UnitMetrics} from "./text";
import {assert, expectNotNull, panic, PathfinderError, scaleRect, UINT32_SIZE} from './utils';
import {unwrapNull} from './utils';
import {DemoView, Timings, TIMINGS} from './view';
import {AdaptiveMonochromeXCAAStrategy} from './xcaa-strategy';

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

const SUBPIXEL_GRANULARITY: number = 4;

const ATLAS_SIZE: glmatrix.vec2 = glmatrix.vec2.fromValues(2048, 4096);

const MIN_SCALE: number = 0.0025;
const MAX_SCALE: number = 0.5;

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
    font: PathfinderFont;
    layout: SimpleTextLayout;
    glyphStore: GlyphStore;
    atlasGlyphs: AtlasGlyph[];

    private hintingSelect: HTMLSelectElement;

    private editTextModal: HTMLElement;
    private editTextArea: HTMLTextAreaElement;

    private _atlas: Atlas;

    private meshes: PathfinderMeshData;

    private _fontSize: number;

    private text: string;

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

        this.loadInitialFile(this.builtinFileURI);
    }

    showTextEditor() {
        this.editTextArea.value = this.text;

        window.jQuery(this.editTextModal).modal();
    }

    protected createView() {
        return new TextDemoView(this,
                                unwrapNull(this.commonShaderSource),
                                unwrapNull(this.shaderSources));
    }

    protected fileLoaded(fileData: ArrayBuffer, builtinName: string | null) {
        const font = new PathfinderFont(fileData, builtinName);
        this.recreateLayout(font);
    }

    private hintingChanged(): void {
        this.view.then(view => view.renderer.updateHinting());
    }

    private updateText(): void {
        this.text = this.editTextArea.value;

        window.jQuery(this.editTextModal).modal('hide');
    }

    private recreateLayout(font: PathfinderFont) {
        const newLayout = new SimpleTextLayout(font, this.text);

        let uniqueGlyphIDs = newLayout.textFrame.allGlyphIDs;
        uniqueGlyphIDs.sort((a, b) => a - b);
        uniqueGlyphIDs = _.sortedUniq(uniqueGlyphIDs);

        const glyphStore = new GlyphStore(font, uniqueGlyphIDs);
        glyphStore.partition().then(result => {
            const meshes = this.expandMeshes(result.meshes, uniqueGlyphIDs.length);

            this.view.then(view => {
                this.font = font;
                this.layout = newLayout;
                this.glyphStore = glyphStore;
                this.meshes = meshes;

                view.attachText();
                view.renderer.uploadPathColors(1);
                view.attachMeshes([this.meshes]);
            });
        });
    }

    private expandMeshes(meshes: PathfinderMeshData, glyphCount: number): PathfinderMeshData {
        const pathIDs = [];
        for (let glyphIndex = 0; glyphIndex < glyphCount; glyphIndex++) {
            for (let subpixel = 0; subpixel < SUBPIXEL_GRANULARITY; subpixel++)
                pathIDs.push(glyphIndex + 1);
        }
        return meshes.expand(pathIDs);
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
        this.view.then(view => view.renderer.relayoutText());
    }

    get layoutPixelsPerUnit(): number {
        return this._fontSize / this.font.opentypeFont.unitsPerEm;
    }

    get useHinting(): boolean {
        return this.hintingSelect.selectedIndex !== 0;
    }

    get pathCount(): number {
        return this.glyphStore.glyphIDs.length * SUBPIXEL_GRANULARITY;
    }

    protected get builtinFileURI(): string {
        return BUILTIN_FONT_URI;
    }

    protected get defaultFile(): string {
        return DEFAULT_FONT;
    }
}

class TextDemoView extends DemoView {
    renderer: TextDemoRenderer;

    appController: TextDemoController;

    protected get camera(): OrthographicCamera {
        return this.renderer.camera;
    }

    constructor(appController: TextDemoController,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(commonShaderSource, shaderSources);

        this.appController = appController;
        this.renderer = new TextDemoRenderer(this);

        this.canvas.addEventListener('dblclick', () => this.appController.showTextEditor(), false);

        this.resizeToFit(true);
    }

    attachText() {
        this.panZoomEventsEnabled = false;
        this.renderer.prepareToAttachText();
        this.renderer.camera.zoomToFit();
        this.appController.fontSize = this.renderer.camera.scale *
            this.appController.font.opentypeFont.unitsPerEm;
        this.renderer.finishAttachingText();
        this.panZoomEventsEnabled = true;
    }

    protected onPan() {
        this.renderer.viewPanned();
    }

    protected onZoom() {
        this.appController.fontSize = this.renderer.camera.scale *
            this.appController.font.opentypeFont.unitsPerEm;
    }

    private set panZoomEventsEnabled(flag: boolean) {
        if (flag) {
            this.renderer.camera.onPan = () => this.onPan();
            this.renderer.camera.onZoom = () => this.onZoom();
        } else {
            this.renderer.camera.onPan = null;
            this.renderer.camera.onZoom = null;
        }
    }
}

class TextDemoRenderer extends Renderer {
    renderContext: TextDemoView;

    camera: OrthographicCamera;

    atlasFramebuffer: WebGLFramebuffer;
    atlasDepthTexture: WebGLTexture;

    glyphPositionsBuffer: WebGLBuffer;
    glyphTexCoordsBuffer: WebGLBuffer;
    glyphElementsBuffer: WebGLBuffer;

    get destFramebuffer(): WebGLFramebuffer {
        return this.atlasFramebuffer;
    }

    get destAllocatedSize(): glmatrix.vec2 {
        return ATLAS_SIZE;
    }

    get destUsedSize(): glmatrix.vec2 {
        return this.renderContext.appController.atlas.usedSize;
    }

    get emboldenAmount(): glmatrix.vec2 {
        return this.stemDarkeningAmount;
    }

    get bgColor(): glmatrix.vec4 {
        return glmatrix.vec4.fromValues(1.0, 1.0, 1.0, 0.0);
    }

    get fgColor(): glmatrix.vec4 {
        return glmatrix.vec4.fromValues(0.0, 0.0, 0.0, 1.0);
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

    protected get depthFunction(): number {
        return this.renderContext.gl.GREATER;
    }

    protected get usedSizeFactor(): glmatrix.vec2 {
        const usedSize = glmatrix.vec2.create();
        glmatrix.vec2.div(usedSize, this.renderContext.appController.atlas.usedSize, ATLAS_SIZE);
        return usedSize;
    }

    private get stemDarkeningAmount(): glmatrix.vec2 {
        const appController = this.renderContext.appController;
        if (this.stemDarkening === 'dark') {
            return computeStemDarkeningAmount(appController.fontSize,
                                              appController.layoutPixelsPerUnit);
        }
        return glmatrix.vec2.create();
    }

    private glyphBounds: Float32Array;
    private stemDarkening: StemDarkeningMode;
    private subpixelAA: SubpixelAAType;

    private get displayPixelsPerUnit(): number {
        return this.renderContext.appController.layoutPixelsPerUnit;
    }

    constructor(renderContext: TextDemoView) {
        super(renderContext);

        this.camera = new OrthographicCamera(this.renderContext.canvas, {
            maxScale: MAX_SCALE,
            minScale: MIN_SCALE,
        });
    }

    setAntialiasingOptions(aaType: AntialiasingStrategyName,
                           aaLevel: number,
                           subpixelAA: SubpixelAAType,
                           stemDarkening: StemDarkeningMode) {
        super.setAntialiasingOptions(aaType, aaLevel, subpixelAA, stemDarkening);

        // Need to relayout because changing AA options can cause font dilation to change...
        this.layoutText();
        this.buildAtlasGlyphs();
        this.renderContext.setDirty();
    }

    setHintsUniform(uniforms: UniformMap): void {
        const hint = this.createHint();
        this.renderContext.gl.uniform4f(uniforms.uHints,
                                        hint.xHeight,
                                        hint.hintedXHeight,
                                        hint.stemHeight,
                                        hint.hintedStemHeight);
    }

    prepareToAttachText(): void {
        if (this.atlasFramebuffer == null)
            this.createAtlasFramebuffer();

        this.layoutText();
    }

    finishAttachingText(): void {
        this.buildAtlasGlyphs();
        this.renderContext.setDirty();
    }

    relayoutText(): void {
        this.layoutText();
        this.buildAtlasGlyphs();
        this.renderContext.setDirty();
    }

    updateHinting(): void {
        // Need to relayout the text because the pixel bounds of the glyphs can change from this...
        this.layoutText();
        this.buildAtlasGlyphs();
        this.renderContext.setDirty();
    }

    viewPanned(): void {
        this.buildAtlasGlyphs();
        this.renderContext.setDirty();
    }

    pathBoundingRects(objectIndex: number): Float32Array {
        const pathCount = this.renderContext.appController.pathCount;
        const atlasGlyphs = this.renderContext.appController.atlasGlyphs;
        const pixelsPerUnit = this.displayPixelsPerUnit;
        const font = this.renderContext.appController.font;
        const hint = this.createHint();

        const boundingRects = new Float32Array((pathCount + 1) * 4);

        for (const glyph of atlasGlyphs) {
            const atlasGlyphMetrics = font.metricsForGlyph(glyph.glyphKey.id);
            if (atlasGlyphMetrics == null)
                continue;
            const atlasUnitMetrics = new UnitMetrics(atlasGlyphMetrics, this.stemDarkeningAmount);

            const pathID = glyph.pathID;
            boundingRects[pathID * 4 + 0] = atlasUnitMetrics.left;
            boundingRects[pathID * 4 + 1] = atlasUnitMetrics.descent;
            boundingRects[pathID * 4 + 2] = atlasUnitMetrics.right;
            boundingRects[pathID * 4 + 3] = atlasUnitMetrics.ascent;
        }

        return boundingRects;
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName,
                               aaLevel: number,
                               subpixelAA: SubpixelAAType,
                               stemDarkening: StemDarkeningMode):
                               AntialiasingStrategy {
        this.subpixelAA = subpixelAA;
        this.stemDarkening = stemDarkening;
        return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel, subpixelAA);
    }

    protected clearForDirectRendering(): void {
        this.renderContext.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        this.renderContext.gl.clearDepth(0.0);
        this.renderContext.gl.depthMask(true);
        this.renderContext.gl.clear(this.renderContext.gl.COLOR_BUFFER_BIT |
                                    this.renderContext.gl.DEPTH_BUFFER_BIT);
    }

    protected compositeIfNecessary() {
        // Set up composite state.
        this.renderContext.gl.bindFramebuffer(this.renderContext.gl.FRAMEBUFFER, null);
        this.renderContext.gl.viewport(0, 0, this.renderContext.canvas.width, this.renderContext.canvas.height);
        this.renderContext.gl.disable(this.renderContext.gl.DEPTH_TEST);
        this.renderContext.gl.disable(this.renderContext.gl.SCISSOR_TEST);
        this.renderContext.gl.blendEquation(this.renderContext.gl.FUNC_ADD);
        this.renderContext.gl.blendFuncSeparate(this.renderContext.gl.SRC_ALPHA,
                                                this.renderContext.gl.ONE_MINUS_SRC_ALPHA,
                                                this.renderContext.gl.ONE,
                                                this.renderContext.gl.ONE);
        this.renderContext.gl.enable(this.renderContext.gl.BLEND);

        // Clear.
        this.renderContext.gl.clearColor(1.0, 1.0, 1.0, 1.0);
        this.renderContext.gl.clear(this.renderContext.gl.COLOR_BUFFER_BIT);

        // Set up the composite VAO.
        const blitProgram = this.renderContext.shaderPrograms.blit;
        const attributes = blitProgram.attributes;
        this.renderContext.gl.useProgram(blitProgram.program);
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ARRAY_BUFFER,
                                         this.glyphPositionsBuffer);
        this.renderContext.gl.vertexAttribPointer(attributes.aPosition,
                                                  2,
                                                  this.renderContext.gl.FLOAT,
                                                  false,
                                                  0,
                                                  0);
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ARRAY_BUFFER,
                                         this.glyphTexCoordsBuffer);
        this.renderContext.gl.vertexAttribPointer(attributes.aTexCoord,
                                                  2,
                                                  this.renderContext.gl.FLOAT,
                                                  false,
                                                  0,
                                                  0);
        this.renderContext.gl.enableVertexAttribArray(attributes.aPosition);
        this.renderContext.gl.enableVertexAttribArray(attributes.aTexCoord);
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ELEMENT_ARRAY_BUFFER,
                                         this.glyphElementsBuffer);

        // Create the transform.
        const transform = glmatrix.mat4.create();
        glmatrix.mat4.fromTranslation(transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform, transform, [
            2.0 / this.renderContext.canvas.width,
            2.0 / this.renderContext.canvas.height,
            1.0,
        ]);
        glmatrix.mat4.translate(transform,
                                transform,
                                [this.camera.translation[0], this.camera.translation[1], 0.0]);

        // Blit.
        this.renderContext.gl.uniformMatrix4fv(blitProgram.uniforms.uTransform, false, transform);
        this.renderContext.gl.activeTexture(this.renderContext.gl.TEXTURE0);
        const destTexture = this.renderContext
                                .appController
                                .atlas
                                .ensureTexture(this.renderContext);
        this.renderContext.gl.bindTexture(this.renderContext.gl.TEXTURE_2D, destTexture);
        this.renderContext.gl.uniform1i(blitProgram.uniforms.uSource, 0);
        this.setIdentityTexScaleUniform(blitProgram.uniforms);
        const totalGlyphCount = this.renderContext.appController.layout.textFrame.totalGlyphCount;
        this.renderContext.gl.drawElements(this.renderContext.gl.TRIANGLES,
                                           totalGlyphCount * 6,
                                           this.renderContext.gl.UNSIGNED_INT,
                                           0);
    }

    protected pathColorsForObject(objectIndex: number): Uint8Array {
        const pathCount = this.renderContext.appController.pathCount;

        const pathColors = new Uint8Array(4 * (pathCount + 1));

        for (let pathIndex = 0; pathIndex < pathCount; pathIndex++) {
            for (let channel = 0; channel < 3; channel++)
                pathColors[(pathIndex + 1) * 4 + channel] = 0x00; // RGB
            pathColors[(pathIndex + 1) * 4 + 3] = 0xff;           // alpha
        }

        return pathColors;
    }

    protected pathTransformsForObject(objectIndex: number): Float32Array {
        const pathCount = this.renderContext.appController.pathCount;
        const atlasGlyphs = this.renderContext.appController.atlasGlyphs;
        const pixelsPerUnit = this.displayPixelsPerUnit;

        const transforms = new Float32Array((pathCount + 1) * 4);

        for (const glyph of atlasGlyphs) {
            const pathID = glyph.pathID;
            const atlasOrigin = glyph.calculateSubpixelOrigin(pixelsPerUnit);

            transforms[pathID * 4 + 0] = pixelsPerUnit;
            transforms[pathID * 4 + 1] = pixelsPerUnit;
            transforms[pathID * 4 + 2] = atlasOrigin[0];
            transforms[pathID * 4 + 3] = atlasOrigin[1];
        }

        return transforms;
    }

    protected newTimingsReceived() {
        this.renderContext.appController.newTimingsReceived(this.lastTimings);
    }

    private createAtlasFramebuffer() {
        const appController = this.renderContext.appController;

        const atlasColorTexture = appController.atlas.ensureTexture(this.renderContext);
        this.atlasDepthTexture = createFramebufferDepthTexture(this.renderContext.gl, ATLAS_SIZE);
        this.atlasFramebuffer = createFramebuffer(this.renderContext.gl,
                                                  this.renderContext.drawBuffersExt,
                                                  [atlasColorTexture],
                                                  this.atlasDepthTexture);

        // Allow the antialiasing strategy to set up framebuffers as necessary.
        if (this.antialiasingStrategy != null)
            this.antialiasingStrategy.setFramebufferSize(this);
    }

    private createHint(): Hint {
        return new Hint(this.renderContext.appController.font,
                        this.displayPixelsPerUnit,
                        this.renderContext.appController.useHinting);
    }

    private layoutText() {
        const layout = this.renderContext.appController.layout;
        layout.layoutRuns();

        const textBounds = layout.textFrame.bounds;
        this.camera.bounds = textBounds;

        const totalGlyphCount = layout.textFrame.totalGlyphCount;
        const glyphPositions = new Float32Array(totalGlyphCount * 8);
        const glyphIndices = new Uint32Array(totalGlyphCount * 6);

        const hint = this.createHint();
        const displayPixelsPerUnit = this.displayPixelsPerUnit;
        const layoutPixelsPerUnit = this.renderContext.appController.layoutPixelsPerUnit;

        let globalGlyphIndex = 0;
        for (const run of layout.textFrame.runs) {
            for (let glyphIndex = 0;
                 glyphIndex < run.glyphIDs.length;
                 glyphIndex++, globalGlyphIndex++) {
                const rect = run.pixelRectForGlyphAt(glyphIndex,
                                                     layoutPixelsPerUnit,
                                                     displayPixelsPerUnit,
                                                     hint,
                                                     this.stemDarkeningAmount,
                                                     SUBPIXEL_GRANULARITY);
                glyphPositions.set([
                    rect[0], rect[3],
                    rect[2], rect[3],
                    rect[0], rect[1],
                    rect[2], rect[1],
                ], globalGlyphIndex * 8);

                for (let glyphIndexIndex = 0;
                    glyphIndexIndex < QUAD_ELEMENTS.length;
                    glyphIndexIndex++) {
                    glyphIndices[glyphIndexIndex + globalGlyphIndex * 6] =
                        QUAD_ELEMENTS[glyphIndexIndex] + 4 * globalGlyphIndex;
                }
            }
        }

        this.glyphPositionsBuffer = unwrapNull(this.renderContext.gl.createBuffer());
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ARRAY_BUFFER,
                                         this.glyphPositionsBuffer);
        this.renderContext.gl.bufferData(this.renderContext.gl.ARRAY_BUFFER,
                                         glyphPositions,
                                         this.renderContext.gl.STATIC_DRAW);
        this.glyphElementsBuffer = unwrapNull(this.renderContext.gl.createBuffer());
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ELEMENT_ARRAY_BUFFER,
                                         this.glyphElementsBuffer);
        this.renderContext.gl.bufferData(this.renderContext.gl.ELEMENT_ARRAY_BUFFER,
                                         glyphIndices,
                                         this.renderContext.gl.STATIC_DRAW);
    }

    private buildAtlasGlyphs() {
        const appController = this.renderContext.appController;
        const font = appController.font;
        const glyphStore = appController.glyphStore;
        const layoutPixelsPerUnit = appController.layoutPixelsPerUnit;
        const displayPixelsPerUnit = this.displayPixelsPerUnit;

        const textFrame = appController.layout.textFrame;
        const hint = this.createHint();

        // Only build glyphs in view.
        const translation = this.camera.translation;
        const canvasRect = glmatrix.vec4.clone([
            -translation[0],
            -translation[1],
            -translation[0] + this.renderContext.canvas.width,
            -translation[1] + this.renderContext.canvas.height,
        ]);

        let atlasGlyphs = [];
        for (const run of textFrame.runs) {
            for (let glyphIndex = 0; glyphIndex < run.glyphIDs.length; glyphIndex++) {
                const pixelRect = run.pixelRectForGlyphAt(glyphIndex,
                                                          layoutPixelsPerUnit,
                                                          displayPixelsPerUnit,
                                                          hint,
                                                          this.stemDarkeningAmount,
                                                          SUBPIXEL_GRANULARITY);
                if (!rectsIntersect(pixelRect, canvasRect))
                    continue;

                const glyphID = run.glyphIDs[glyphIndex];
                const glyphStoreIndex = glyphStore.indexOfGlyphWithID(glyphID);
                if (glyphStoreIndex == null)
                    continue;

                const subpixel = run.subpixelForGlyphAt(glyphIndex,
                                                        layoutPixelsPerUnit,
                                                        hint,
                                                        SUBPIXEL_GRANULARITY);
                const glyphKey = new GlyphKey(glyphID, subpixel);
                atlasGlyphs.push(new AtlasGlyph(glyphStoreIndex, glyphKey));
            }
        }

        atlasGlyphs.sort((a, b) => a.glyphKey.sortKey - b.glyphKey.sortKey);
        atlasGlyphs = _.sortedUniqBy(atlasGlyphs, glyph => glyph.glyphKey.sortKey);
        if (atlasGlyphs.length === 0)
            return;

        appController.atlasGlyphs = atlasGlyphs;
        appController.atlas.layoutGlyphs(atlasGlyphs,
                                         font,
                                         displayPixelsPerUnit,
                                         hint,
                                         this.stemDarkeningAmount);

        this.uploadPathTransforms(1);

        // TODO(pcwalton): Regenerate the IBOs to include only the glyphs we care about.

        this.setGlyphTexCoords();
    }

    private setGlyphTexCoords() {
        const appController = this.renderContext.appController;
        const textFrame = appController.layout.textFrame;
        const font = appController.font;
        const atlasGlyphs = appController.atlasGlyphs;

        const hint = this.createHint();
        const layoutPixelsPerUnit = appController.layoutPixelsPerUnit;
        const displayPixelsPerUnit = this.displayPixelsPerUnit;

        const atlasGlyphKeys = atlasGlyphs.map(atlasGlyph => atlasGlyph.glyphKey.sortKey);

        this.glyphBounds = new Float32Array(textFrame.totalGlyphCount * 8);

        let globalGlyphIndex = 0;
        for (const run of textFrame.runs) {
            for (let glyphIndex = 0;
                 glyphIndex < run.glyphIDs.length;
                 glyphIndex++, globalGlyphIndex++) {
                const textGlyphID = run.glyphIDs[glyphIndex];

                const subpixel = run.subpixelForGlyphAt(glyphIndex,
                                                        layoutPixelsPerUnit,
                                                        hint,
                                                        SUBPIXEL_GRANULARITY);

                const glyphKey = new GlyphKey(textGlyphID, subpixel);

                const atlasGlyphIndex = _.sortedIndexOf(atlasGlyphKeys, glyphKey.sortKey);
                if (atlasGlyphIndex < 0)
                    continue;

                // Set texture coordinates.
                const atlasGlyph = atlasGlyphs[atlasGlyphIndex];
                const atlasGlyphMetrics = font.metricsForGlyph(atlasGlyph.glyphKey.id);
                if (atlasGlyphMetrics == null)
                    continue;

                const atlasGlyphUnitMetrics = new UnitMetrics(atlasGlyphMetrics,
                                                              this.stemDarkeningAmount);

                const atlasGlyphPixelOrigin = atlasGlyph.calculateSubpixelOrigin(displayPixelsPerUnit);
                const atlasGlyphRect = calculatePixelRectForGlyph(atlasGlyphUnitMetrics,
                                                                  atlasGlyphPixelOrigin,
                                                                  displayPixelsPerUnit,
                                                                  hint);
                const atlasGlyphBL = atlasGlyphRect.slice(0, 2) as glmatrix.vec2;
                const atlasGlyphTR = atlasGlyphRect.slice(2, 4) as glmatrix.vec2;
                glmatrix.vec2.div(atlasGlyphBL, atlasGlyphBL, ATLAS_SIZE);
                glmatrix.vec2.div(atlasGlyphTR, atlasGlyphTR, ATLAS_SIZE);

                this.glyphBounds.set([
                    atlasGlyphBL[0], atlasGlyphTR[1],
                    atlasGlyphTR[0], atlasGlyphTR[1],
                    atlasGlyphBL[0], atlasGlyphBL[1],
                    atlasGlyphTR[0], atlasGlyphBL[1],
                ], globalGlyphIndex * 8);
            }
        }

        this.glyphTexCoordsBuffer = unwrapNull(this.renderContext.gl.createBuffer());
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ARRAY_BUFFER,
                                         this.glyphTexCoordsBuffer);
        this.renderContext.gl.bufferData(this.renderContext.gl.ARRAY_BUFFER,
                                         this.glyphBounds,
                                         this.renderContext.gl.STATIC_DRAW);
    }

    private setIdentityTexScaleUniform(uniforms: UniformMap) {
        this.renderContext.gl.uniform2f(uniforms.uTexScale, 1.0, 1.0);
    }
}

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
    xcaa: typeof AdaptiveMonochromeXCAAStrategy;
}

class Atlas {
    private _texture: WebGLTexture | null;
    private _usedSize: Size2D;

    constructor() {
        this._texture = null;
        this._usedSize = glmatrix.vec2.create();
    }

    layoutGlyphs(glyphs: AtlasGlyph[],
                 font: PathfinderFont, pixelsPerUnit: number, hint: Hint, stemDarkeningAmount: glmatrix.vec2) {
        let nextOrigin = glmatrix.vec2.fromValues(1.0, 1.0);
        let shelfBottom = 2.0;

        for (const glyph of glyphs) {
            // Place the glyph, and advance the origin.
            const metrics = font.metricsForGlyph(glyph.glyphKey.id);
            if (metrics == null)
                continue;

            const unitMetrics = new UnitMetrics(metrics, stemDarkeningAmount);

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

    ensureTexture(view: DemoView): WebGLTexture {
        if (this._texture != null)
            return this._texture;

        const texture = unwrapNull(view.gl.createTexture());
        this._texture = texture;
        view.gl.bindTexture(view.gl.TEXTURE_2D, texture);
        view.gl.texImage2D(view.gl.TEXTURE_2D,
                           0,
                           view.colorAlphaFormat,
                           ATLAS_SIZE[0],
                           ATLAS_SIZE[1],
                           0,
                           view.colorAlphaFormat,
                           view.gl.UNSIGNED_BYTE,
                           null);
        setTextureParameters(view.gl, view.gl.NEAREST);

        return texture;
    }

    get usedSize(): glmatrix.vec2 {
        return this._usedSize;
    }
}

class AtlasGlyph {
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
        pixelOrigin[0] += this.glyphKey.subpixel / SUBPIXEL_GRANULARITY;
        return pixelOrigin;
    }

    setPixelLowerLeft(pixelLowerLeft: glmatrix.vec2, metrics: UnitMetrics, pixelsPerUnit: number):
                      void {
        const pixelXMin = calculatePixelXMin(metrics, pixelsPerUnit);
        const pixelDescent = calculatePixelDescent(metrics, pixelsPerUnit);
        const pixelOrigin = glmatrix.vec2.clone([pixelLowerLeft[0] - pixelXMin,
                                                 pixelLowerLeft[1] + pixelDescent]);
        this.setPixelOrigin(pixelOrigin, pixelsPerUnit);
    }

    private setPixelOrigin(pixelOrigin: glmatrix.vec2, pixelsPerUnit: number): void {
        glmatrix.vec2.scale(this.origin, pixelOrigin, 1.0 / pixelsPerUnit);
    }

    get pathID(): number {
        return this.glyphStoreIndex * SUBPIXEL_GRANULARITY + this.glyphKey.subpixel + 1;
    }
}

class GlyphKey {
    readonly id: number;
    readonly subpixel: number;

    constructor(id: number, subpixel: number) {
        this.id = id;
        this.subpixel = subpixel;
    }

    get sortKey(): number {
        return this.id * SUBPIXEL_GRANULARITY + this.subpixel;
    }
}

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
    xcaa: AdaptiveMonochromeXCAAStrategy,
};

function main() {
    const controller = new TextDemoController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
