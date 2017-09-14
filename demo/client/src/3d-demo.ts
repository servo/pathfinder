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
import {Hint, SimpleTextLayout, TextFrame, TextRun} from "./text";
import {PathfinderError, assert, panic, unwrapNull} from "./utils";
import {PathfinderDemoView, Timings} from "./view";
import SSAAStrategy from "./ssaa-strategy";
import * as _ from "lodash";
import PathfinderBufferTexture from "./buffer-texture";

const TEXT_AVAILABLE_WIDTH: number = 150000;
const TEXT_PADDING: number = 2000;

const TEXT_SCALE: glmatrix.vec3 = glmatrix.vec3.fromValues(1.0 / 200.0, 1.0 / 200.0, 1.0 / 200.0);

const TEXT_DATA_URI: string = "/data/mozmonument.json";

const FONT: string = 'open-sans';

const PIXELS_PER_UNIT: number = 1.0;

const FOV: number = 45.0;
const NEAR_CLIP_PLANE: number = 0.1;
const FAR_CLIP_PLANE: number = 10000.0;

const TEXT_TRANSLATION: number[] = [
    -TEXT_AVAILABLE_WIDTH * 0.5,
    0.0,
    TEXT_AVAILABLE_WIDTH * 0.5 + TEXT_PADDING,
];

const MONUMENT_TRANSLATION: glmatrix.vec3 = glmatrix.vec3.fromValues(0.0, -690.0, 0.0);
const MONUMENT_SCALE: glmatrix.vec3 =
    glmatrix.vec3.fromValues((TEXT_AVAILABLE_WIDTH * 0.5 + TEXT_PADDING) * TEXT_SCALE[0],
                             700.0,
                             (TEXT_AVAILABLE_WIDTH * 0.5 + TEXT_PADDING) * TEXT_SCALE[2]);

const TEXT_COLOR: Uint8Array = new Uint8Array([0xf2, 0xf8, 0xf8, 0xff]);
const MONUMENT_COLOR: number[] = [0x70 / 0xff, 0x80 / 0xff, 0x80 / 0xff];

const CUBE_VERTEX_POSITIONS: Float32Array = new Float32Array([
    -1.0, -1.0, -1.0,  // 0
     1.0, -1.0, -1.0,  // 1
    -1.0, -1.0,  1.0,  // 2
     1.0, -1.0,  1.0,  // 3
    -1.0,  1.0, -1.0,  // 4
     1.0,  1.0, -1.0,  // 5
    -1.0,  1.0,  1.0,  // 6
     1.0,  1.0,  1.0,  // 7
]);

