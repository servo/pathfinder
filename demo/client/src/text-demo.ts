// pathfinder/client/src/text-demo.ts
//
// Copyright © 2018 The Pathfinder Project Developers.
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
import {StemDarkeningMode, SubpixelAAType} from './aa-strategy';
import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from "./aa-strategy";
import {AAOptions, DemoAppController, setSwitchInputsValue, SwitchInputs} from './app-controller';
import {Atlas, ATLAS_SIZE, AtlasGlyph, GlyphKey, SUBPIXEL_GRANULARITY} from './atlas';
import PathfinderBufferTexture from './buffer-texture';
import {CameraView, OrthographicCamera} from "./camera";
import {createFramebuffer, createFramebufferColorTexture} from './gl-utils';
import {createFramebufferDepthTexture, QUAD_ELEMENTS, setTextureParameters} from './gl-utils';
import {UniformMap} from './gl-utils';
import {PathfinderMeshPack, PathfinderPackedMeshBuffers, PathfinderPackedMeshes} from './meshes';
import {PathfinderShaderProgram, ShaderMap, ShaderProgramSource} from './shader-loader';
import SSAAStrategy from './ssaa-strategy';
import {calculatePixelRectForGlyph, PathfinderFont} from "./text";
import {BUILTIN_FONT_URI, calculatePixelXMin, computeStemDarkeningAmount} from "./text";
import {GlyphStore, Hint, SimpleTextLayout, UnitMetrics} from "./text";
import {SimpleTextLayoutRenderContext, SimpleTextLayoutRenderer} from './text-renderer';
import {TextRenderContext, TextRenderer} from './text-renderer';
import {assert, expectNotNull, panic, PathfinderError, scaleRect, UINT32_SIZE} from './utils';
import {unwrapNull} from './utils';
import {DemoView, RenderContext, Timings, TIMINGS} from './view';

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

interface TabShownEvent {
    target: EventTarget;
    relatedTarget: EventTarget;
}

interface JQuerySubset {
    modal(options?: any): void;
    on(name: 'shown.bs.tab', handler: (event: TabShownEvent) => void): void;
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
    font!: PathfinderFont;
    layout!: SimpleTextLayout;
    glyphStore!: GlyphStore;
    atlasGlyphs!: AtlasGlyph[];

    private hintingSelect!: HTMLSelectElement;
    private emboldenInput!: HTMLInputElement;

    private editTextModal!: HTMLElement;
    private editTextArea!: HTMLTextAreaElement;

    private _atlas: Atlas;

    private meshes!: PathfinderPackedMeshes;

    private _fontSize!: number;
    private _rotationAngle!: number;
    private _emboldenAmount!: number;

    private text: string;

    constructor() {
        super();
        this.text = DEFAULT_TEXT;
        this._atlas = new Atlas;
    }

    start(): void {
        this._fontSize = INITIAL_FONT_SIZE;
        this._rotationAngle = 0.0;
        this._emboldenAmount = 0.0;

        this.hintingSelect = unwrapNull(document.getElementById('pf-hinting-select')) as
            HTMLSelectElement;
        this.hintingSelect.addEventListener('change', () => this.hintingChanged(), false);

        this.emboldenInput = unwrapNull(document.getElementById('pf-embolden')) as
            HTMLInputElement;
        this.emboldenInput.addEventListener('input', () => this.emboldenAmountChanged(), false);

        this.editTextModal = unwrapNull(document.getElementById('pf-edit-text-modal'));
        this.editTextArea = unwrapNull(document.getElementById('pf-edit-text-area')) as
            HTMLTextAreaElement;

        const editTextOkButton = unwrapNull(document.getElementById('pf-edit-text-ok-button'));
        editTextOkButton.addEventListener('click', () => this.updateText(), false);

        super.start();

        this.loadInitialFile(this.builtinFileURI);
    }

    showTextEditor(): void {
        this.editTextArea.value = this.text;

        window.jQuery(this.editTextModal).modal();
    }

    get emboldenAmount(): number {
        return this._emboldenAmount;
    }

