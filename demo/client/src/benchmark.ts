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
import * as _ from 'lodash';
import * as opentype from "opentype.js";

import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from "./aa-strategy";
import {SubpixelAAType} from "./aa-strategy";
import {AppController, DemoAppController, setSwitchInputsValue} from "./app-controller";
import PathfinderBufferTexture from './buffer-texture';
import {OrthographicCamera} from './camera';
import {UniformMap} from './gl-utils';
import {PathfinderMeshPack, PathfinderPackedMeshes} from "./meshes";
import {PathTransformBuffers, Renderer} from './renderer';
import {ShaderMap, ShaderProgramSource} from "./shader-loader";
import SSAAStrategy from './ssaa-strategy';
import {BUILTIN_SVG_URI, SVGLoader} from './svg-loader';
import {SVGRenderer} from './svg-renderer';
import {BUILTIN_FONT_URI, ExpandedMeshData, GlyphStore, PathfinderFont, TextFrame} from "./text";
import {computeStemDarkeningAmount, TextRun} from "./text";
import {assert, lerp, PathfinderError, unwrapNull, unwrapUndef} from "./utils";
import {DemoView, Timings} from "./view";
import {AdaptiveStencilMeshAAAStrategy} from './xcaa-strategy';

const STRING: string = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

const DEFAULT_FONT: string = 'nimbus-sans';
const DEFAULT_SVG_FILE: string = 'tiger';

const TEXT_COLOR: number[] = [0, 0, 0, 255];

// In milliseconds.
const MIN_RUNTIME: number = 100;
const MAX_RUNTIME: number = 3000;

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
    xcaa: AdaptiveStencilMeshAAAStrategy,
};

interface BenchmarkModeMap<T> {
    text: T;
    svg: T;
}

type BenchmarkMode = 'text' | 'svg';

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
    xcaa: typeof AdaptiveStencilMeshAAAStrategy;
}

interface TestParameter {
    start: number;
    stop: number;
    step: number;
}

const DISPLAY_HEADER_LABELS: BenchmarkModeMap<string[]> = {
    svg: ["Size (px)", "GPU time (ms)"],
    text: ["Font size (px)", "GPU time per glyph (µs)"],
};

const TEST_SIZES: BenchmarkModeMap<TestParameter> = {
    svg: { start: 64, stop: 2048, step: 16 },
    text: { start: 6, stop: 200, step: 1 },
};

class BenchmarkAppController extends DemoAppController<BenchmarkTestView> {
    font: PathfinderFont | null = null;
    textRun: TextRun | null = null;

    svgLoader!: SVGLoader;

    mode!: BenchmarkMode;

    protected get defaultFile(): string {
        if (this.mode === 'text')
            return DEFAULT_FONT;
        return DEFAULT_SVG_FILE;
    }

    protected get builtinFileURI(): string {
        if (this.mode === 'text')
            return BUILTIN_FONT_URI;
        return BUILTIN_SVG_URI;
    }

    private optionsModal!: HTMLDivElement;

    private resultsModal!: HTMLDivElement;
    private resultsTableHeader!: HTMLTableSectionElement;
    private resultsTableBody!: HTMLTableSectionElement;
    private resultsPartitioningTimeLabel!: HTMLSpanElement;

    private glyphStore!: GlyphStore;
    private baseMeshes!: PathfinderMeshPack;
    private expandedMeshes!: ExpandedMeshData;

    private size!: number;
    private currentRun!: number;
    private startTime!: number;
    private elapsedTimes!: ElapsedTime[];
    private partitionTime!: number;

