// pathfinder/client/src/reference-test.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';
import * as imageSSIM from 'image-ssim';
import * as _ from 'lodash';
import * as papaparse from 'papaparse';

import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from './aa-strategy';
import {SubpixelAAType} from './aa-strategy';
import {DemoAppController} from "./app-controller";
import {SUBPIXEL_GRANULARITY} from './atlas';
import {OrthographicCamera} from './camera';
import {UniformMap} from './gl-utils';
import {PathfinderMeshData} from './meshes';
import {PathTransformBuffers, Renderer} from "./renderer";
import {ShaderMap, ShaderProgramSource} from "./shader-loader";
import SSAAStrategy from './ssaa-strategy';
import {BUILTIN_FONT_URI, computeStemDarkeningAmount, ExpandedMeshData, GlyphStore} from "./text";
import {Hint} from "./text";
import {PathfinderFont, TextFrame, TextRun} from "./text";
import {unwrapNull} from "./utils";
import {DemoView} from "./view";
import {AdaptiveMonochromeXCAAStrategy} from './xcaa-strategy';

const FONT: string = 'open-sans';
const TEXT_COLOR: number[] = [0, 0, 0, 255];

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
    xcaa: AdaptiveMonochromeXCAAStrategy,
};

const RENDER_REFERENCE_URI: string = "/render-reference";
const TEST_DATA_URI: string = "/test-data/reference-test-text.csv";

const SSIM_TOLERANCE: number = 0.01;
const SSIM_WINDOW_SIZE: number = 8;

const FILES: PerTestType<File[]> = {
    font: [
        { id: 'open-sans', title: "Open Sans" },
        { id: 'eb-garamond', title: "EB Garamond" },
        { id: 'nimbus-sans', title: "Nimbus Sans" },
    ],
    svg: [
        { id: 'tiger', title: "Ghostscript Tiger" },
    ],
};

interface ReferenceTestGroup {
    font: string;
    tests: ReferenceTestCase[];
}

interface ReferenceTestCase {
    size: number;
    character: string;
    aaMode: keyof AntialiasingStrategyTable;
    subpixel: boolean;
    referenceRenderer: ReferenceRenderer;
    expectedSSIM: number;
}

interface PerTestType<T> {
    font: T;
    svg: T;
}

interface File {
    id: string;
    title: string;
}

type ReferenceRenderer = 'core-graphics' | 'freetype';

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
    xcaa: typeof AdaptiveMonochromeXCAAStrategy;
}

class ReferenceTestAppController extends DemoAppController<ReferenceTestView> {
    font: PathfinderFont | null;
    textRun: TextRun | null;

    referenceCanvas: HTMLCanvasElement;

    tests: Promise<ReferenceTestGroup[]>;

    protected readonly defaultFile: string = FONT;
    protected readonly builtinFileURI: string = BUILTIN_FONT_URI;

    private glyphStore: GlyphStore;
    private baseMeshes: PathfinderMeshData;
    private expandedMeshes: ExpandedMeshData;

    private fontSizeInput: HTMLInputElement;
    private characterInput: HTMLInputElement;
    private referenceRendererSelect: HTMLSelectElement;

    private differenceCanvas: HTMLCanvasElement;

    private aaLevelGroup: HTMLElement;

    private customTabs: PerTestType<HTMLElement>;
    private customTestForms: PerTestType<HTMLFormElement>;
    private selectFileGroups: PerTestType<HTMLElement>;
    private runTestsButtons: PerTestType<HTMLButtonElement>;
    private ssimGroups: PerTestType<HTMLElement>;
    private ssimLabels: PerTestType<HTMLElement>;
    private resultsTables: PerTestType<HTMLTableElement>;

    private currentTestType: 'font' | 'svg';
    private currentTestGroupIndex: number | null;
    private currentTestCaseIndex: number | null;
    private currentGlobalTestCaseIndex: number | null;

