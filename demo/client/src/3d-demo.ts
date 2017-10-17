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
import * as _ from "lodash";
import * as opentype from "opentype.js";

import {mat4, vec2} from "gl-matrix";
import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from "./aa-strategy";
import {SubpixelAAType} from "./aa-strategy";
import {DemoAppController} from "./app-controller";
import PathfinderBufferTexture from "./buffer-texture";
import {PerspectiveCamera} from "./camera";
import {UniformMap} from './gl-utils';
import {PathfinderMeshData} from "./meshes";
import {Renderer} from './renderer';
import {ShaderMap, ShaderProgramSource} from "./shader-loader";
import SSAAStrategy from "./ssaa-strategy";
import {BUILTIN_FONT_URI, ExpandedMeshData} from "./text";
import {GlyphStore, Hint, PathfinderFont, TextFrame, TextRun} from "./text";
import {assert, panic, PathfinderError, unwrapNull} from "./utils";
import {DemoView, Timings} from "./view";

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

interface MeshDescriptor {
    glyphID: number;
    textFrameIndex: number;
    positions: glmatrix.vec2[];
}

class ThreeDController extends DemoAppController<ThreeDView> {
    textFrames: TextFrame[];
    glyphStore: GlyphStore;
    meshDescriptors: MeshDescriptor[];

    private baseMeshes: PathfinderMeshData;
    private expandedMeshes: PathfinderMeshData[];

    private monumentPromise: Promise<MonumentSide[]>;

    start() {
        super.start();

        this.monumentPromise = window.fetch(TEXT_DATA_URI)
                                     .then(response => response.json())
                                     .then(textData => this.parseTextData(textData));

        this.loadInitialFile(this.builtinFileURI);
    }

    protected fileLoaded(fileData: ArrayBuffer, builtinName: string | null): void {
        const font = new PathfinderFont(fileData, builtinName);
        this.monumentPromise.then(monument => this.layoutMonument(font, fileData, monument));
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

    private parseTextData(textData: any): MonumentSide[] {
        const sides = [];
        for (let sideIndex = 0; sideIndex < 4; sideIndex++)
            sides[sideIndex] = { upper: { lines: [] }, lower: { lines: [] } };

        for (const nameData of textData.monument) {
            const side = parseInt(nameData.side, 10) - 1;
            const row = parseInt(nameData.row, 10) - 1;
            const index = parseInt(nameData.number, 10) - 1;

            if (sides[side] == null)
                continue;

            const lines: TextLine[] = sides[side][nameData.panel as ('upper' | 'lower')].lines;
            if (lines[row] == null)
                lines[row] = { names: [] };

            lines[row].names[index] = nameData.name;
        }

        return sides.map(side => ({ lines: side.upper.lines.concat(side.lower.lines) }));
    }

    private layoutMonument(font: PathfinderFont, fileData: ArrayBuffer, monument: MonumentSide[]) {
        this.textFrames = [];
        let glyphsNeeded: number[] = [];

        for (const monumentSide of monument) {
            const textRuns = [];
            for (let lineNumber = 0; lineNumber < monumentSide.lines.length; lineNumber++) {
                const line = monumentSide.lines[lineNumber];

                const lineY = -lineNumber * font.opentypeFont.lineHeight();
                const lineGlyphs = line.names.map(name => {
                    const glyphs = font.opentypeFont.stringToGlyphs(name);
                    const glyphIDs = glyphs.map(glyph => (glyph as any).index);
                    const width = _.sumBy(glyphs, glyph => glyph.advanceWidth);
                    return { glyphs: glyphIDs, width: width };
                });

                const usedSpace = _.sumBy(lineGlyphs, 'width');
                const emptySpace = Math.max(TEXT_AVAILABLE_WIDTH - usedSpace, 0.0);
                const spacing = emptySpace / Math.max(lineGlyphs.length - 1, 1);

                let currentX = 0.0;
                for (const glyphInfo of lineGlyphs) {
                    const textRunOrigin = [currentX, lineY];
                    const textRun = new TextRun(glyphInfo.glyphs, textRunOrigin, font);
                    textRun.layout();
                    textRuns.push(textRun);
                    currentX += glyphInfo.width + spacing;
                }
            }

            const textFrame = new TextFrame(textRuns, font);
            this.textFrames.push(textFrame);
            glyphsNeeded.push(...textFrame.allGlyphIDs);
        }

        glyphsNeeded.sort((a, b) => a - b);
        glyphsNeeded = _.sortedUniq(glyphsNeeded);

        this.glyphStore = new GlyphStore(font, glyphsNeeded);
        this.glyphStore.partition().then(result => {
            const hint = new Hint(this.glyphStore.font, PIXELS_PER_UNIT, false);

            this.baseMeshes = result.meshes;

            this.meshDescriptors = [];

            for (let textFrameIndex = 0;
                 textFrameIndex < this.textFrames.length;
                 textFrameIndex++) {
                const textFrame = this.textFrames[textFrameIndex];

                let glyphDescriptors = [];
                for (const run of textFrame.runs) {
                    for (let glyphIndex = 0; glyphIndex < run.glyphIDs.length; glyphIndex++) {
                        glyphDescriptors.push({
                            glyphID: run.glyphIDs[glyphIndex],
                            position: run.calculatePixelOriginForGlyphAt(glyphIndex,
                                                                         PIXELS_PER_UNIT,
                                                                         hint),
                        });
                    }
                }

                glyphDescriptors = _.sortBy(glyphDescriptors, descriptor => descriptor.glyphID);

                let currentMeshDescriptor: (MeshDescriptor | null) = null;
                for (const glyphDescriptor of glyphDescriptors) {
                    if (currentMeshDescriptor == null ||
                        glyphDescriptor.glyphID !== currentMeshDescriptor.glyphID) {
                        if (currentMeshDescriptor != null)
                            this.meshDescriptors.push(currentMeshDescriptor);
                        currentMeshDescriptor = {
                            glyphID: glyphDescriptor.glyphID,
                            positions: [],
                            textFrameIndex: textFrameIndex,
                        };
                    }
                    currentMeshDescriptor.positions.push(glyphDescriptor.position);
                }
                if (currentMeshDescriptor != null)
                    this.meshDescriptors.push(currentMeshDescriptor);
            }

            this.expandedMeshes = this.meshDescriptors.map(meshDescriptor => {
                const glyphIndex = _.sortedIndexOf(glyphsNeeded, meshDescriptor.glyphID);
                return this.baseMeshes.expand([glyphIndex + 1]);
            });

            this.view.then(view => view.attachMeshes(this.expandedMeshes));
        });
    }
}

class ThreeDView extends DemoView {
    renderer: ThreeDRenderer;