    start(): void {
        super.start();

        this.mode = 'text';

        this.optionsModal = unwrapNull(document.getElementById('pf-benchmark-modal')) as
            HTMLDivElement;

        this.resultsModal = unwrapNull(document.getElementById('pf-benchmark-results-modal')) as
            HTMLDivElement;
        this.resultsTableHeader =
            unwrapNull(document.getElementById('pf-benchmark-results-table-header')) as
            HTMLTableSectionElement;
        this.resultsTableBody =
            unwrapNull(document.getElementById('pf-benchmark-results-table-body')) as
            HTMLTableSectionElement;
        this.resultsPartitioningTimeLabel =
            unwrapNull(document.getElementById('pf-benchmark-results-partitioning-time')) as
            HTMLSpanElement;

        const resultsSaveCSVButton =
            unwrapNull(document.getElementById('pf-benchmark-results-save-csv-button'));
        resultsSaveCSVButton.addEventListener('click', () => this.saveCSV(), false);

        const resultsCloseButton =
            unwrapNull(document.getElementById('pf-benchmark-results-close-button'));
        resultsCloseButton.addEventListener('click', () => {
            window.jQuery(this.resultsModal).modal('hide');
        }, false);

        const runBenchmarkButton = unwrapNull(document.getElementById('pf-run-benchmark-button'));
        runBenchmarkButton.addEventListener('click', () => this.runBenchmark(), false);

        const aaLevelFormGroup = unwrapNull(document.getElementById('pf-aa-level-form-group')) as
            HTMLDivElement;
        const benchmarkTextForm = unwrapNull(document.getElementById('pf-benchmark-text-form')) as
            HTMLFormElement;
        const benchmarkSVGForm = unwrapNull(document.getElementById('pf-benchmark-svg-form')) as
            HTMLFormElement;

        window.jQuery(this.optionsModal).modal();

        const benchmarkTextTab = document.getElementById('pf-benchmark-text-tab') as
            HTMLAnchorElement;
        const benchmarkSVGTab = document.getElementById('pf-benchmark-svg-tab') as
            HTMLAnchorElement;
        window.jQuery(benchmarkTextTab).on('shown.bs.tab', event => {
            this.mode = 'text';
            if (aaLevelFormGroup.parentElement != null)
                aaLevelFormGroup.parentElement.removeChild(aaLevelFormGroup);
            benchmarkTextForm.insertBefore(aaLevelFormGroup, benchmarkTextForm.firstChild);
            this.modeChanged();
        });
        window.jQuery(benchmarkSVGTab).on('shown.bs.tab', event => {
            this.mode = 'svg';
            if (aaLevelFormGroup.parentElement != null)
                aaLevelFormGroup.parentElement.removeChild(aaLevelFormGroup);
            benchmarkSVGForm.insertBefore(aaLevelFormGroup, benchmarkSVGForm.firstChild);
            this.modeChanged();
        });

        this.loadInitialFile(this.builtinFileURI);
    }

    protected fileLoaded(fileData: ArrayBuffer, builtinName: string | null): void {
        switch (this.mode) {
        case 'text':
            this.textFileLoaded(fileData, builtinName);
            return;
        case 'svg':
            this.svgFileLoaded(fileData, builtinName);
            return;
        }
    }

    protected createView(areaLUT: HTMLImageElement,
                         gammaLUT: HTMLImageElement,
                         commonShaderSource: string,
                         shaderSources: ShaderMap<ShaderProgramSource>):
                         BenchmarkTestView {
        return new BenchmarkTestView(this, areaLUT, gammaLUT, commonShaderSource, shaderSources);
    }

    private modeChanged(): void {
        this.loadInitialFile(this.builtinFileURI);
        if (this.aaLevelSelect != null)
            this.aaLevelSelect.selectedIndex = 0;
        if (this.subpixelAASelect != null)
            this.subpixelAASelect.selectedIndex = 0;
        this.updateAALevel();
    }