    protected updateUIForAALevelChange(aaType: AntialiasingStrategyName, aaLevel: number): void {
        const gammaCorrectionSwitchInputs = unwrapNull(this.gammaCorrectionSwitchInputs);
        const stemDarkeningSwitchInputs = unwrapNull(this.stemDarkeningSwitchInputs);
        const emboldenInput = unwrapNull(this.emboldenInput);
        const emboldenLabel = getLabelFor(emboldenInput);

        switch (aaType) {
        case 'none':
        case 'ssaa':
            enableSwitchInputs(gammaCorrectionSwitchInputs, false);
            enableSwitchInputs(stemDarkeningSwitchInputs, false);
            emboldenInput.value = "0";
            emboldenInput.disabled = true;
            enableLabel(emboldenLabel, false);
            break;

        case 'xcaa':
            enableSwitchInputs(gammaCorrectionSwitchInputs, true);
            enableSwitchInputs(stemDarkeningSwitchInputs, true);
            emboldenInput.value = "0";
            emboldenInput.disabled = false;
            enableLabel(emboldenLabel, true);
            break;
        }

        function enableSwitchInputs(switchInputs: SwitchInputs, enabled: boolean): void {
            switchInputs.off.disabled = switchInputs.on.disabled = !enabled;
            enableLabel(getLabelFor(switchInputs.on), enabled);
        }

        function enableLabel(label: HTMLLabelElement | null, enabled: boolean): void {
            if (label == null)
                return;

            if (enabled)
                label.classList.remove('pf-disabled');
            else
                label.classList.add('pf-disabled');
        }

        function getLabelFor(element: HTMLElement): HTMLLabelElement | null {
            return document.querySelector(`label[for="${element.id}"]`) as HTMLLabelElement | null;
        }
    }

    protected createView(areaLUT: HTMLImageElement,
                         gammaLUT: HTMLImageElement,
                         commonShaderSource: string,
                         shaderSources: ShaderMap<ShaderProgramSource>):
                         TextDemoView {
        return new TextDemoView(this, areaLUT, gammaLUT, commonShaderSource, shaderSources);
    }

    protected fileLoaded(fileData: ArrayBuffer, builtinName: string | null) {
        const font = new PathfinderFont(fileData, builtinName);
        this.recreateLayout(font);
    }

    private hintingChanged(): void {
        this.view.then(view => view.renderer.updateHinting());
    }

    private emboldenAmountChanged(): void {
        this._emboldenAmount = parseFloat(this.emboldenInput.value);
        this.view.then(view => view.renderer.updateEmboldenAmount());
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

    private expandMeshes(meshes: PathfinderMeshPack, glyphCount: number): PathfinderPackedMeshes {
        const pathIDs = [];
        for (let glyphIndex = 0; glyphIndex < glyphCount; glyphIndex++) {
            for (let subpixel = 0; subpixel < SUBPIXEL_GRANULARITY; subpixel++)
                pathIDs.push(glyphIndex + 1);
        }
        return new PathfinderPackedMeshes(meshes, pathIDs);
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

    get unitsPerEm(): number {
        return this.font.opentypeFont.unitsPerEm;
    }

    get pixelsPerUnit(): number {
        return this._fontSize / this.unitsPerEm;
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

class TextDemoView extends DemoView implements SimpleTextLayoutRenderContext {
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

    get layout(): SimpleTextLayout {
        return this.appController.layout;
    }

    get rotationAngle(): number {
        return this.appController.rotationAngle;
    }

    get emboldenAmount(): number {
        return this.appController.emboldenAmount;
    }

    get unitsPerEm(): number {
        return this.appController.unitsPerEm;
    }

    protected get camera(): OrthographicCamera {
        return this.renderer.camera;
    }

    constructor(appController: TextDemoController,
                areaLUT: HTMLImageElement,
                gammaLUT: HTMLImageElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(areaLUT, gammaLUT, commonShaderSource, shaderSources);

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

class TextDemoRenderer extends SimpleTextLayoutRenderer {
    renderContext!: TextDemoView;
}

function main(): void {
    const controller = new TextDemoController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
