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
import {Atlas, AtlasGlyph, SUBPIXEL_GRANULARITY} from './atlas';
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

class TextDemoView extends DemoView implements TextRenderContext {
    renderer: TextRenderer;

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

    get layout(): SimpleTextLayout {
        return this.appController.layout;
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

    get layoutPixelsPerUnit(): number {
        return this.appController.layoutPixelsPerUnit;
    }

    get useHinting(): boolean {
        return this.appController.useHinting;
    }

    protected get camera(): OrthographicCamera {
        return this.renderer.camera;
    }

    constructor(appController: TextDemoController,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(commonShaderSource, shaderSources);

        this.appController = appController;
        this.renderer = new TextRenderer(this);

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

function main() {
    const controller = new TextDemoController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