const CUBE_INDICES: Uint16Array = new Uint16Array([
    0, 1, 2, 2, 1, 3,   // bottom
    0, 5, 1, 0, 4, 5,   // front
    2, 4, 0, 2, 6, 4,   // left
    3, 5, 1, 3, 7, 5,   // right
    2, 7, 3, 2, 6, 7,   // back
    4, 5, 6, 6, 5, 7,   // top
]);

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
                const emptySpace = Math.max(TEXT_AVAILABLE_WIDTH - usedSpace, 0.0);
                const spacing = emptySpace / Math.max(lineGlyphs.length - 1, 1);

                let currentX = 0.0;
                for (const glyphInfo of lineGlyphs) {
                    const textRunOrigin = [currentX, lineY];
                    textRuns.push(new TextRun(glyphInfo.glyphs, textRunOrigin, font, createGlyph));
                    currentX += glyphInfo.width + spacing;
                }
            }

            textFrames.push(new TextFrame(textRuns, font));
        }

        this.glyphStorage = new GlyphStorage(this.fileData, textFrames, createGlyph, font);
        this.glyphStorage.layoutRuns();

        this.glyphStorage.partition().then((baseMeshes: PathfinderMeshData) => {
            this.baseMeshes = baseMeshes;
            this.expandedMeshes = this.glyphStorage.expandMeshes(baseMeshes);
            this.view.then(view => {
                view.uploadPathColors(this.expandedMeshes.length);
                view.uploadPathTransforms(this.expandedMeshes.length);
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

        this.cubeVertexPositionBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.cubeVertexPositionBuffer);
        this.gl.bufferData(this.gl.ARRAY_BUFFER, CUBE_VERTEX_POSITIONS, this.gl.STATIC_DRAW);

        this.cubeIndexBuffer = unwrapNull(this.gl.createBuffer());
        this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.cubeIndexBuffer);
        this.gl.bufferData(this.gl.ELEMENT_ARRAY_BUFFER, CUBE_INDICES, this.gl.STATIC_DRAW);
    }

    protected pathColorsForObject(textFrameIndex: number): Uint8Array {
        const textFrame = this.appController.glyphStorage.textFrames[textFrameIndex];
        const textGlyphs = textFrame.allGlyphs;
        const pathCount = textGlyphs.length;
        
        const pathColors = new Uint8Array(4 * (pathCount + 1));
        for (let pathIndex = 0; pathIndex < pathCount; pathIndex++)
            pathColors.set(TEXT_COLOR, (pathIndex + 1) * 4);

        return pathColors;
    }

    protected pathTransformsForObject(textFrameIndex: number): Float32Array {
        const textFrame = this.appController.glyphStorage.textFrames[textFrameIndex];
        const textGlyphs = textFrame.allGlyphs;
        const pathCount = textGlyphs.length;
        
        const hint = new Hint(this.appController.glyphStorage.font, PIXELS_PER_UNIT, false);

        const pathTransforms = new Float32Array(4 * (pathCount + 1));

        for (let pathIndex = 0; pathIndex < pathCount; pathIndex++) {
            const textGlyph = textGlyphs[pathIndex];
            const glyphOrigin = textGlyph.calculatePixelOrigin(hint, PIXELS_PER_UNIT);
            pathTransforms.set([1, 1, glyphOrigin[0], glyphOrigin[1]], (pathIndex + 1) * 4);
        }

        return pathTransforms;
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName,
                               aaLevel: number,
                               subpixelAA: boolean):
                               AntialiasingStrategy {
        if (aaType !== 'ecaa')
            return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel, subpixelAA);
        throw new PathfinderError("Unsupported antialiasing type!");
    }

    protected drawSceneryIfNecessary(): void {
        // Set up the cube VBO.
        const shaderProgram = this.shaderPrograms.demo3DMonument;
        this.gl.useProgram(shaderProgram.program);
        this.gl.bindBuffer(this.gl.ARRAY_BUFFER, this.cubeVertexPositionBuffer);
        this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, this.cubeIndexBuffer);
        this.gl.vertexAttribPointer(shaderProgram.attributes.aPosition,
                                    3,
                                    this.gl.FLOAT,
                                    false,
                                    0,
                                    0);
        this.gl.enableVertexAttribArray(shaderProgram.attributes.aPosition);

        // Set uniforms for the monument.
        const transform = this.calculateWorldTransform(MONUMENT_TRANSLATION, MONUMENT_SCALE);
        this.gl.uniformMatrix4fv(shaderProgram.uniforms.uTransform, false, transform);
        this.gl.uniform4f(shaderProgram.uniforms.uColor,
                          MONUMENT_COLOR[0],
                          MONUMENT_COLOR[1],
                          MONUMENT_COLOR[2],
                          1.0);

        // Set state for the monument.
        this.gl.enable(this.gl.DEPTH_TEST);
        this.gl.depthMask(true);
        this.gl.disable(this.gl.SCISSOR_TEST);

        // Draw the monument!
        this.gl.drawElements(this.gl.TRIANGLES, CUBE_INDICES.length, this.gl.UNSIGNED_SHORT, 0);

        // Clear to avoid Z-fighting.
        this.gl.clearDepth(1.0);
        this.gl.clear(this.gl.DEPTH_BUFFER_BIT);
    }

    protected compositeIfNecessary(): void {}

    protected newTimingsReceived() {
        this.appController.newTimingsReceived(_.pick(this.lastTimings, ['rendering']));
    }

    get destAllocatedSize(): glmatrix.vec2 {
        return glmatrix.vec2.fromValues(this.canvas.width, this.canvas.height);
    }

    destFramebuffer: WebGLFramebuffer | null = null;

    get destUsedSize(): glmatrix.vec2 {
        return this.destAllocatedSize;
    }

    protected usedSizeFactor: glmatrix.vec2 = glmatrix.vec2.clone([1.0, 1.0]);

    private calculateWorldTransform(modelviewTranslation: glmatrix.vec3,
                                    modelviewScale: glmatrix.vec3):
                                    glmatrix.mat4 {
        const projection = glmatrix.mat4.create();
        glmatrix.mat4.perspective(projection,
                                  FOV / 180.0 * Math.PI,
                                  this.canvas.width / this.canvas.height,
                                  NEAR_CLIP_PLANE,
                                  FAR_CLIP_PLANE);

        const modelview = glmatrix.mat4.create();
        glmatrix.mat4.mul(modelview, modelview, this.camera.rotationMatrix);
        glmatrix.mat4.translate(modelview, modelview, this.camera.translation);
        glmatrix.mat4.translate(modelview, modelview, modelviewTranslation);
        glmatrix.mat4.scale(modelview, modelview, modelviewScale);

        const transform = glmatrix.mat4.create();
        glmatrix.mat4.mul(transform, projection, modelview);
        return transform;
    }

    protected clearForResolve(): void {
        this.gl.clearColor(1.0, 1.0, 1.0, 1.0);
        this.gl.clearDepth(1.0);
        this.gl.depthMask(true);
        this.gl.clear(this.gl.COLOR_BUFFER_BIT | this.gl.DEPTH_BUFFER_BIT);
    }

    protected get worldTransform() {
        return this.calculateWorldTransform(glmatrix.vec3.create(), TEXT_SCALE);
    }

    protected getModelviewTransform(objectIndex: number): glmatrix.mat4 {
        const transform = glmatrix.mat4.create();
        glmatrix.mat4.rotateY(transform, transform, Math.PI / 2.0 * objectIndex);
        glmatrix.mat4.translate(transform, transform, TEXT_TRANSLATION);
        return transform;
    }

    // Cheap but effective backface culling.
    protected shouldRenderObject(objectIndex: number): boolean {
        const translation = this.camera.translation;
        const extent = TEXT_TRANSLATION[2] * TEXT_SCALE[2];
        switch (objectIndex) {
        case 0:     return translation[2] < -extent;
        case 1:     return translation[0] < -extent;
        case 2:     return translation[2] > extent;
        default:    return translation[0] > extent;
        }
    }

    protected directCurveProgramName: keyof ShaderMap<void> = 'direct3DCurve';
    protected directInteriorProgramName: keyof ShaderMap<void> = 'direct3DInterior';

    protected depthFunction: number = this.gl.LESS;

    private _scale: number;

    private appController: ThreeDController;

    private cubeVertexPositionBuffer: WebGLBuffer;
    private cubeIndexBuffer: WebGLBuffer;

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