    get currentFontSize(): number {
        return parseInt(this.fontSizeInput.value, 10);
    }

    set currentFontSize(newFontSize: number) {
        this.fontSizeInput.value = "" + newFontSize;
    }

    get currentCharacter(): string {
        return this.characterInput.value;
    }

    set currentCharacter(newCharacter: string) {
        this.characterInput.value = newCharacter;
    }

    get currentReferenceRenderer(): ReferenceRenderer {
        return this.referenceRendererSelect.value as ReferenceRenderer;
    }

    set currentReferenceRenderer(newReferenceRenderer: ReferenceRenderer) {
        this.referenceRendererSelect.value = newReferenceRenderer;
    }

    start(): void {
        this.referenceRendererSelect =
            unwrapNull(document.getElementById('pf-font-reference-renderer')) as HTMLSelectElement;
        this.referenceRendererSelect.addEventListener('change', () => {
            this.view.then(view => this.runSingleTest());
        }, false);

        super.start();

        this.currentTestGroupIndex = null;
        this.currentTestCaseIndex = null;
        this.currentGlobalTestCaseIndex = null;

        this.referenceCanvas = unwrapNull(document.getElementById('pf-reference-canvas')) as
            HTMLCanvasElement;

        this.fontSizeInput = unwrapNull(document.getElementById('pf-font-size')) as
            HTMLInputElement;
        this.fontSizeInput.addEventListener('change', () => {
            this.view.then(view => this.runSingleTest());
        }, false);

        this.characterInput = unwrapNull(document.getElementById('pf-character')) as
            HTMLInputElement;
        this.characterInput.addEventListener('change', () => {
            this.view.then(view => this.runSingleTest());
        }, false);

        this.aaLevelGroup = unwrapNull(document.getElementById('pf-aa-level-group')) as
            HTMLElement;

        this.differenceCanvas = unwrapNull(document.getElementById('pf-difference-canvas')) as
            HTMLCanvasElement;

        this.customTabs = {
            font: unwrapNull(document.getElementById('pf-font-custom-test-tab')) as HTMLElement,
            svg: unwrapNull(document.getElementById('pf-svg-custom-test-tab')) as HTMLElement,
        };
        this.customTestForms = {
            font: unwrapNull(document.getElementById('pf-font-custom-form')) as HTMLFormElement,
            svg: unwrapNull(document.getElementById('pf-svg-custom-form')) as HTMLFormElement,
        };
        this.selectFileGroups = {
            font: unwrapNull(document.getElementById('pf-font-select-file-group')) as HTMLElement,
            svg: unwrapNull(document.getElementById('pf-svg-select-file-group')) as HTMLElement,
        };
        this.runTestsButtons = {
            font: unwrapNull(document.getElementById('pf-run-font-tests-button')) as
                HTMLButtonElement,
            svg: unwrapNull(document.getElementById('pf-run-svg-tests-button')) as
                HTMLButtonElement,
        };
        this.ssimGroups = {
            font: unwrapNull(document.getElementById('pf-font-ssim-group')) as HTMLElement,
            svg: unwrapNull(document.getElementById('pf-svg-ssim-group')) as HTMLElement,
        };
        this.ssimLabels = {
            font: unwrapNull(document.getElementById('pf-font-ssim-label')) as HTMLElement,
            svg: unwrapNull(document.getElementById('pf-svg-ssim-label')) as HTMLElement,
        };
        this.resultsTables = {
            font: unwrapNull(document.getElementById('pf-font-results-table')) as HTMLTableElement,
            svg: unwrapNull(document.getElementById('pf-svg-results-table')) as HTMLTableElement,
        };

        this.customTabs.font.addEventListener('click',
                                              () => this.showCustomTabPane('font'),
                                              false);
        this.customTabs.svg.addEventListener('click', () => this.showCustomTabPane('svg'), false);

        this.runTestsButtons.font.addEventListener('click', () => {
            this.view.then(view => this.runTests());
        }, false);
        this.runTestsButtons.svg.addEventListener('click', () => {
            this.view.then(view => this.runTests());
        }, false);

        this.currentTestType = 'font';

        this.loadTestData();
        this.populateResultsTable();
        this.populateFilesSelect();

        this.loadInitialFile(this.builtinFileURI);
    }

