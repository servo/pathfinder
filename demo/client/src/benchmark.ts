// pathfinder/client/src/benchmark.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';
import * as opentype from "opentype.js";

import { AppController, DemoAppController } from "./app-controller";
import {PathfinderMeshData} from "./meshes";
import { BUILTIN_FONT_URI, TextFrame, TextRun, ExpandedMeshData, GlyphStore, PathfinderFont } from "./text";
import { assert, unwrapNull, PathfinderError } from "./utils";
import { PathfinderDemoView, Timings, MonochromePathfinderView } from "./view";
import { ShaderMap, ShaderProgramSource } from "./shader-loader";
import { AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy } from "./aa-strategy";
import SSAAStrategy from './ssaa-strategy';
import { OrthographicCamera } from './camera';
import { ECAAStrategy, ECAAMonochromeStrategy } from './ecaa-strategy';
import PathfinderBufferTexture from './buffer-texture';

const STRING: string = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

const FONT: string = 'nimbus-sans';

const TEXT_COLOR: number[] = [0, 0, 0, 255];

const MIN_FONT_SIZE: number = 6;
const MAX_FONT_SIZE: number = 200;

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
    ecaa: ECAAMonochromeStrategy,
};

interface ElapsedTime {
    size: number;
    time: number;
}

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
    ecaa: typeof ECAAStrategy;
}

class BenchmarkAppController extends DemoAppController<BenchmarkTestView> {
    start() {
        super.start();

        this.resultsModal = unwrapNull(document.getElementById('pf-benchmark-results-modal')) as
            HTMLDivElement;
        this.resultsTableBody =
            unwrapNull(document.getElementById('pf-benchmark-results-table-body')) as
            HTMLTableSectionElement;
        this.resultsPartitioningTimeLabel =
            unwrapNull(document.getElementById('pf-benchmark-results-partitioning-time')) as
            HTMLSpanElement;

        const resultsCloseButton =
            unwrapNull(document.getElementById('pf-benchmark-results-close-button'));
        resultsCloseButton.addEventListener('click', () => {
            window.jQuery(this.resultsModal).modal('hide');
        }, false);

        const runBenchmarkButton = unwrapNull(document.getElementById('pf-run-benchmark-button'));
        runBenchmarkButton.addEventListener('click', () => this.runBenchmark(), false);

        this.loadInitialFile(this.builtinFileURI);
    }

    protected fileLoaded(fileData: ArrayBuffer): void {
        const font = new PathfinderFont(fileData);
        this.font = font;

        const textRun = new TextRun(STRING, [0, 0], font);
        textRun.layout();
        this.textRun = textRun;
        const textFrame = new TextFrame([textRun], font);

        const glyphIDs = textFrame.allGlyphIDs;
        glyphIDs.sort((a, b) => a - b);
        this.glyphStore = new GlyphStore(font, glyphIDs);

        this.glyphStore.partition().then(result => {
            this.baseMeshes = result.meshes;

            const partitionTime = result.time / this.glyphStore.glyphIDs.length * 1e6;
            const timeLabel = this.resultsPartitioningTimeLabel;
            while (timeLabel.firstChild != null)
                timeLabel.removeChild(timeLabel.firstChild);
            timeLabel.appendChild(document.createTextNode("" + partitionTime));

            const expandedMeshes = textFrame.expandMeshes(this.baseMeshes, glyphIDs);
            this.expandedMeshes = expandedMeshes;

            this.view.then(view => {
                view.uploadPathColors(1);
                view.uploadPathTransforms(1);
                view.uploadHints();
                view.attachMeshes([expandedMeshes.meshes]);
            })
        })
    }

    protected createView(): BenchmarkTestView {
        return new BenchmarkTestView(this,
                                     unwrapNull(this.commonShaderSource),
                                     unwrapNull(this.shaderSources));
    }

    private runBenchmark(): void {
        this.pixelsPerEm = MIN_FONT_SIZE;
        this.elapsedTimes = [];
        this.view.then(view => this.runOneBenchmarkTest(view));
    }

    private runOneBenchmarkTest(view: BenchmarkTestView): void {
        const renderedPromise = new Promise<number>((resolve, reject) => {
            view.renderingPromiseCallback = resolve;
            view.pixelsPerEm = this.pixelsPerEm;
        });
        renderedPromise.then(elapsedTime => {
            this.elapsedTimes.push({ size: this.pixelsPerEm, time: elapsedTime });

            if (this.pixelsPerEm == MAX_FONT_SIZE) {
                this.showResults();
                return;
            }

            this.pixelsPerEm++;
            this.runOneBenchmarkTest(view);
        });
    }

    private showResults(): void {
        while (this.resultsTableBody.lastChild != null)
            this.resultsTableBody.removeChild(this.resultsTableBody.lastChild);

        for (const elapsedTime of this.elapsedTimes) {
            const tr = document.createElement('tr');
            const sizeTH = document.createElement('th');
            const timeTD = document.createElement('td');
            sizeTH.appendChild(document.createTextNode("" + elapsedTime.size));
            timeTD.appendChild(document.createTextNode("" + elapsedTime.time));
            sizeTH.scope = 'row';
            tr.appendChild(sizeTH);
            tr.appendChild(timeTD);
            this.resultsTableBody.appendChild(tr);
        }

        window.jQuery(this.resultsModal).modal();
    }

    protected readonly defaultFile: string = FONT;
    protected readonly builtinFileURI: string = BUILTIN_FONT_URI;