    private textFileLoaded(fileData: ArrayBuffer, builtinName: string | null): void {
        const font = new PathfinderFont(fileData, builtinName);
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
                view.recreateRenderer();
                view.attachMeshes([expandedMeshes.meshes]);
            });
        });
    }

    private svgFileLoaded(fileData: ArrayBuffer, builtinName: string | null): void {
        this.svgLoader = new SVGLoader;
        this.svgLoader.loadFile(fileData);
        this.svgLoader.partition().then(meshes => {
            this.view.then(view => {
                view.recreateRenderer();
                view.attachMeshes([new PathfinderPackedMeshes(meshes)]);
                view.initCameraBounds(this.svgLoader.svgViewBox);
            });
        });
    }

    private reset(): void {
        this.currentRun = 0;
        this.startTime = Date.now();
    }

    private runBenchmark(): void {
        window.jQuery(this.optionsModal).modal('hide');

        this.reset();
        this.elapsedTimes = [];
        this.size = TEST_SIZES[this.mode].start;
        this.view.then(view => this.runOneBenchmarkTest(view));
    }

    private runDone(): boolean {
        const totalElapsedTime = Date.now() - this.startTime;
        if (totalElapsedTime < MIN_RUNTIME)
            return false;
        if (totalElapsedTime >= MAX_RUNTIME)
            return true;

        // Compute median absolute devation.
        const elapsedTime = unwrapUndef(_.last(this.elapsedTimes));
        elapsedTime.times.sort((a, b) => a - b);
        const median = unwrapNull(computeMedian(elapsedTime.times));
        const absoluteDeviations = elapsedTime.times.map(time => Math.abs(time - median));
        absoluteDeviations.sort((a, b) => a - b);
        const medianAbsoluteDeviation = unwrapNull(computeMedian(absoluteDeviations));
        const medianAbsoluteDeviationFraction = medianAbsoluteDeviation / median;
        return medianAbsoluteDeviationFraction <= 0.01;
    }

    private runOneBenchmarkTest(view: BenchmarkTestView): void {
        const renderedPromise = new Promise<number>((resolve, reject) => {
            view.renderingPromiseCallback = resolve;
            view.size = this.size;
        });
        renderedPromise.then(elapsedTime => {
            if (this.currentRun === 0)
                this.elapsedTimes.push(new ElapsedTime(this.size));
            unwrapUndef(_.last(this.elapsedTimes)).times.push(elapsedTime);

            this.currentRun++;
            if (this.runDone()) {
                this.reset();

                if (this.size >= TEST_SIZES[this.mode].stop) {
                    this.showResults();
                    return;
                }

                this.size += TEST_SIZES[this.mode].step;
            }

            this.runOneBenchmarkTest(view);
        });
    }

    private showResults(): void {
        while (this.resultsTableHeader.lastChild != null)
            this.resultsTableHeader.removeChild(this.resultsTableHeader.lastChild);
        while (this.resultsTableBody.lastChild != null)
            this.resultsTableBody.removeChild(this.resultsTableBody.lastChild);

        const tr = document.createElement('tr');
        for (const headerLabel of DISPLAY_HEADER_LABELS[this.mode]) {
            const th = document.createElement('th');
            th.appendChild(document.createTextNode(headerLabel));
            tr.appendChild(th);
        }
        this.resultsTableHeader.appendChild(tr);

        for (const elapsedTime of this.elapsedTimes) {
            const tr = document.createElement('tr');
            const sizeTH = document.createElement('th');
            const timeTD = document.createElement('td');
            sizeTH.appendChild(document.createTextNode("" + elapsedTime.size));
            const time = this.mode === 'svg' ? elapsedTime.timeInMS : elapsedTime.time;
            timeTD.appendChild(document.createTextNode("" + time));
            sizeTH.scope = 'row';
            tr.appendChild(sizeTH);
            tr.appendChild(timeTD);
            this.resultsTableBody.appendChild(tr);
        }

        window.jQuery(this.resultsModal).modal();
    }

    private saveCSV(): void {
        let output = "Size,Time\n";
        for (const elapsedTime of this.elapsedTimes) {
            const time = this.mode === 'svg' ? elapsedTime.timeInMS : elapsedTime.time;
            output += `${elapsedTime.size},${time}\n`;
        }

        // https://stackoverflow.com/a/30832210
        const file = new Blob([output], {type: 'text/csv'});
        const a = document.createElement('a');
        const url = URL.createObjectURL(file);
        a.href = url;
        a.download = "pathfinder-benchmark-results.csv";
        document.body.appendChild(a);
        a.click();

        window.setTimeout(() => {
            document.body.removeChild(a);
            URL.revokeObjectURL(url);
        }, 0);
    }
}

class BenchmarkTestView extends DemoView {
    renderer!: BenchmarkTextRenderer | BenchmarkSVGRenderer;

    readonly appController: BenchmarkAppController;