    runNextTestIfNecessary(tests: ReferenceTestGroup[]): void {
        if (this.currentTestGroupIndex == null || this.currentTestCaseIndex == null ||
            this.currentGlobalTestCaseIndex == null) {
            return;
        }

        this.currentTestCaseIndex++;
        this.currentGlobalTestCaseIndex++;
        if (this.currentTestCaseIndex === tests[this.currentTestGroupIndex].tests.length) {
            this.currentTestCaseIndex = 0;
            this.currentTestGroupIndex++;
            if (this.currentTestGroupIndex === tests.length) {
                // Done running tests.
                this.currentTestCaseIndex = null;
                this.currentTestGroupIndex = null;
                this.currentGlobalTestCaseIndex = null;
                this.view.then(view => view.suppressAutomaticRedraw = false);
                return;
            }
        }

        this.loadFontForTestGroupIfNecessary(tests).then(() => {
            this.setOptionsForCurrentTest(tests).then(() => this.runSingleTest());
        });
    }

    recordSSIMResult(tests: ReferenceTestGroup[], ssimResult: imageSSIM.IResult): void {
        const formattedSSIM: string = "" + (Math.round(ssimResult.ssim * 1000.0) / 1000.0);
        this.ssimLabels[this.currentTestType].textContent = formattedSSIM;

        if (this.currentTestGroupIndex == null || this.currentTestCaseIndex == null ||
            this.currentGlobalTestCaseIndex == null) {
            return;
        }

        const testGroup = tests[this.currentTestGroupIndex];
        const expectedSSIM = testGroup.tests[this.currentTestCaseIndex].expectedSSIM;
        const passed = Math.abs(expectedSSIM - ssimResult.ssim) <= SSIM_TOLERANCE;

        const resultsBody: Element = unwrapNull(this.resultsTables[this.currentTestType]
                                                    .lastElementChild);
        let resultsRow = unwrapNull(resultsBody.firstElementChild);
        for (let rowIndex = 0; rowIndex < this.currentGlobalTestCaseIndex; rowIndex++)
            resultsRow = unwrapNull(resultsRow.nextElementSibling);

        const passCell = unwrapNull(resultsRow.firstElementChild);
        const resultsCell = unwrapNull(resultsRow.lastElementChild);
        resultsCell.textContent = formattedSSIM;
        passCell.textContent = passed ? "✓" : "✗";

        resultsRow.classList.remove('table-success', 'table-danger');
        resultsRow.classList.add(passed ? 'table-success' : 'table-danger');
    }

    drawDifferenceImage(differenceImage: imageSSIM.IImage): void {
        const canvas = this.differenceCanvas;
        const context = unwrapNull(canvas.getContext('2d'));
        context.fillStyle = 'white';
        context.fillRect(0, 0, canvas.width, canvas.height);

        const data = new Uint8ClampedArray(differenceImage.data);
        const imageData = new ImageData(data, differenceImage.width, differenceImage.height);
        context.putImageData(imageData, 0, 0);
    }

    protected createView(gammaLUT: HTMLImageElement,
                         commonShaderSource: string,
                         shaderSources: ShaderMap<ShaderProgramSource>):
                         ReferenceTestView {
        return new ReferenceTestView(this, gammaLUT, commonShaderSource, shaderSources);
    }

    protected fileLoaded(fileData: ArrayBuffer, builtinName: string | null): void {
        const font = new PathfinderFont(fileData, builtinName);
        this.font = font;

        // Don't automatically run the test unless this is a custom test.
        if (this.currentGlobalTestCaseIndex == null)
            this.runSingleTest();
    }

