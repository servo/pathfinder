// pathfinder/client/src/3d-demo.ts
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

import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from "./aa-strategy";
import {DemoAppController} from "./app-controller";
import {PerspectiveCamera} from "./camera";
import {mat4, vec2} from "gl-matrix";
import {PathfinderMeshData} from "./meshes";
import {ShaderMap, ShaderProgramSource} from "./shader-loader";
import {BUILTIN_FONT_URI, ExpandedMeshData, GlyphStorage, PathfinderGlyph} from "./text";
import {SimpleTextLayout, TextFrame, TextRun} from "./text";
import {PathfinderError, assert, panic, unwrapNull} from "./utils";
import {PathfinderDemoView, Timings} from "./view";
import SSAAStrategy from "./ssaa-strategy";
import * as _ from "lodash";
import PathfinderBufferTexture from "./buffer-texture";

const WIDTH: number = 150000;

const TEXT_DATA_URI: string = "/data/mozmonument.json";

const FONT: string = 'open-sans';

const PIXELS_PER_UNIT: number = 1.0;

const FOV: number = 45.0;
const NEAR_CLIP_PLANE: number = 0.01;
const FAR_CLIP_PLANE: number = 200000.0;

const SCALE: glmatrix.vec3 = glmatrix.vec3.fromValues(1.0 / 200.0, 1.0 / 200.0, 1.0 / 200.0);

const TEXT_TRANSLATION: number[] = [-WIDTH * 0.5, 0.0, WIDTH * 0.5];

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
};

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
}

interface TextLine {
    names: string[];
}

interface MonumentSide {
    lines: TextLine[];
}

class ThreeDController extends DemoAppController<ThreeDView> {
    start() {
        super.start();

        this.monumentPromise = window.fetch(TEXT_DATA_URI)
                                     .then(response => response.json())
                                     .then(textData => this.parseTextData(textData));

        this.loadInitialFile();
    }

    private parseTextData(textData: any): MonumentSide[] {
        const sides = [];
        for (let sideIndex = 0; sideIndex < 4; sideIndex++)
            sides[sideIndex] = { upper: { lines: [] }, lower: { lines: [] } };

        for (const nameData of textData.monument) {
            const side = parseInt(nameData.side) - 1;
            const row = parseInt(nameData.row) - 1;
            const number = parseInt(nameData.number) - 1;

            if (sides[side] == null)
                continue;

            const lines: TextLine[] = sides[side][nameData.panel as ('upper' | 'lower')].lines;
            if (lines[row] == null)
                lines[row] = { names: [] };

            lines[row].names[number] = nameData.name;
        }

        return sides.map(side => ({ lines: side.upper.lines.concat(side.lower.lines) }));
    }

    protected fileLoaded(): void {
        const font = opentype.parse(this.fileData);
        assert(font.isSupported(), "The font type is unsupported!");

        this.monumentPromise.then(monument => this.layoutMonument(font, monument));
    }

    private layoutMonument(font: opentype.Font, monument: MonumentSide[]) {
        const createGlyph = (glyph: opentype.Glyph) => new ThreeDGlyph(glyph);
        let textFrames = [];
        for (const monumentSide of monument) {
            let textRuns = [];
            for (let lineNumber = 0; lineNumber < monumentSide.lines.length; lineNumber++) {
                const line = monumentSide.lines[lineNumber];

                const lineY = -lineNumber * font.lineHeight();
                const lineGlyphs = line.names.map(string => {
                    const glyphs = font.stringToGlyphs(string).map(createGlyph);
                    return { glyphs: glyphs, width: _.sumBy(glyphs, glyph => glyph.advanceWidth) };
                });

                const usedSpace = _.sumBy(lineGlyphs, 'width');
                const emptySpace = Math.max(WIDTH - usedSpace, 0.0);
                const spacing = emptySpace / Math.max(lineGlyphs.length - 1, 1);

                let currentX = 0.0;
                for (const glyphInfo of lineGlyphs) {
                    const textRunOrigin = [currentX, lineY];
                    textRuns.push(new TextRun(glyphInfo.glyphs, textRunOrigin, font, createGlyph));
                    currentX += glyphInfo.width + spacing;
                }
            }

            textFrames.push(new TextFrame(textRuns));
        }

        this.glyphStorage = new GlyphStorage(this.fileData, textFrames, createGlyph, font);
        this.glyphStorage.layoutRuns();

        this.glyphStorage.partition().then((baseMeshes: PathfinderMeshData) => {
            this.baseMeshes = baseMeshes;
            this.expandedMeshes = this.glyphStorage.expandMeshes(baseMeshes);
            this.view.then(view => {
                view.uploadPathMetadata();
                view.attachMeshes(this.expandedMeshes.map(meshes => meshes.meshes));
            });
        });
    }

    protected createView(): ThreeDView {
        return new ThreeDView(this,
                              unwrapNull(this.commonShaderSource),
                              unwrapNull(this.shaderSources));
    }

