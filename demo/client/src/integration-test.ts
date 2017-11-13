// pathfinder/client/src/integration-test.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';

import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from './aa-strategy';
import {SubpixelAAType} from './aa-strategy';
import {DemoAppController} from "./app-controller";
import {OrthographicCamera} from './camera';
import {UniformMap} from './gl-utils';
import {PathfinderMeshData} from './meshes';
import {Renderer} from "./renderer";
import {ShaderMap, ShaderProgramSource} from "./shader-loader";
import SSAAStrategy from './ssaa-strategy';
import {BUILTIN_FONT_URI, computeStemDarkeningAmount, ExpandedMeshData, GlyphStore} from "./text";
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

const STRING: string = "A";

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
    xcaa: typeof AdaptiveMonochromeXCAAStrategy;
}

class IntegrationTestAppController extends DemoAppController<IntegrationTestView> {
    font: PathfinderFont | null;
    textRun: TextRun | null;

    protected readonly defaultFile: string = FONT;
    protected readonly builtinFileURI: string = BUILTIN_FONT_URI;

    private glyphStore: GlyphStore;
    private baseMeshes: PathfinderMeshData;
    private expandedMeshes: ExpandedMeshData;

    start(): void {
        super.start();

        this.loadInitialFile(this.builtinFileURI);
    }

    protected createView(): IntegrationTestView {
        return new IntegrationTestView(this,
                                       unwrapNull(this.gammaLUT),
                                       unwrapNull(this.commonShaderSource),
                                       unwrapNull(this.shaderSources));
    }

    protected fileLoaded(fileData: ArrayBuffer, builtinName: string | null): void {
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

            const expandedMeshes = textFrame.expandMeshes(this.baseMeshes, glyphIDs);
            this.expandedMeshes = expandedMeshes;

            this.view.then(view => {
                view.attachMeshes([expandedMeshes.meshes]);
            });
        });
    }
}

class IntegrationTestView extends DemoView {
    readonly renderer: IntegrationTestRenderer;
    readonly appController: IntegrationTestAppController;

    get camera(): OrthographicCamera {
        return this.renderer.camera;
    }

    constructor(appController: IntegrationTestAppController,
                gammaLUT: HTMLImageElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(gammaLUT, commonShaderSource, shaderSources);

        this.appController = appController;
        this.renderer = new IntegrationTestRenderer(this);

        this.resizeToFit(true);
    }
}

class IntegrationTestRenderer extends Renderer {
    renderContext: IntegrationTestView;
    camera: OrthographicCamera;

    private _pixelsPerEm: number = 32.0;

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
        return this.meshes.length;
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
        const font = unwrapNull(this.renderContext.appController.font);
        return this._pixelsPerEm / font.opentypeFont.unitsPerEm;
    }

    private get stemDarkeningAmount(): glmatrix.vec2 {
        return computeStemDarkeningAmount(this._pixelsPerEm, this.pixelsPerUnit);
    }

    constructor(renderContext: IntegrationTestView) {
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

    protected createAAStrategy(aaType: AntialiasingStrategyName,
                               aaLevel: number,
                               subpixelAA: SubpixelAAType):
                               AntialiasingStrategy {
        return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel, subpixelAA);
    }

    protected compositeIfNecessary(): void {}

    protected pathColorsForObject(objectIndex: number): Uint8Array {
        const pathColors = new Uint8Array(4 * (STRING.length + 1));
        for (let pathIndex = 0; pathIndex < STRING.length; pathIndex++)
            pathColors.set(TEXT_COLOR, (pathIndex + 1) * 4);
        return pathColors;
    }

    protected pathTransformsForObject(objectIndex: number): Float32Array {
        const appController = this.renderContext.appController;
        const canvas = this.renderContext.canvas;
        const font = unwrapNull(appController.font);

        const pathTransforms = new Float32Array(4 * (STRING.length + 1));

        let currentX = 0, currentY = 0;
        const availableWidth = canvas.width / this.pixelsPerUnit;
        const lineHeight = font.opentypeFont.lineHeight();

        for (let glyphIndex = 0; glyphIndex < STRING.length; glyphIndex++) {
            const glyphID = unwrapNull(appController.textRun).glyphIDs[glyphIndex];
            pathTransforms.set([1, 1, currentX, currentY], (glyphIndex + 1) * 4);

            currentX += font.opentypeFont.glyphs.get(glyphID).advanceWidth;
            if (currentX > availableWidth) {
                currentX = 0;
                currentY += lineHeight;
            }
        }

        return pathTransforms;
    }

    protected directCurveProgramName(): keyof ShaderMap<void> {
        return 'directCurve';
    }

    protected directInteriorProgramName(): keyof ShaderMap<void> {
        return 'directInterior';
    }
}

function main() {
    const controller = new IntegrationTestAppController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