    private populateFilesSelect(): void {
        const selectFileElement = unwrapNull(this.selectFileElement);
        while (selectFileElement.lastChild != null)
            selectFileElement.removeChild(selectFileElement.lastChild);

        for (const file of FILES[this.currentTestType]) {
            const option = document.createElement('option');
            option.value = file.id;
            option.appendChild(document.createTextNode(file.title));
            selectFileElement.appendChild(option);
        }
    }

    private loadTestData(): void {
        this.tests = window.fetch(TEST_DATA_URI)
                           .then(response => response.text())
                           .then(testDataText => {
            const fontNames = [];
            const groups: {[font: string]: ReferenceTestCase[]} = {};

            const testData = papaparse.parse(testDataText, {
                comments: "#",
                header: true,
                skipEmptyLines: true,
            });

            for (const row of testData.data) {
                if (!groups.hasOwnProperty(row.Font)) {
                    fontNames.push(row.Font);
                    groups[row.Font] = [];
                }
                groups[row.Font].push({
                    aaMode: row['AA Mode'] as keyof AntialiasingStrategyTable,
                    character: row.Character,
                    expectedSSIM: parseFloat(row['Expected SSIM']),
                    referenceRenderer: row['Reference Renderer'],
                    size: parseInt(row.Size, 10),
                    subpixel: !!row.Subpixel,
                });
            }
            return fontNames.map(fontName => {
                return {
                    font: fontName,
                    tests: groups[fontName],
                };
            });
        });
    }

    private populateResultsTable(): void {
        this.tests.then(tests => {
            const resultsBody: Element = unwrapNull(this.resultsTables[this.currentTestType]
                                                        .lastElementChild);
            for (const testGroup of tests) {
                for (const test of testGroup.tests) {
                    const row = document.createElement('tr');
                    addCell(row, "");
                    addCell(row, testGroup.font);
                    addCell(row, test.character);
                    addCell(row, "" + test.size);
                    addCell(row, "" + test.aaMode);
                    addCell(row, test.subpixel ? "Y" : "N");
                    addCell(row, test.referenceRenderer);
                    addCell(row, "" + test.expectedSSIM);
                    addCell(row, "");
                    resultsBody.appendChild(row);
                }
            }
        });
    }

    private runSingleTest(): void {
        this.setUpTextRun();
        this.loadReference().then(() => this.loadRendering());
    }

    private runTests(): void {
        this.view.then(view => {
            view.suppressAutomaticRedraw = true;
            this.tests.then(tests => {
                this.currentTestGroupIndex = 0;
                this.currentTestCaseIndex = 0;
                this.currentGlobalTestCaseIndex = 0;

                this.loadFontForTestGroupIfNecessary(tests).then(() => {
                    this.setOptionsForCurrentTest(tests).then(() => this.runSingleTest());
                });
            });
        });
    }

    private loadFontForTestGroupIfNecessary(tests: ReferenceTestGroup[]): Promise<void> {
        return new Promise(resolve => {
            if (this.currentTestGroupIndex == null) {
                resolve();
                return;
            }

            this.fetchFile(tests[this.currentTestGroupIndex].font, BUILTIN_FONT_URI).then(() => {
                resolve();
            });
        });
    }

    private setOptionsForCurrentTest(tests: ReferenceTestGroup[]): Promise<void> {
        if (this.currentTestGroupIndex == null || this.currentTestCaseIndex == null)
            return new Promise(resolve => resolve());

        const currentTestCase = tests[this.currentTestGroupIndex].tests[this.currentTestCaseIndex];
        this.currentFontSize = currentTestCase.size;
        this.currentCharacter = currentTestCase.character;
        this.currentReferenceRenderer = currentTestCase.referenceRenderer;

        const aaLevelSelect = unwrapNull(this.aaLevelSelect);
        aaLevelSelect.selectedIndex = _.findIndex(aaLevelSelect.options, option => {
            return option.value.startsWith(currentTestCase.aaMode);
        });

        unwrapNull(this.subpixelAARadioButton).checked = currentTestCase.subpixel;
        return this.updateAALevel();
    }