    protected get builtinFileURI(): string {
        return BUILTIN_FONT_URI;
    }

    protected get defaultFile(): string {
        return FONT;
    }

    glyphStorage: GlyphStorage<ThreeDGlyph>;

    private baseMeshes: PathfinderMeshData;
    private expandedMeshes: ExpandedMeshData[];

    private monumentPromise: Promise<MonumentSide[]>;
}

class ThreeDView extends PathfinderDemoView {
    constructor(appController: ThreeDController,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(commonShaderSource, shaderSources);

        this.appController = appController;

        this.camera = new PerspectiveCamera(this.canvas);
        this.camera.onChange = () => this.setDirty();
    }

    uploadPathMetadata() {
        this.pathColorsBufferTextures = [];
        this.pathTransformBufferTextures = [];

        const textFrameCount = this.appController.glyphStorage.textFrames.length;
        for (let textFrameIndex = 0;
             textFrameIndex < textFrameCount;
             textFrameIndex++) {
            const textFrame = this.appController.glyphStorage.textFrames[textFrameIndex];
            const textGlyphs = textFrame.allGlyphs;
            const pathCount = textGlyphs.length;

            const pathColors = new Uint8Array(4 * (pathCount + 1));
            const pathTransforms = new Float32Array(4 * (pathCount + 1));

            for (let pathIndex = 0; pathIndex < pathCount; pathIndex++) {
                const startOffset = (pathIndex + 1) * 4;

                for (let channel = 0; channel < 3; channel++)
                    pathColors[startOffset + channel] = 0x00; // RGB
                pathColors[startOffset + 3] = 0xff;           // alpha

                const textGlyph = textGlyphs[pathIndex];
                const glyphRect = textGlyph.pixelRect(PIXELS_PER_UNIT);
                pathTransforms.set([1, 1, glyphRect[0], glyphRect[1]], startOffset);
            }

            const pathColorsBufferTexture = new PathfinderBufferTexture(this.gl, 'uPathColors');
            const pathTransformBufferTexture = new PathfinderBufferTexture(this.gl,
                                                                           'uPathTransform');
            pathColorsBufferTexture.upload(this.gl, pathColors);
            pathTransformBufferTexture.upload(this.gl, pathTransforms);
            this.pathColorsBufferTextures.push(pathColorsBufferTexture);
            this.pathTransformBufferTextures.push(pathTransformBufferTexture);
        }
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName,
                               aaLevel: number,
                               subpixelAA: boolean):
                               AntialiasingStrategy {
        if (aaType !== 'ecaa')
            return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel, subpixelAA);
        throw new PathfinderError("Unsupported antialiasing type!");
    }

    protected compositeIfNecessary(): void {}

    protected updateTimings(timings: Timings) {
        // TODO(pcwalton)
    }

    get destAllocatedSize(): glmatrix.vec2 {
        return glmatrix.vec2.fromValues(this.canvas.width, this.canvas.height);
    }

    get destFramebuffer(): WebGLFramebuffer | null {
        return null;
    }

    get destUsedSize(): glmatrix.vec2 {
        return this.destAllocatedSize;
    }

    protected get usedSizeFactor(): glmatrix.vec2 {
        return glmatrix.vec2.fromValues(1.0, 1.0);
    }

    protected get worldTransform() {
        const projection = glmatrix.mat4.create();
        glmatrix.mat4.perspective(projection,
                                  FOV / 180.0 * Math.PI,
                                  this.canvas.width / this.canvas.height,
                                  NEAR_CLIP_PLANE,
                                  FAR_CLIP_PLANE);

        const modelview = glmatrix.mat4.create();
        glmatrix.mat4.mul(modelview, modelview, this.camera.rotationMatrix);
        glmatrix.mat4.translate(modelview, modelview, this.camera.translation);
        glmatrix.mat4.scale(modelview, modelview, SCALE);

        const transform = glmatrix.mat4.create();
        glmatrix.mat4.mul(transform, projection, modelview);
        return transform;
    }

    protected getModelviewTransform(objectIndex: number): glmatrix.mat4 {
        const transform = glmatrix.mat4.create();
        glmatrix.mat4.rotateY(transform, transform, Math.PI / 2.0 * objectIndex);
        glmatrix.mat4.translate(transform, transform, TEXT_TRANSLATION);
        return transform;
    }

    protected get directCurveProgramName(): keyof ShaderMap<void> {
        return 'direct3DCurve';
    }

    protected get directInteriorProgramName(): keyof ShaderMap<void> {
        return 'direct3DInterior';
    }

    private _scale: number;

    private appController: ThreeDController;

    camera: PerspectiveCamera;
}

class ThreeDGlyph extends PathfinderGlyph {
    constructor(glyph: opentype.Glyph) {
        super(glyph);
    }
}

function main() {
    const controller = new ThreeDController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