    appController: ThreeDController;

    protected get camera(): PerspectiveCamera {
        return this.renderer.camera;
    }

    constructor(appController: ThreeDController,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(commonShaderSource, shaderSources);

        this.appController = appController;
        this.renderer = new ThreeDRenderer(this);

        this.resizeToFit(true);
    }
}

class ThreeDRenderer extends Renderer {
    renderContext: ThreeDView;

    camera: PerspectiveCamera;

    get destFramebuffer(): WebGLFramebuffer | null {
        return null;
    }

    get destAllocatedSize(): glmatrix.vec2 {
        return glmatrix.vec2.clone([
            this.renderContext.canvas.width,
            this.renderContext.canvas.height,
        ]);
    }

    get destUsedSize(): glmatrix.vec2 {
        return this.destAllocatedSize;
    }

    protected get directCurveProgramName(): keyof ShaderMap<void> {
        return 'direct3DCurve';
    }

    protected get directInteriorProgramName(): keyof ShaderMap<void> {
        return 'direct3DInterior';
    }

    protected get depthFunction(): number {
        return this.renderContext.gl.LESS;
    }

    protected get pathIDsAreInstanced(): boolean {
        return true;
    }

    protected get worldTransform() {
        return this.calculateWorldTransform(glmatrix.vec3.create(), TEXT_SCALE);
    }

    protected get usedSizeFactor(): glmatrix.vec2 {
        return glmatrix.vec2.clone([1.0, 1.0]);
    }

    private cubeVertexPositionBuffer: WebGLBuffer;
    private cubeIndexBuffer: WebGLBuffer;

    constructor(renderContext: ThreeDView) {
        super(renderContext);

        this.camera = new PerspectiveCamera(renderContext.canvas, {
            innerCollisionExtent: MONUMENT_SCALE[0],
        });
        this.camera.onChange = () => renderContext.setDirty();

        this.cubeVertexPositionBuffer = unwrapNull(renderContext.gl.createBuffer());
        renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER, this.cubeVertexPositionBuffer);
        renderContext.gl.bufferData(renderContext.gl.ARRAY_BUFFER,
                                    CUBE_VERTEX_POSITIONS,
                                    renderContext.gl.STATIC_DRAW);

        this.cubeIndexBuffer = unwrapNull(renderContext.gl.createBuffer());
        renderContext.gl.bindBuffer(renderContext.gl.ELEMENT_ARRAY_BUFFER, this.cubeIndexBuffer);
        renderContext.gl.bufferData(renderContext.gl.ELEMENT_ARRAY_BUFFER,
                                    CUBE_INDICES,
                                    renderContext.gl.STATIC_DRAW);
    }

    attachMeshes(expandedMeshes: PathfinderMeshData[]) {
        super.attachMeshes(expandedMeshes);

        this.uploadPathColors(expandedMeshes.length);
        this.uploadPathTransforms(expandedMeshes.length);
    }

    pathCountForObject(objectIndex: number): number {
        return this.renderContext.appController.meshDescriptors[objectIndex].positions.length;
    }

    pathBoundingRects(objectIndex: number): Float32Array {
        panic("ThreeDRenderer.pathBoundingRects(): TODO");
        return glmatrix.vec4.create();
    }

    setHintsUniform(uniforms: UniformMap): void {
        this.renderContext.gl.uniform4f(uniforms.uHints, 0, 0, 0, 0);
    }