    private setUpTextRun(): void {
        const font = unwrapNull(this.font);

        const textRun = new TextRun(this.currentCharacter, [0, 0], font);
        textRun.layout();
        this.textRun = textRun;

        this.glyphStore = new GlyphStore(font, [textRun.glyphIDs[0]]);
    }

    private loadRendering(): void {
        this.glyphStore.partition().then(result => {
            const textRun = unwrapNull(this.textRun);

            this.baseMeshes = result.meshes;

            const textFrame = new TextFrame([textRun], unwrapNull(this.font));
            const expandedMeshes = textFrame.expandMeshes(this.baseMeshes, [textRun.glyphIDs[0]]);
            this.expandedMeshes = expandedMeshes;

            this.view.then(view => {
                view.attachMeshes([expandedMeshes.meshes]);
                view.redraw();
            });
        });
    }

    private loadReference(): Promise<void> {
        const request = {
            face: {
                Builtin: unwrapNull(this.font).builtinFontName,
            },
            fontIndex: 0,
            glyph: this.glyphStore.glyphIDs[0],
            pointSize: this.currentFontSize,
            renderer: this.currentReferenceRenderer,
        };

        return window.fetch(RENDER_REFERENCE_URI, {
            body: JSON.stringify(request),
            headers: {'Content-Type': 'application/json'} as any,
            method: 'POST',
        }).then(response => response.blob()).then(blob => {
            const imgElement = document.createElement('img');
            imgElement.src = URL.createObjectURL(blob);
            imgElement.addEventListener('load', () => {
                const canvas = this.referenceCanvas;
                const context = unwrapNull(canvas.getContext('2d'));
                context.fillStyle = 'white';
                context.fillRect(0, 0, canvas.width, canvas.height);
                context.drawImage(imgElement, 0, 0);
            }, false);
        });
    }

    private showCustomTabPane(testType: 'font' | 'svg'): void {
        this.currentTestType = testType;

        const selectFileElement = unwrapNull(this.selectFileElement);
        const aaLevelGroup = unwrapNull(this.aaLevelGroup);

        const customTestForm = this.customTestForms[testType];
        const selectFileGroup = this.selectFileGroups[testType];
        const ssimGroup = this.ssimGroups[testType];

        unwrapNull(selectFileElement.parentNode).removeChild(selectFileElement);
        unwrapNull(aaLevelGroup.parentNode).removeChild(aaLevelGroup);

        selectFileGroup.appendChild(selectFileElement);
        customTestForm.insertBefore(aaLevelGroup, ssimGroup);

        this.populateFilesSelect();
    }
}

class ReferenceTestView extends DemoView {
    readonly renderer: ReferenceTestRenderer;
    readonly appController: ReferenceTestAppController;

    get camera(): OrthographicCamera {
        return this.renderer.camera;
    }

    constructor(appController: ReferenceTestAppController,
                gammaLUT: HTMLImageElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(gammaLUT, commonShaderSource, shaderSources);

        this.appController = appController;
        this.renderer = new ReferenceTestRenderer(this);

        this.resizeToFit(true);
    }

