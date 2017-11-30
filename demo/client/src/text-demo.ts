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
import {AAOptions, DemoAppController} from './app-controller';
import {Atlas, ATLAS_SIZE, AtlasGlyph, GlyphKey, SUBPIXEL_GRANULARITY} from './atlas';
import PathfinderBufferTexture from './buffer-texture';
import {CameraView, OrthographicCamera} from "./camera";
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
import {TextRenderContext, TextRenderer} from './text-renderer';
import {assert, expectNotNull, panic, PathfinderError, scaleRect, UINT32_SIZE} from './utils';
import {unwrapNull} from './utils';
import {DemoView, RenderContext, Timings, TIMINGS} from './view';
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
    private _rotationAngle: number;

    private text: string;

    constructor() {
        super();
        this.text = DEFAULT_TEXT;
        this._atlas = new Atlas;
    }

    start() {
        super.start();

        this._fontSize = INITIAL_FONT_SIZE;
        this._rotationAngle = 0.0;

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

    protected createView(gammaLUT: HTMLImageElement,
                         commonShaderSource: string,
                         shaderSources: ShaderMap<ShaderProgramSource>):
                         TextDemoView {
        return new TextDemoView(this, gammaLUT, commonShaderSource, shaderSources);
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

        this.recreateLayout(this.font);
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

    get rotationAngle(): number {
        return this._rotationAngle;
    }

    set rotationAngle(newRotationAngle: number) {
        this._rotationAngle = newRotationAngle;
        this.view.then(view => view.renderer.relayoutText());
    }

    get pixelsPerUnit(): number {
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

class TextDemoView extends DemoView implements TextRenderContext {
    renderer: TextDemoRenderer;

    appController: TextDemoController;

    get cameraView(): CameraView {
        return this.canvas;
    }

    get atlasGlyphs(): AtlasGlyph[] {
        return this.appController.atlasGlyphs;
    }

    set atlasGlyphs(newAtlasGlyphs: AtlasGlyph[]) {
        this.appController.atlasGlyphs = newAtlasGlyphs;
    }

    get atlas(): Atlas {
        return this.appController.atlas;
    }

    get glyphStore(): GlyphStore {
        return this.appController.glyphStore;
    }

    get font(): PathfinderFont {
        return this.appController.font;
    }

    get fontSize(): number {
        return this.appController.fontSize;
    }

    get pathCount(): number {
        return this.appController.pathCount;
    }

    get pixelsPerUnit(): number {
        return this.appController.pixelsPerUnit;
    }

    get useHinting(): boolean {
        return this.appController.useHinting;
    }

    protected get camera(): OrthographicCamera {
        return this.renderer.camera;
    }

    constructor(appController: TextDemoController,
                gammaLUT: HTMLImageElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(gammaLUT, commonShaderSource, shaderSources);

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

    newTimingsReceived(newTimings: Timings) {
        this.appController.newTimingsReceived(newTimings);
    }

    protected onPan(): void {
        this.renderer.viewPanned();
    }

    protected onZoom(): void {
        this.appController.fontSize = this.renderer.camera.scale *
            this.appController.font.opentypeFont.unitsPerEm;
    }

    protected onRotate(): void {
        this.appController.rotationAngle = this.renderer.camera.rotationAngle;
    }

    private set panZoomEventsEnabled(flag: boolean) {
        if (flag) {
            this.renderer.camera.onPan = () => this.onPan();
            this.renderer.camera.onZoom = () => this.onZoom();
            this.renderer.camera.onRotate = () => this.onRotate();
        } else {
            this.renderer.camera.onPan = null;
            this.renderer.camera.onZoom = null;
            this.renderer.camera.onRotate = null;
        }
    }
}

class TextDemoRenderer extends TextRenderer {
    renderContext: TextDemoView;

    glyphPositionsBuffer: WebGLBuffer;
    glyphTexCoordsBuffer: WebGLBuffer;
    glyphElementsBuffer: WebGLBuffer;

    private glyphBounds: Float32Array;

    get layout(): SimpleTextLayout {
        return this.renderContext.appController.layout;
    }

    get backgroundColor(): glmatrix.vec4 {
        return glmatrix.vec4.create();
    }

    get rotationAngle(): number {
        return this.renderContext.appController.rotationAngle;
    }

    prepareToAttachText(): void {
        if (this.atlasFramebuffer == null)
            this.createAtlasFramebuffer();

        this.layoutText();
    }

    finishAttachingText(): void {
        this.buildGlyphs();
        this.renderContext.setDirty();
    }

    setAntialiasingOptions(aaType: AntialiasingStrategyName,
                           aaLevel: number,
                           aaOptions: AAOptions):
                           void {
        super.setAntialiasingOptions(aaType, aaLevel, aaOptions);

        // Need to relayout because changing AA options can cause font dilation to change...
        this.layoutText();
        this.buildGlyphs();
        this.renderContext.setDirty();
    }

    relayoutText(): void {
        this.layoutText();
        this.buildGlyphs();
        this.renderContext.setDirty();
    }

    updateHinting(): void {
        // Need to relayout the text because the pixel bounds of the glyphs can change from this...
        this.layoutText();
        this.buildGlyphs();
        this.renderContext.setDirty();
    }

    viewPanned(): void {
        this.buildGlyphs();
        this.renderContext.setDirty();
    }

    protected compositeIfNecessary(): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        // Set up composite state.
        gl.bindFramebuffer(gl.FRAMEBUFFER, null);
        gl.viewport(0, 0, renderContext.cameraView.width, renderContext.cameraView.height);
        gl.disable(gl.DEPTH_TEST);
        gl.disable(gl.SCISSOR_TEST);
        gl.blendEquation(gl.FUNC_REVERSE_SUBTRACT);
        gl.blendFuncSeparate(gl.ONE, gl.ONE, gl.ZERO, gl.ONE);
        gl.enable(gl.BLEND);

        // Clear.
        gl.clearColor(1.0, 1.0, 1.0, 1.0);
        gl.clear(gl.COLOR_BUFFER_BIT);

        // Set the appropriate program.
        const programName = this.gammaCorrectionMode === 'off' ? 'blitLinear' : 'blitGamma';
        const blitProgram = this.renderContext.shaderPrograms[programName];

        // Set up the composite VAO.
        const attributes = blitProgram.attributes;
        gl.useProgram(blitProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.glyphPositionsBuffer);
        gl.vertexAttribPointer(attributes.aPosition, 2, gl.FLOAT, false, 0, 0);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.glyphTexCoordsBuffer);
        gl.vertexAttribPointer(attributes.aTexCoord, 2, gl.FLOAT, false, 0, 0);
        gl.enableVertexAttribArray(attributes.aPosition);
        gl.enableVertexAttribArray(attributes.aTexCoord);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.glyphElementsBuffer);

        // Create the transform.
        const transform = glmatrix.mat4.create();
        glmatrix.mat4.fromTranslation(transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform, transform, [
            2.0 / this.renderContext.cameraView.width,
            2.0 / this.renderContext.cameraView.height,
            1.0,
        ]);
        glmatrix.mat4.translate(transform,
                                transform,
                                [this.camera.translation[0], this.camera.translation[1], 0.0]);

        // Blit.
        gl.uniformMatrix4fv(blitProgram.uniforms.uTransform, false, transform);
        gl.activeTexture(gl.TEXTURE0);
        const destTexture = this.renderContext
                                .atlas
                                .ensureTexture(this.renderContext);
        gl.bindTexture(gl.TEXTURE_2D, destTexture);
        gl.uniform1i(blitProgram.uniforms.uSource, 0);
        this.setIdentityTexScaleUniform(blitProgram.uniforms);
        this.bindGammaLUT(glmatrix.vec3.clone([1.0, 1.0, 1.0]), 1, blitProgram.uniforms);
        const totalGlyphCount = this.layout.textFrame.totalGlyphCount;
        gl.drawElements(gl.TRIANGLES, totalGlyphCount * 6, gl.UNSIGNED_INT, 0);
    }

    private layoutText(): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        this.layout.layoutRuns();

        const textBounds = this.layout.textFrame.bounds;
        this.camera.bounds = textBounds;

        const totalGlyphCount = this.layout.textFrame.totalGlyphCount;
        const glyphPositions = new Float32Array(totalGlyphCount * 8);
        const glyphIndices = new Uint32Array(totalGlyphCount * 6);

        const hint = this.createHint();
        const pixelsPerUnit = this.pixelsPerUnit;

        let globalGlyphIndex = 0;
        for (const run of this.layout.textFrame.runs) {
            for (let glyphIndex = 0;
                 glyphIndex < run.glyphIDs.length;
                 glyphIndex++, globalGlyphIndex++) {
                const rect = run.pixelRectForGlyphAt(glyphIndex,
                                                     pixelsPerUnit,
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

        this.glyphPositionsBuffer = unwrapNull(gl.createBuffer());
        gl.bindBuffer(gl.ARRAY_BUFFER, this.glyphPositionsBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, glyphPositions, gl.STATIC_DRAW);
        this.glyphElementsBuffer = unwrapNull(gl.createBuffer());
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.glyphElementsBuffer);
        gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, glyphIndices, gl.STATIC_DRAW);
    }

    private buildGlyphs(): void {
        const font = this.renderContext.font;
        const glyphStore = this.renderContext.glyphStore;
        const pixelsPerUnit = this.pixelsPerUnit;

        const textFrame = this.layout.textFrame;
        const hint = this.createHint();

        // Only build glyphs in view.
        const translation = this.camera.translation;
        const canvasRect = glmatrix.vec4.clone([
            -translation[0],
            -translation[1],
            -translation[0] + this.renderContext.cameraView.width,
            -translation[1] + this.renderContext.cameraView.height,
        ]);

        const atlasGlyphs = [];
        for (const run of textFrame.runs) {
            for (let glyphIndex = 0; glyphIndex < run.glyphIDs.length; glyphIndex++) {
                const pixelRect = run.pixelRectForGlyphAt(glyphIndex,
                                                          pixelsPerUnit,
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
                                                        pixelsPerUnit,
                                                        hint,
                                                        SUBPIXEL_GRANULARITY);
                const glyphKey = new GlyphKey(glyphID, subpixel);
                atlasGlyphs.push(new AtlasGlyph(glyphStoreIndex, glyphKey));
            }
        }

        this.buildAtlasGlyphs(atlasGlyphs);

        // TODO(pcwalton): Regenerate the IBOs to include only the glyphs we care about.

        this.setGlyphTexCoords();
    }

    private setGlyphTexCoords(): void {
        const textFrame = this.layout.textFrame;
        const font = this.renderContext.font;
        const atlasGlyphs = this.renderContext.atlasGlyphs;

        const hint = this.createHint();
        const pixelsPerUnit = this.pixelsPerUnit;

        const atlasGlyphKeys = atlasGlyphs.map(atlasGlyph => atlasGlyph.glyphKey.sortKey);

        this.glyphBounds = new Float32Array(textFrame.totalGlyphCount * 8);

        let globalGlyphIndex = 0;
        for (const run of textFrame.runs) {
            for (let glyphIndex = 0;
                 glyphIndex < run.glyphIDs.length;
                 glyphIndex++, globalGlyphIndex++) {
                const textGlyphID = run.glyphIDs[glyphIndex];

                const subpixel = run.subpixelForGlyphAt(glyphIndex,
                                                        pixelsPerUnit,
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

                const atlasGlyphPixelOrigin =
                    atlasGlyph.calculateSubpixelOrigin(pixelsPerUnit);
                const atlasGlyphRect = calculatePixelRectForGlyph(atlasGlyphUnitMetrics,
                                                                  atlasGlyphPixelOrigin,
                                                                  pixelsPerUnit,
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

    private setIdentityTexScaleUniform(uniforms: UniformMap): void {
        this.renderContext.gl.uniform2f(uniforms.uTexScale, 1.0, 1.0);
    }
}

/// The separating axis theorem.
function rectsIntersect(a: glmatrix.vec4, b: glmatrix.vec4): boolean {
    return a[2] > b[0] && a[3] > b[1] && a[0] < b[2] && a[1] < b[3];
}

function main(): void {
    const controller = new TextDemoController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