    private resultsModal: HTMLDivElement;
    private resultsTableBody: HTMLTableSectionElement;
    private resultsPartitioningTimeLabel: HTMLSpanElement;

    private glyphStore: GlyphStore;
    private baseMeshes: PathfinderMeshData;
    private expandedMeshes: ExpandedMeshData;

    private pixelsPerEm: number;
    private elapsedTimes: ElapsedTime[];
    private partitionTime: number;

    font: PathfinderFont | null;
    textRun: TextRun | null;
}
    
class BenchmarkTestView extends MonochromePathfinderView {
    constructor(appController: BenchmarkAppController,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(commonShaderSource, shaderSources);

        this.appController = appController;

        this.camera = new OrthographicCamera(this.canvas);
        this.camera.onPan = () => this.setDirty();
        this.camera.onZoom = () => this.setDirty();
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName,
                               aaLevel: number,
                               subpixelAA: boolean):
                               AntialiasingStrategy {
        return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel, subpixelAA);
    }

    protected compositeIfNecessary(): void {}

    protected updateTimings(timings: Timings): void {
        // TODO(pcwalton)
    }

    protected pathColorsForObject(objectIndex: number): Uint8Array {
        const pathColors = new Uint8Array(4 * (STRING.length + 1));
        for (let pathIndex = 0; pathIndex < STRING.length; pathIndex++)
            pathColors.set(TEXT_COLOR, (pathIndex + 1) * 4);
        return pathColors;
    }

    protected pathTransformsForObject(objectIndex: number): Float32Array {
        const pathTransforms = new Float32Array(4 * (STRING.length + 1));

        let currentX = 0, currentY = 0;
        const availableWidth = this.canvas.width / this.pixelsPerUnit;
        const lineHeight = unwrapNull(this.appController.font).opentypeFont.lineHeight();

        for (let glyphIndex = 0; glyphIndex < STRING.length; glyphIndex++) {
            const glyphID = unwrapNull(this.appController.textRun).glyphIDs[glyphIndex];
            pathTransforms.set([1, 1, currentX, currentY], (glyphIndex + 1) * 4);

            currentX += unwrapNull(this.appController.font).opentypeFont
                                                           .glyphs
                                                           .get(glyphID)
                                                           .advanceWidth;
            if (currentX > availableWidth) {
                currentX = 0;
                currentY += lineHeight;
            }
        }

        return pathTransforms;
    }

    protected renderingFinished(): void {
        if (this.renderingPromiseCallback != null) {
            const glyphCount = unwrapNull(this.appController.textRun).glyphIDs.length;
            const usPerGlyph = this.lastTimings.rendering * 1000.0 / glyphCount;
            this.renderingPromiseCallback(usPerGlyph);
        }
    }

    uploadHints(): void {
        const glyphCount = unwrapNull(this.appController.textRun).glyphIDs.length;
        const pathHints = new Float32Array((glyphCount + 1) * 4);

        const pathHintsBufferTexture = new PathfinderBufferTexture(this.gl, 'uPathHints');
        pathHintsBufferTexture.upload(this.gl, pathHints);
        this.pathHintsBufferTexture = pathHintsBufferTexture;
    }

    destFramebuffer: WebGLFramebuffer | null = null;

    get destAllocatedSize(): glmatrix.vec2 {
        return glmatrix.vec2.clone([this.canvas.width, this.canvas.height]);
    }

    get destUsedSize(): glmatrix.vec2 {
        return this.destAllocatedSize;
    }

    private readonly appController: BenchmarkAppController;

    protected usedSizeFactor: glmatrix.vec2 = glmatrix.vec2.clone([1.0, 1.0]);

    protected get worldTransform() {
        const transform = glmatrix.mat4.create();
        const translation = this.camera.translation;
        glmatrix.mat4.translate(transform, transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform,
                            transform,
                            [2.0 / this.canvas.width, 2.0 / this.canvas.height, 1.0]);
        glmatrix.mat4.translate(transform, transform, [translation[0], translation[1], 0]);
        glmatrix.mat4.scale(transform, transform, [this.camera.scale, this.camera.scale, 1.0]);

        const pixelsPerUnit = this.pixelsPerUnit;
        glmatrix.mat4.scale(transform, transform, [pixelsPerUnit, pixelsPerUnit, 1.0]);

        return transform;
    }

    private get pixelsPerUnit(): number {
        return this._pixelsPerEm / unwrapNull(this.appController.font).opentypeFont.unitsPerEm;
    }

    get pixelsPerEm(): number {
        return this._pixelsPerEm;
    }

    set pixelsPerEm(newPixelsPerEm: number) {
        this._pixelsPerEm = newPixelsPerEm;
        this.uploadPathTransforms(1);
        this.setDirty();
    }

    renderingPromiseCallback: ((time: number) => void) | null;

    private _pixelsPerEm: number = 32.0;

    readonly bgColor: glmatrix.vec4 = glmatrix.vec4.clone([1.0, 1.0, 1.0, 0.0]);
    readonly fgColor: glmatrix.vec4 = glmatrix.vec4.clone([0.0, 0.0, 0.0, 1.0]);

    protected directCurveProgramName: keyof ShaderMap<void> = 'directCurve';
    protected directInteriorProgramName: keyof ShaderMap<void> = 'directInterior';

    protected depthFunction: number = this.gl.GREATER;

    protected camera: OrthographicCamera;
}

function main() {
    const controller = new BenchmarkAppController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