    protected drawSceneryIfNecessary(): void {
        // Set up the cube VBO.
        const shaderProgram = this.renderContext.shaderPrograms.demo3DMonument;
        this.renderContext.gl.useProgram(shaderProgram.program);
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ARRAY_BUFFER,
                                         this.cubeVertexPositionBuffer);
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ELEMENT_ARRAY_BUFFER,
                                         this.cubeIndexBuffer);
        this.renderContext.gl.vertexAttribPointer(shaderProgram.attributes.aPosition,
                                    3,
                                    this.renderContext.gl.FLOAT,
                                    false,
                                    0,
                                    0);
        this.renderContext.gl.enableVertexAttribArray(shaderProgram.attributes.aPosition);

        // Set uniforms for the monument.
        const transform = this.calculateWorldTransform(MONUMENT_TRANSLATION, MONUMENT_SCALE);
        this.renderContext.gl.uniformMatrix4fv(shaderProgram.uniforms.uTransform,
                                               false,
                                               transform);
        this.renderContext.gl.uniform4f(shaderProgram.uniforms.uColor,
                                        MONUMENT_COLOR[0],
                                        MONUMENT_COLOR[1],
                                        MONUMENT_COLOR[2],
                                        1.0);

        // Set state for the monument.
        this.renderContext.gl.enable(this.renderContext.gl.DEPTH_TEST);
        this.renderContext.gl.depthMask(true);
        this.renderContext.gl.disable(this.renderContext.gl.SCISSOR_TEST);

        // Draw the monument!
        this.renderContext.gl.drawElements(this.renderContext.gl.TRIANGLES,
                                           CUBE_INDICES.length,
                                           this.renderContext.gl.UNSIGNED_SHORT,
                                           0);

        // Clear to avoid Z-fighting.
        this.renderContext.gl.clearDepth(1.0);
        this.renderContext.gl.clear(this.renderContext.gl.DEPTH_BUFFER_BIT);
    }

    protected compositeIfNecessary(): void {}

    protected pathColorsForObject(objectIndex: number): Uint8Array {
        return TEXT_COLOR;
    }

    protected pathTransformsForObject(objectIndex: number): Float32Array {
        const meshDescriptor = this.renderContext.appController.meshDescriptors[objectIndex];
        const pathCount = this.pathCountForObject(objectIndex);
        const pathTransforms = new Float32Array(4 * (pathCount + 1));
        for (let pathIndex = 0; pathIndex < pathCount; pathIndex++) {
            const glyphOrigin = meshDescriptor.positions[pathIndex];
            pathTransforms.set([1, 1, glyphOrigin[0], glyphOrigin[1]], (pathIndex + 1) * 4);
        }
        return pathTransforms;
    }

    protected meshInstanceCountForObject(objectIndex: number): number {
        return this.renderContext.appController.meshDescriptors[objectIndex].positions.length;
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName,
                               aaLevel: number,
                               subpixelAA: SubpixelAAType):
                               AntialiasingStrategy {
        if (aaType !== 'xcaa')
            return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel, subpixelAA);
        throw new PathfinderError("Unsupported antialiasing type!");
    }

    protected clearForDirectRendering(): void {
        this.renderContext.gl.clearColor(1.0, 1.0, 1.0, 1.0);
        this.renderContext.gl.clearDepth(1.0);
        this.renderContext.gl.depthMask(true);
        this.renderContext.gl.clear(this.renderContext.gl.COLOR_BUFFER_BIT |
                                    this.renderContext.gl.DEPTH_BUFFER_BIT);
    }

    protected getModelviewTransform(objectIndex: number): glmatrix.mat4 {
        const textFrameIndex = this.renderContext
                                   .appController
                                   .meshDescriptors[objectIndex]
                                   .textFrameIndex;

        const transform = glmatrix.mat4.create();
        glmatrix.mat4.rotateY(transform, transform, Math.PI / 2.0 * textFrameIndex);
        glmatrix.mat4.translate(transform, transform, TEXT_TRANSLATION);
        return transform;
    }

    // Cheap but effective backface culling.
    protected shouldRenderObject(objectIndex: number): boolean {
        const textFrameIndex = this.renderContext
                                   .appController
                                   .meshDescriptors[objectIndex]
                                   .textFrameIndex;

        const translation = this.camera.translation;
        const extent = TEXT_TRANSLATION[2] * TEXT_SCALE[2];
        switch (textFrameIndex) {
        case 0:     return translation[2] < -extent;
        case 1:     return translation[0] < -extent;
        case 2:     return translation[2] > extent;
        default:    return translation[0] > extent;
        }
    }
    protected newTimingsReceived() {
        const newTimings: Timings = _.pick(this.lastTimings, ['rendering']);
        this.renderContext.appController.newTimingsReceived(newTimings);
    }

    private calculateWorldTransform(modelviewTranslation: glmatrix.vec3,
                                    modelviewScale: glmatrix.vec3):
                                    glmatrix.mat4 {
        const canvas = this.renderContext.canvas;
        const projection = glmatrix.mat4.create();
        glmatrix.mat4.perspective(projection,
                                  FOV / 180.0 * Math.PI,
                                  canvas.width / canvas.height,
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
}

function main() {
    const controller = new ThreeDController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