    renderingPromiseCallback: ((time: number) => void) | null = null;

    get camera(): OrthographicCamera {
        return this.renderer.camera;
    }

    set size(newSize: number) {
        if (this.renderer instanceof BenchmarkTextRenderer) {
            this.renderer.pixelsPerEm = newSize;
        } else if (this.renderer instanceof BenchmarkSVGRenderer) {
            const camera = this.renderer.camera;
            camera.zoomToSize(newSize);
            camera.center();
        }
    }

    constructor(appController: BenchmarkAppController,
                areaLUT: HTMLImageElement,
                gammaLUT: HTMLImageElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(areaLUT, gammaLUT, commonShaderSource, shaderSources);
        this.appController = appController;
        this.recreateRenderer();
        this.resizeToFit(true);
    }

    recreateRenderer(): void {
        switch (this.appController.mode) {
        case 'svg':
            this.renderer = new BenchmarkSVGRenderer(this);
            break;
        case 'text':
            this.renderer = new BenchmarkTextRenderer(this);
            break;
        }
    }

    initCameraBounds(viewBox: glmatrix.vec4): void {
        if (this.renderer instanceof BenchmarkSVGRenderer)
            this.renderer.initCameraBounds(viewBox);
    }

    protected renderingFinished(): void {
        if (this.renderingPromiseCallback == null)
            return;

        const appController = this.appController;
        let time = this.renderer.lastTimings.rendering * 1000.0;
        if (appController.mode === 'text')
            time /= unwrapNull(appController.textRun).glyphIDs.length;
        this.renderingPromiseCallback(time);
    }
}

class BenchmarkTextRenderer extends Renderer {
    renderContext!: BenchmarkTestView;

    camera: OrthographicCamera;

    needsStencil: boolean = false;
    isMulticolor: boolean = false;

    get destFramebuffer(): WebGLFramebuffer | null {
        return null;
    }

    get bgColor(): glmatrix.vec4 {
        return glmatrix.vec4.clone([1.0, 1.0, 1.0, 0.0]);
    }

    get fgColor(): glmatrix.vec4 {
        return glmatrix.vec4.clone([0.0, 0.0, 0.0, 1.0]);
    }

    get destAllocatedSize(): glmatrix.vec2 {
        const canvas = this.renderContext.canvas;
        return glmatrix.vec2.clone([canvas.width, canvas.height]);
    }

    get destUsedSize(): glmatrix.vec2 {
        return this.destAllocatedSize;
    }

    get allowSubpixelAA(): boolean {
        return true;
    }

    get emboldenAmount(): glmatrix.vec2 {
        return this.stemDarkeningAmount;
    }

    get pixelsPerEm(): number {
        return this._pixelsPerEm;
    }

    set pixelsPerEm(newPixelsPerEm: number) {
        this._pixelsPerEm = newPixelsPerEm;
        this.uploadPathTransforms(1);
        this.renderContext.setDirty();
    }

    protected get usedSizeFactor(): glmatrix.vec2 {
        return glmatrix.vec2.clone([1.0, 1.0]);
    }

    protected get worldTransform(): glmatrix.mat4 {
        const canvas = this.renderContext.canvas;

        const transform = glmatrix.mat4.create();
        const translation = this.camera.translation;
        glmatrix.mat4.translate(transform, transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform, transform, [2.0 / canvas.width, 2.0 / canvas.height, 1.0]);
        glmatrix.mat4.translate(transform, transform, [translation[0], translation[1], 0]);
        glmatrix.mat4.scale(transform, transform, [this.camera.scale, this.camera.scale, 1.0]);

        const pixelsPerUnit = this.pixelsPerUnit;
        glmatrix.mat4.scale(transform, transform, [pixelsPerUnit, pixelsPerUnit, 1.0]);

        return transform;
    }

    protected get objectCount(): number {
        return this.meshBuffers == null ? 0 : this.meshBuffers.length;
    }

    private _pixelsPerEm: number = 32.0;

    private get pixelsPerUnit(): number {
        const font = unwrapNull(this.renderContext.appController.font);
        return this._pixelsPerEm / font.opentypeFont.unitsPerEm;
    }