    protected renderingFinished(): void {
        const gl = this.renderContext.gl;
        gl.bindFramebuffer(gl.FRAMEBUFFER, null);

        const pixelRect = this.renderer.getPixelRectForGlyphAt(0);

        const canvasHeight = this.canvas.height;
        const width = pixelRect[2] - pixelRect[0], height = pixelRect[3] - pixelRect[1];
        const originY = Math.max(canvasHeight - height, 0);
        const flippedBuffer = new Uint8Array(width * height * 4);
        gl.readPixels(0, originY, width, height, gl.RGBA, gl.UNSIGNED_BYTE, flippedBuffer);

        const buffer = new Uint8Array(width * height * 4);
        for (let y = 0; y < height; y++) {
            const destRowStart = y * width * 4;
            const srcRowStart = (height - y - 1) * width * 4;
            buffer.set(flippedBuffer.slice(srcRowStart, srcRowStart + width * 4),
                       destRowStart);
        }

        const renderedImage = createSSIMImage(buffer, pixelRect);

        this.appController.tests.then(tests => {
            const referenceImage = createSSIMImage(this.appController.referenceCanvas,
                                                   pixelRect);
            const ssimResult = imageSSIM.compare(referenceImage, renderedImage, SSIM_WINDOW_SIZE);
            const differenceImage = generateDifferenceImage(referenceImage, renderedImage);
            this.appController.recordSSIMResult(tests, ssimResult);
            this.appController.drawDifferenceImage(differenceImage);
            this.appController.runNextTestIfNecessary(tests);
        });
    }
}

class ReferenceTestRenderer extends Renderer {
    renderContext: ReferenceTestView;
    camera: OrthographicCamera;

    get usesSTTransform(): boolean {
        return this.camera.usesSTTransform;
    }

    get destFramebuffer(): WebGLFramebuffer | null {
        return null;
    }