    private get stemDarkeningAmount(): glmatrix.vec2 {
        return computeStemDarkeningAmount(this._pixelsPerEm, this.pixelsPerUnit);
    }

    constructor(renderContext: BenchmarkTestView) {
        super(renderContext);

        this.camera = new OrthographicCamera(renderContext.canvas, { fixed: true });
        this.camera.onPan = () => renderContext.setDirty();
        this.camera.onZoom = () => renderContext.setDirty();
    }

    attachMeshes(meshes: PathfinderPackedMeshes[]): void {
        super.attachMeshes(meshes);

        this.uploadPathColors(1);
        this.uploadPathTransforms(1);
    }

    pathCountForObject(objectIndex: number): number {
        return STRING.length;
    }

    pathBoundingRects(objectIndex: number): Float32Array {
        const appController = this.renderContext.appController;
        const font = unwrapNull(appController.font);

        const boundingRects = new Float32Array((STRING.length + 1) * 4);

        for (let glyphIndex = 0; glyphIndex < STRING.length; glyphIndex++) {
            const glyphID = unwrapNull(appController.textRun).glyphIDs[glyphIndex];

            const metrics = font.metricsForGlyph(glyphID);
            if (metrics == null)
                continue;

            boundingRects[(glyphIndex + 1) * 4 + 0] = metrics.xMin;
            boundingRects[(glyphIndex + 1) * 4 + 1] = metrics.yMin;
            boundingRects[(glyphIndex + 1) * 4 + 2] = metrics.xMax;
            boundingRects[(glyphIndex + 1) * 4 + 3] = metrics.yMax;
        }

        return boundingRects;
    }

    setHintsUniform(uniforms: UniformMap): void {
        this.renderContext.gl.uniform4f(uniforms.uHints, 0, 0, 0, 0);
    }

    pathTransformsForObject(objectIndex: number): PathTransformBuffers<Float32Array> {
        const appController = this.renderContext.appController;
        const canvas = this.renderContext.canvas;
        const font = unwrapNull(appController.font);

        const pathTransforms = this.createPathTransformBuffers(STRING.length);

        let currentX = 0, currentY = 0;
        const availableWidth = canvas.width / this.pixelsPerUnit;
        const lineHeight = font.opentypeFont.lineHeight();

        for (let glyphIndex = 0; glyphIndex < STRING.length; glyphIndex++) {
            const glyphID = unwrapNull(appController.textRun).glyphIDs[glyphIndex];
            pathTransforms.st.set([1, 1, currentX, currentY], (glyphIndex + 1) * 4);

            currentX += font.opentypeFont.glyphs.get(glyphID).advanceWidth;
            if (currentX > availableWidth) {
                currentX = 0;
                currentY += lineHeight;
            }
        }

        return pathTransforms;
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName,
                               aaLevel: number,
                               subpixelAA: SubpixelAAType):
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

    protected directCurveProgramName(): keyof ShaderMap<void> {
        return 'directCurve';
    }

    protected directInteriorProgramName(): keyof ShaderMap<void> {
        return 'directInterior';
    }
}

class BenchmarkSVGRenderer extends SVGRenderer {
    renderContext!: BenchmarkTestView;

    protected get loader(): SVGLoader {
        return this.renderContext.appController.svgLoader;
    }

    protected get canvas(): HTMLCanvasElement {
        return this.renderContext.canvas;
    }

    constructor(renderContext: BenchmarkTestView) {
        super(renderContext, {sizeToFit: false});
    }
}

function computeMedian(values: number[]): number | null {
    if (values.length === 0)
        return null;
    const mid = values.length / 2;
    if (values.length % 2 === 1)
        return values[Math.floor(mid)];
    return lerp(values[mid - 1], values[mid], 0.5);
}

class ElapsedTime {
    readonly size: number;
    readonly times: number[];

    constructor(size: number) {
        this.size = size;
        this.times = [];
    }

    get time(): number {
        const median = computeMedian(this.times);
        return median == null ? 0.0 : median;
    }

    get timeInMS(): number {
        return this.time / 1000.0;
    }
}

function main() {
    const controller = new BenchmarkAppController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