    get bgColor(): glmatrix.vec4 {
        return glmatrix.vec4.clone([1.0, 1.0, 1.0, 1.0]);
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

    get emboldenAmount(): glmatrix.vec2 {
        return this.stemDarkeningAmount;
    }

    protected get objectCount(): number {
        return this.meshes == null ? 0 : this.meshes.length;
    }

    protected get usedSizeFactor(): glmatrix.vec2 {
        return glmatrix.vec2.clone([1.0, 1.0]);
    }

    protected get worldTransform() {
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

    private get pixelsPerUnit(): number {
        const appController = this.renderContext.appController;
        const font = unwrapNull(appController.font);
        return appController.currentFontSize / font.opentypeFont.unitsPerEm;
    }

    private get stemDarkeningAmount(): glmatrix.vec2 {
        const appController = this.renderContext.appController;
        return computeStemDarkeningAmount(appController.currentFontSize, this.pixelsPerUnit);
    }

    constructor(renderContext: ReferenceTestView) {
        super(renderContext);

        this.camera = new OrthographicCamera(renderContext.canvas);
        this.camera.onPan = () => renderContext.setDirty();
        this.camera.onZoom = () => renderContext.setDirty();
    }

    attachMeshes(meshes: PathfinderMeshData[]): void {
        super.attachMeshes(meshes);

        this.uploadPathColors(1);
        this.uploadPathTransforms(1);
    }

    pathCountForObject(objectIndex: number): number {
        return 1;
    }

    pathBoundingRects(objectIndex: number): Float32Array {
        const appController = this.renderContext.appController;
        const font = unwrapNull(appController.font);

        const boundingRects = new Float32Array(2 * 4);

        const glyphID = unwrapNull(appController.textRun).glyphIDs[0];

        const metrics = unwrapNull(font.metricsForGlyph(glyphID));

        boundingRects[4 + 0] = metrics.xMin;
        boundingRects[4 + 1] = metrics.yMin;
        boundingRects[4 + 2] = metrics.xMax;
        boundingRects[4 + 3] = metrics.yMax;

        return boundingRects;
    }

    setHintsUniform(uniforms: UniformMap): void {
        this.renderContext.gl.uniform4f(uniforms.uHints, 0, 0, 0, 0);
    }

    getPixelRectForGlyphAt(glyphIndex: number): glmatrix.vec4 {
        const textRun = unwrapNull(this.renderContext.appController.textRun);
        return textRun.pixelRectForGlyphAt(glyphIndex);
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName,
                               aaLevel: number,
                               subpixelAA: SubpixelAAType):
                               AntialiasingStrategy {
        return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel, subpixelAA);
    }

    protected compositeIfNecessary(): void {}

    protected pathColorsForObject(objectIndex: number): Uint8Array {
        const pathColors = new Uint8Array(4 * 2);
        pathColors.set(TEXT_COLOR, 1 * 4);
        return pathColors;
    }

    protected pathTransformsForObject(objectIndex: number): PathTransformBuffers<Float32Array> {
        const appController = this.renderContext.appController;
        const canvas = this.renderContext.canvas;
        const font = unwrapNull(appController.font);
        const hint = new Hint(font, this.pixelsPerUnit, true);

        const pathTransforms = this.createPathTransformBuffers(1);

        const textRun = unwrapNull(appController.textRun);
        const glyphID = textRun.glyphIDs[0];
        textRun.recalculatePixelRects(this.pixelsPerUnit,
                                      0.0,
                                      hint,
                                      glmatrix.vec2.create(),
                                      SUBPIXEL_GRANULARITY,
                                      glmatrix.vec4.create());
        const pixelRect = textRun.pixelRectForGlyphAt(0);

        const x = -pixelRect[0] / this.pixelsPerUnit;
        const y = (canvas.height - (pixelRect[3] - pixelRect[1])) / this.pixelsPerUnit;

        pathTransforms.st.set([1, 1, x, y], 1 * 4);

        return pathTransforms;
    }

    protected directCurveProgramName(): keyof ShaderMap<void> {
        return 'directCurve';
    }

    protected directInteriorProgramName(): keyof ShaderMap<void> {
        return 'directInterior';
    }
}

function createSSIMImage(image: HTMLCanvasElement | Uint8Array, rect: glmatrix.vec4):
                         imageSSIM.IImage {
    const size = glmatrix.vec2.clone([rect[2] - rect[0], rect[3] - rect[1]]);

    let data;
    if (image instanceof HTMLCanvasElement) {
        const context = unwrapNull(image.getContext('2d'));
        data = new Uint8Array(context.getImageData(0, 0, size[0], size[1]).data);
    } else {
        data = image;
    }

    return {
        channels: imageSSIM.Channels.RGBAlpha,
        data: data,
        height: size[1],
        width: size[0],
    };
}

function generateDifferenceImage(referenceImage: imageSSIM.IImage,
                                 renderedImage: imageSSIM.IImage):
                                 imageSSIM.IImage {
    const differenceImage = new Uint8Array(referenceImage.width * referenceImage.height * 4);
    for (let y = 0; y < referenceImage.height; y++) {
        const rowStart = y * referenceImage.width * 4;
        for (let x = 0; x < referenceImage.width; x++) {
            const pixelStart = rowStart + x * 4;

            let differenceSum = 0;
            for (let channel = 0; channel < 3; channel++) {
                differenceSum += Math.abs(referenceImage.data[pixelStart + channel] -
                                          renderedImage.data[pixelStart + channel]);
            }

            if (differenceSum === 0) {
                // Lighten to indicate no difference.
                for (let channel = 0; channel < 4; channel++) {
                    differenceImage[pixelStart + channel] =
                        Math.floor(referenceImage.data[pixelStart + channel] / 2) + 128;
                }
                continue;
            }

            // Draw differences in red.
            const differenceMean = differenceSum / 3;
            differenceImage[pixelStart + 0] = 127 + Math.round(differenceMean / 2);
            differenceImage[pixelStart + 1] = differenceImage[pixelStart + 2] = 0;
            differenceImage[pixelStart + 3] = 255;
        }
    }
    return {
        channels: referenceImage.channels,
        data: differenceImage,
        height: referenceImage.height,
        width: referenceImage.width,
    };
}

function addCell(row: HTMLTableRowElement, text: string): void {
    const tableCell = document.createElement('td');
    tableCell.textContent = text;
    row.appendChild(tableCell);
}

function main() {
    const controller = new ReferenceTestAppController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
