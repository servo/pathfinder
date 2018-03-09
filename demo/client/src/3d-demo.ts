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
import {Atlas, ATLAS_SIZE, AtlasGlyph, GlyphKey} from './atlas';
import PathfinderBufferTexture from "./buffer-texture";
import {CameraView, PerspectiveCamera} from "./camera";
import {UniformMap} from './gl-utils';
import {PathfinderMeshPack, PathfinderPackedMeshes} from "./meshes";
import {PathTransformBuffers, Renderer} from './renderer';
import {ShaderMap, ShaderProgramSource} from "./shader-loader";
import SSAAStrategy from "./ssaa-strategy";
import {BUILTIN_FONT_URI, ExpandedMeshData} from "./text";
import {calculatePixelRectForGlyph, GlyphStore, Hint, PathfinderFont} from "./text";
import {SimpleTextLayout, TextFrame, TextRun, UnitMetrics} from "./text";
import {TextRenderContext, TextRenderer} from './text-renderer';
import {assert, FLOAT32_SIZE, panic, PathfinderError, Range, UINT16_SIZE} from "./utils";
import {unwrapNull} from "./utils";
import {DemoView, Timings} from "./view";

const TEXT_AVAILABLE_WIDTH: number = 150000;
const TEXT_PADDING: number = 2000;

const TEXT_SCALE: glmatrix.vec3 = glmatrix.vec3.fromValues(1.0 / 200.0, 1.0 / 200.0, 1.0 / 200.0);

const TEXT_DATA_URI: string = "/data/mozmonument.json";

const FONT: string = 'open-sans';

const PIXELS_PER_UNIT: number = 1.0;

const FOV: number = 45.0;
export const NEAR_CLIP_PLANE: number = 0.1;
export const FAR_CLIP_PLANE: number = 100000.0;

const ATLAS_FONT_SIZE: number = 48;

const MAX_DISTANCE: number = 200.0;

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

const AMBIENT_COLOR: glmatrix.vec3 = glmatrix.vec3.clone([0.063, 0.063, 0.063]);
const DIFFUSE_COLOR: glmatrix.vec3 = glmatrix.vec3.clone([0.356, 0.264, 0.136]);
const SPECULAR_COLOR: glmatrix.vec3 = glmatrix.vec3.clone([0.490, 0.420, 0.324]);

const MONUMENT_SHININESS: number = 32.0;

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

const MONUMENT_NORMALS: glmatrix.vec4[] = [
    glmatrix.vec4.clone([ 0.0, -1.0,  0.0, 1.0]),
    glmatrix.vec4.clone([ 0.0,  0.0, -1.0, 1.0]),
    glmatrix.vec4.clone([-1.0,  0.0,  0.0, 1.0]),
    glmatrix.vec4.clone([ 1.0,  0.0,  0.0, 1.0]),
    glmatrix.vec4.clone([ 0.0,  0.0,  1.0, 1.0]),
    glmatrix.vec4.clone([ 0.0,  1.0,  0.0, 1.0]),
];

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

function F32ArrayToMat4(array: Float32Array): mat4 {
    const mat = glmatrix.mat4.create();
    glmatrix.mat4.set(mat, array[0], array[1], array[2], array[3],
                           array[4], array[5], array[6], array[7],
                           array[8], array[9], array[10], array[11],
                           array[12], array[13], array[14], array[15]);
    return mat;
}

class ThreeDController extends DemoAppController<ThreeDView> {
    font!: PathfinderFont;
    textFrames!: TextFrame[];
    glyphStore!: GlyphStore;
    meshDescriptors!: MeshDescriptor[];

    atlasGlyphs!: AtlasGlyph[];
    atlas!: Atlas;

    baseMeshes!: PathfinderMeshPack;
    private expandedMeshes!: PathfinderPackedMeshes[];

    private monumentPromise!: Promise<MonumentSide[]>;

    start() {
        super.start();

        this.atlas = new Atlas;

        this.monumentPromise = window.fetch(TEXT_DATA_URI)
                                     .then(response => response.json())
                                     .then(textData => this.parseTextData(textData));

        this.loadInitialFile(this.builtinFileURI);
    }

    protected fileLoaded(fileData: ArrayBuffer, builtinName: string | null): void {
        this.font = new PathfinderFont(fileData, builtinName);
        this.monumentPromise.then(monument => this.layoutMonument(fileData, monument));
    }

    protected createView(areaLUT: HTMLImageElement,
                         gammaLUT: HTMLImageElement,
                         commonShaderSource: string,
                         shaderSources: ShaderMap<ShaderProgramSource>):
                         ThreeDView {
        return new ThreeDView(this, areaLUT, gammaLUT, commonShaderSource, shaderSources);
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

    private layoutMonument(fileData: ArrayBuffer, monument: MonumentSide[]) {
        this.textFrames = [];
        let glyphsNeeded: number[] = [];

        for (const monumentSide of monument) {
            const textRuns = [];
            for (let lineNumber = 0; lineNumber < monumentSide.lines.length; lineNumber++) {
                const line = monumentSide.lines[lineNumber];

                const lineY = -lineNumber * this.font.opentypeFont.lineHeight();
                const lineGlyphs = line.names.map(name => {
                    const glyphs = this.font.opentypeFont.stringToGlyphs(name);
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
                    const textRun = new TextRun(glyphInfo.glyphs, textRunOrigin, this.font);
                    textRun.layout();
                    textRuns.push(textRun);
                    currentX += glyphInfo.width + spacing;
                }
            }

            const textFrame = new TextFrame(textRuns, this.font);
            this.textFrames.push(textFrame);
            glyphsNeeded.push(...textFrame.allGlyphIDs);
        }

        glyphsNeeded.sort((a, b) => a - b);
        glyphsNeeded = _.sortedUniq(glyphsNeeded);

        this.glyphStore = new GlyphStore(this.font, glyphsNeeded);
        this.glyphStore.partition().then(result => {
            // Build the atlas glyphs needed.
            this.atlasGlyphs = [];
            for (const glyphID of glyphsNeeded) {
                const glyphKey = new GlyphKey(glyphID, null);
                const glyphStoreIndex = this.glyphStore.indexOfGlyphWithID(glyphID);
                if (glyphStoreIndex != null)
                    this.atlasGlyphs.push(new AtlasGlyph(glyphStoreIndex, glyphKey));
            }

            const hint = new Hint(this.glyphStore.font, PIXELS_PER_UNIT, false);

            this.baseMeshes = result.meshes;

            this.meshDescriptors = [];

            for (let textFrameIndex = 0;
                 textFrameIndex < this.textFrames.length;
                 textFrameIndex++) {
                const textFrame = this.textFrames[textFrameIndex];
                const textBounds = textFrame.bounds;

                let glyphDescriptors = [];
                for (const run of textFrame.runs) {
                    for (let glyphIndex = 0; glyphIndex < run.glyphIDs.length; glyphIndex++) {
                        glyphDescriptors.push({
                            glyphID: run.glyphIDs[glyphIndex],
                            position: run.calculatePixelOriginForGlyphAt(glyphIndex,
                                                                         PIXELS_PER_UNIT,
                                                                         0.0,
                                                                         hint,
                                                                         textBounds),
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
                return new PathfinderPackedMeshes(this.baseMeshes, [glyphIndex + 1]);
            });

            this.view.then(view => view.attachMeshes(this.expandedMeshes));
        });
    }
}

class ThreeDView extends DemoView implements TextRenderContext {
    cameraView: CameraView;

    get atlas(): Atlas {
        return this.appController.atlas;
    }

    get atlasGlyphs(): AtlasGlyph[] {
        return this.appController.atlasGlyphs;
    }

    set atlasGlyphs(newAtlasGlyphs: AtlasGlyph[]) {
        this.appController.atlasGlyphs = newAtlasGlyphs;
    }

    get glyphStore(): GlyphStore {
        return this.appController.glyphStore;
    }

    get font(): PathfinderFont {
        return this.appController.font;
    }

    get fontSize(): number {
        return ATLAS_FONT_SIZE;
    }

    get useHinting(): boolean {
        return false;
    }

    get atlasPixelsPerUnit(): number {
        return ATLAS_FONT_SIZE / this.font.opentypeFont.unitsPerEm;
    }

    renderer: ThreeDRenderer;

    appController: ThreeDController;

    protected get camera(): PerspectiveCamera {
        return this.renderer.camera;
    }

    constructor(appController: ThreeDController,
                areaLUT: HTMLImageElement,
                gammaLUT: HTMLImageElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(areaLUT, gammaLUT, commonShaderSource, shaderSources);

        this.cameraView = new ThreeDAtlasCameraView;

        this.appController = appController;
        this.renderer = new ThreeDRenderer(this);

        this.resizeToFit(true);
    }

    newTimingsReceived(timings: Timings): void {}
}

class ThreeDRenderer extends Renderer {
    renderContext!: ThreeDView;

    camera: PerspectiveCamera;

    needsStencil: boolean = false;
    rightEye: boolean = false;

    get isMulticolor(): boolean {
        return false;
    }

    get destFramebuffer(): WebGLFramebuffer | null {
        return null;
    }

    get destAllocatedSize(): glmatrix.vec2 {
        let width = this.renderContext.canvas.width;
        if (this.inVR) {
            width = width / 2;
        }
        return glmatrix.vec2.clone([
            width,
            this.renderContext.canvas.height,
        ]);
    }

    get destUsedSize(): glmatrix.vec2 {
        return this.destAllocatedSize;
    }

    get allowSubpixelAA(): boolean {
        return false;
    }

    get backgroundColor(): glmatrix.vec4 {
        return glmatrix.vec4.clone([1.0, 1.0, 1.0, 1.0]);
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

    protected get objectCount(): number {
        return this.meshBuffers == null ? 0 : this.meshBuffers.length;
    }

    private cubeVertexPositionBuffer: WebGLBuffer;
    private cubeIndexBuffer: WebGLBuffer;

    private glyphPositionsBuffer!: WebGLBuffer;
    private glyphPositions: number[];
    private glyphPositionRanges: Range[];
    private glyphTexCoords: glmatrix.vec4[];
    private glyphSizes: glmatrix.vec2[];

    private distantGlyphVAO: WebGLVertexArrayObjectOES | null;

    private vrProjectionMatrix: Float32Array | null;

    constructor(renderContext: ThreeDView) {
        super(renderContext);

        const gl = renderContext.gl;

        this.glyphPositions = [];
        this.glyphPositionRanges = [];
        this.glyphTexCoords = [];
        this.glyphSizes = [];

        this.distantGlyphVAO = null;
        this.vrProjectionMatrix = null;
        this.camera = new PerspectiveCamera(renderContext.canvas, {
            innerCollisionExtent: MONUMENT_SCALE[0],
        });
        this.camera.onChange = () => renderContext.setDirty();

        this.cubeVertexPositionBuffer = unwrapNull(gl.createBuffer());
        gl.bindBuffer(gl.ARRAY_BUFFER, this.cubeVertexPositionBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, CUBE_VERTEX_POSITIONS, gl.STATIC_DRAW);

        this.cubeIndexBuffer = unwrapNull(gl.createBuffer());
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.cubeIndexBuffer);
        gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, CUBE_INDICES, gl.STATIC_DRAW);
    }

    attachMeshes(expandedMeshes: PathfinderPackedMeshes[]) {
        super.attachMeshes(expandedMeshes);

        this.renderAtlasGlyphs(this.renderContext.appController.atlasGlyphs);

        this.uploadPathColors(expandedMeshes.length);
        this.uploadPathTransforms(expandedMeshes.length);

        this.uploadGlyphPositions();
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

    redrawVR(frame: VRFrameData): void {
        this.clearDestFramebuffer(true);
        this.vrProjectionMatrix = frame.leftProjectionMatrix;
        this.rightEye = false;
        this.camera.setView(F32ArrayToMat4(frame.leftViewMatrix), frame.pose);
        this.redraw();
        this.rightEye = true;
        this.vrProjectionMatrix = frame.rightProjectionMatrix;
        this.camera.setView(F32ArrayToMat4(frame.rightViewMatrix), frame.pose);
        this.redraw();
    }

    setDrawViewport() {
        let offset = 0;
        if (this.rightEye) {
            offset = this.destAllocatedSize[0];
        }
        const renderContext = this.renderContext;
        const gl = renderContext.gl;
        gl.viewport(offset, 0, this.destAllocatedSize[0], this.destAllocatedSize[1]);
    }

    pathTransformsForObject(objectIndex: number): PathTransformBuffers<Float32Array> {
        const meshDescriptor = this.renderContext.appController.meshDescriptors[objectIndex];
        const pathCount = this.pathCountForObject(objectIndex);
        const pathTransforms = this.createPathTransformBuffers(pathCount);
        for (let pathIndex = 0; pathIndex < pathCount; pathIndex++) {
            const glyphOrigin = meshDescriptor.positions[pathIndex];
            pathTransforms.st.set([1, 1, glyphOrigin[0], glyphOrigin[1]], (pathIndex + 1) * 4);
        }
        return pathTransforms;
    }

    protected clearColorForObject(objectIndex: number): glmatrix.vec4 | null {
        return null;
    }

    protected drawSceneryIfNecessary(): void {
        const gl = this.renderContext.gl;

        // Set up the depth buffer for drawing the monument.
        gl.clearDepth(1.0);
        gl.clear(gl.DEPTH_BUFFER_BIT);

        this.drawMonument();

        // Clear to avoid Z-fighting.
        gl.clearDepth(1.0);
        gl.clear(gl.DEPTH_BUFFER_BIT);

        this.drawDistantGlyphs();

        // Set up the depth buffer for direct rendering.
        gl.clearDepth(0.0);
        gl.clear(gl.DEPTH_BUFFER_BIT);
    }

    protected compositeIfNecessary(): void {}

    protected pathColorsForObject(objectIndex: number): Uint8Array {
        return TEXT_COLOR;
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

    protected clearDestFramebuffer(force: boolean): void {
        const gl = this.renderContext.gl;

        gl.bindFramebuffer(gl.FRAMEBUFFER, this.destFramebuffer);
        // clear the entire viewport
        gl.viewport(0, 0, this.renderContext.canvas.width, this.renderContext.canvas.height);

        gl.clearColor(1.0, 1.0, 1.0, 1.0);
        gl.clearDepth(1.0);
        gl.depthMask(true);
        if (force || this.vrProjectionMatrix == null) {
            gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
        }
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

    protected instanceRangeForObject(glyphDescriptorIndex: number): Range {
        if (!this.objectIsVisible(glyphDescriptorIndex))
            return new Range(0, 0);

        const totalLength =
            this.renderContext.appController.meshDescriptors[glyphDescriptorIndex].positions.length;

        const cameraTransform = this.calculateCameraTransform(glmatrix.vec3.create(), TEXT_SCALE);
        const worldTransform = this.calculateWorldTransform(glmatrix.vec3.create(), TEXT_SCALE);
        const glyphsTransform = glmatrix.mat4.clone(cameraTransform);
        const renderTransform = glmatrix.mat4.clone(worldTransform);
        const modelviewTransform = this.getModelviewTransform(glyphDescriptorIndex);
        glmatrix.mat4.mul(glyphsTransform, cameraTransform, modelviewTransform);
        glmatrix.mat4.mul(renderTransform, renderTransform, modelviewTransform);

        const nearbyRange = this.findNearbyGlyphPositions(glyphsTransform, glyphDescriptorIndex);
        const glyphPositionRange = this.glyphPositionRanges[glyphDescriptorIndex];
        nearbyRange.start -= glyphPositionRange.start;
        nearbyRange.end -= glyphPositionRange.start;
        return nearbyRange;
    }

    protected newTimingsReceived(): void {
        const newTimings: Partial<Timings> = _.pick(this.lastTimings, ['rendering']);
        this.renderContext.appController.newTimingsReceived(newTimings);
    }

    protected directCurveProgramName(): keyof ShaderMap<void> {
        return 'direct3DCurve';
    }

    protected directInteriorProgramName(): keyof ShaderMap<void> {
        return 'direct3DInterior';
    }

    // Cheap but effective backface culling.
    private objectIsVisible(objectIndex: number): boolean {
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

    private uploadGlyphPositions(): void {
        const gl = this.renderContext.gl;
        const font = this.renderContext.font;
        const meshDescriptors = this.renderContext.appController.meshDescriptors;

        this.glyphPositions = [];
        this.glyphPositionRanges = [];
        for (const meshDescriptor of meshDescriptors) {
            const glyphIndex = this.renderContext.atlasGlyphs.findIndex(atlasGlyph => {
                return atlasGlyph.glyphKey.id === meshDescriptor.glyphID;
            });
            const glyph = this.renderContext.atlasGlyphs[glyphIndex];
            const glyphMetrics = unwrapNull(font.metricsForGlyph(glyph.glyphKey.id));
            const glyphUnitMetrics = new UnitMetrics(glyphMetrics, 0.0, glmatrix.vec2.create());

            const firstPosition = this.glyphPositions.length / 2;

            for (const position of meshDescriptor.positions) {
                this.glyphPositions.push(position[0] + glyphUnitMetrics.left,
                                         position[1] + glyphUnitMetrics.descent);
            }

            const lastPosition = this.glyphPositions.length / 2;
            this.glyphPositionRanges.push(new Range(firstPosition, lastPosition));
        }

        this.glyphPositionsBuffer = unwrapNull(gl.createBuffer());
        gl.bindBuffer(gl.ARRAY_BUFFER, this.glyphPositionsBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(this.glyphPositions), gl.STATIC_DRAW);
    }

    private drawMonument(): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        // Set up the cube VBO.
        const monumentProgram = this.renderContext.shaderPrograms.demo3DMonument;
        gl.useProgram(monumentProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.cubeVertexPositionBuffer);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.cubeIndexBuffer);
        gl.vertexAttribPointer(monumentProgram.attributes.aPosition, 3, gl.FLOAT, false, 0, 0);
        gl.enableVertexAttribArray(monumentProgram.attributes.aPosition);

        // Set uniforms for the monument.
        const projection = this.calculateProjectionTransform();
        const modelview = this.calculateModelviewTransform(MONUMENT_TRANSLATION, MONUMENT_SCALE);
        gl.uniformMatrix4fv(monumentProgram.uniforms.uProjection, false, projection);
        gl.uniformMatrix4fv(monumentProgram.uniforms.uModelview, false, modelview);
        const cameraModelview = this.calculateCameraModelviewTransform();
        const lightPosition = glmatrix.vec4.clone([-1750.0, -700.0, 1750.0, 1.0]);
        glmatrix.vec4.transformMat4(lightPosition, lightPosition, cameraModelview);
        gl.uniform3f(monumentProgram.uniforms.uLightPosition,
                     lightPosition[0] / lightPosition[3],
                     lightPosition[1] / lightPosition[3],
                     lightPosition[2] / lightPosition[3]);
        gl.uniform3fv(monumentProgram.uniforms.uAmbientColor, AMBIENT_COLOR);
        gl.uniform3fv(monumentProgram.uniforms.uDiffuseColor, DIFFUSE_COLOR);
        gl.uniform3fv(monumentProgram.uniforms.uSpecularColor, SPECULAR_COLOR);
        gl.uniform1f(monumentProgram.uniforms.uShininess, MONUMENT_SHININESS);

        // Set state for the monument.
        gl.enable(gl.DEPTH_TEST);
        gl.depthFunc(gl.LESS);
        gl.depthMask(true);
        gl.disable(gl.SCISSOR_TEST);
        gl.disable(gl.BLEND);

        // Loop over each face.
        for (let face = 0; face < 6; face++) {
            // Set the uniforms for this face.
            const normal = glmatrix.vec4.clone(MONUMENT_NORMALS[face]);
            glmatrix.vec4.transformMat4(normal, normal, this.camera.rotationMatrix);
            gl.uniform3f(monumentProgram.uniforms.uNormal,
                         normal[0] / normal[3],
                         normal[1] / normal[3],
                         normal[2] / normal[3]);

            // Draw the face!
            gl.drawElements(gl.TRIANGLES, 6, gl.UNSIGNED_SHORT, face * 6 * UINT16_SIZE);
        }
    }

    private drawDistantGlyphs(): void {
        const appController = this.renderContext.appController;
        const gl = this.renderContext.gl;

        // Prepare the distant glyph VAO.
        if (this.distantGlyphVAO == null)
            this.distantGlyphVAO = this.renderContext.vertexArrayObjectExt.createVertexArrayOES();
        this.renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.distantGlyphVAO);
        const distantGlyphProgram = this.renderContext.shaderPrograms.demo3DDistantGlyph;
        gl.useProgram(distantGlyphProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.renderContext.quadPositionsBuffer);
        gl.vertexAttribPointer(distantGlyphProgram.attributes.aQuadPosition,
                               2,
                               gl.FLOAT,
                               false,
                               0,
                               0);
        gl.enableVertexAttribArray(distantGlyphProgram.attributes.aQuadPosition);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.renderContext.quadElementsBuffer);

        // Set global uniforms.
        gl.uniform4fv(distantGlyphProgram.uniforms.uColor,
                      _.map(TEXT_COLOR, number => number / 0xff));
        const atlasTexture = this.renderContext.atlas.ensureTexture(this.renderContext);
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, atlasTexture);
        gl.uniform1i(distantGlyphProgram.uniforms.uAtlas, 0);

        // Set state.
        gl.disable(gl.DEPTH_TEST);
        gl.disable(gl.SCISSOR_TEST);
        gl.enable(gl.BLEND);
        gl.blendEquation(gl.FUNC_ADD);
        gl.blendFuncSeparate(gl.ONE, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE);

        // Draw textures for distant glyphs.
        const cameraTransform = this.calculateCameraTransform(glmatrix.vec3.create(), TEXT_SCALE);
        const worldTransform = this.calculateWorldTransform(glmatrix.vec3.create(), TEXT_SCALE);

        for (let glyphDescriptorIndex = 0;
             glyphDescriptorIndex < this.glyphPositionRanges.length;
             glyphDescriptorIndex++) {
            if (!this.objectIsVisible(glyphDescriptorIndex))
                continue;

            const meshDescriptor = appController.meshDescriptors[glyphDescriptorIndex];
            const glyphIndex = this.renderContext.atlasGlyphs.findIndex(glyph => {
                return glyph.glyphKey.id === meshDescriptor.glyphID;
            });

            // Calculate transforms.
            const glyphsTransform = glmatrix.mat4.clone(cameraTransform);
            const renderTransform = glmatrix.mat4.clone(worldTransform);
            const modelviewTransform = this.getModelviewTransform(glyphDescriptorIndex);
            glmatrix.mat4.mul(glyphsTransform, cameraTransform, modelviewTransform);
            glmatrix.mat4.mul(renderTransform, renderTransform, modelviewTransform);

            const glyphPositionRange = this.glyphPositionRanges[glyphDescriptorIndex];
            const nearbyGlyphPositionRange = this.findNearbyGlyphPositions(glyphsTransform,
                                                                           glyphDescriptorIndex);

            // Set uniforms.
            gl.uniformMatrix4fv(distantGlyphProgram.uniforms.uTransform, false, renderTransform);

            const glyphTexCoords = this.glyphTexCoords[glyphIndex];
            gl.uniform4f(distantGlyphProgram.uniforms.uGlyphTexCoords,
                         glyphTexCoords[0],
                         glyphTexCoords[1],
                         glyphTexCoords[2],
                         glyphTexCoords[3]);
            const glyphSize = this.glyphSizes[glyphIndex];
            gl.uniform2f(distantGlyphProgram.uniforms.uGlyphSize, glyphSize[0], glyphSize[1]);

            const rangeBefore = new Range(glyphPositionRange.start,
                                          nearbyGlyphPositionRange.start);
            if (!rangeBefore.isEmpty) {
                // Would be nice to have `glDrawElementsInstancedBaseInstance`...
                // FIXME(pcwalton): Cache VAOs?
                gl.bindBuffer(gl.ARRAY_BUFFER, this.glyphPositionsBuffer);
                gl.vertexAttribPointer(distantGlyphProgram.attributes.aPosition,
                                       2,
                                       gl.FLOAT,
                                       false,
                                       0,
                                       rangeBefore.start * FLOAT32_SIZE * 2);
                gl.enableVertexAttribArray(distantGlyphProgram.attributes.aPosition);
                this.renderContext
                    .instancedArraysExt
                    .vertexAttribDivisorANGLE(distantGlyphProgram.attributes.aPosition, 1);

                this.renderContext
                    .instancedArraysExt
                    .drawElementsInstancedANGLE(gl.TRIANGLES,
                                                6,
                                                gl.UNSIGNED_BYTE,
                                                0,
                                                rangeBefore.length);
            }

            const rangeAfter = new Range(nearbyGlyphPositionRange.end, glyphPositionRange.end);
            if (!rangeAfter.isEmpty) {
                gl.bindBuffer(gl.ARRAY_BUFFER, this.glyphPositionsBuffer);
                gl.vertexAttribPointer(distantGlyphProgram.attributes.aPosition,
                                       2,
                                       gl.FLOAT,
                                       false,
                                       0,
                                       rangeAfter.start * FLOAT32_SIZE * 2);
                gl.enableVertexAttribArray(distantGlyphProgram.attributes.aPosition);
                this.renderContext
                    .instancedArraysExt
                    .vertexAttribDivisorANGLE(distantGlyphProgram.attributes.aPosition, 1);

                this.renderContext
                    .instancedArraysExt
                    .drawElementsInstancedANGLE(gl.TRIANGLES,
                                                6,
                                                gl.UNSIGNED_BYTE,
                                                0,
                                                rangeAfter.length);
            }
        }

        this.renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private calculateProjectionTransform(): glmatrix.mat4 {
        if (this.vrProjectionMatrix != null) {
            return F32ArrayToMat4(this.vrProjectionMatrix);
        }
        const canvas = this.renderContext.canvas;
        const projection = glmatrix.mat4.create();
        glmatrix.mat4.perspective(projection,
                                  FOV / 180.0 * Math.PI,
                                  canvas.width / canvas.height,
                                  NEAR_CLIP_PLANE,
                                  FAR_CLIP_PLANE);
        return projection;
    }

    private calculateCameraModelviewTransform(): glmatrix.mat4 {
        const modelview = glmatrix.mat4.create();
        glmatrix.mat4.mul(modelview, modelview, this.camera.rotationMatrix);
        glmatrix.mat4.translate(modelview, modelview, this.camera.translation);
        return modelview;
    }

    private calculateModelviewTransform(modelviewTranslation: glmatrix.vec3,
                                        modelviewScale: glmatrix.vec3):
                                        glmatrix.mat4 {
        const modelview = this.calculateCameraModelviewTransform();
        glmatrix.mat4.translate(modelview, modelview, modelviewTranslation);
        glmatrix.mat4.scale(modelview, modelview, modelviewScale);
        return modelview;
    }

    private calculateWorldTransform(modelviewTranslation: glmatrix.vec3,
                                    modelviewScale: glmatrix.vec3):
                                    glmatrix.mat4 {
        const projection = this.calculateProjectionTransform();
        const modelview = this.calculateModelviewTransform(modelviewTranslation, modelviewScale);

        const transform = glmatrix.mat4.create();
        glmatrix.mat4.mul(transform, projection, modelview);
        return transform;
    }

    private calculateCameraTransform(modelviewTranslation: glmatrix.vec3,
                                     modelviewScale: glmatrix.vec3):
                                     glmatrix.mat4 {
        const transform = glmatrix.mat4.create();
        glmatrix.mat4.translate(transform, transform, this.camera.translation);
        glmatrix.mat4.translate(transform, transform, modelviewTranslation);
        glmatrix.mat4.scale(transform, transform, modelviewScale);
        return transform;
    }

    private renderAtlasGlyphs(atlasGlyphs: AtlasGlyph[]): void {
        const hint = new Hint(this.renderContext.font,
                              this.renderContext.atlasPixelsPerUnit,
                              false);
        this.renderContext.atlas.layoutGlyphs(atlasGlyphs,
                                              this.renderContext.font,
                                              this.renderContext.atlasPixelsPerUnit,
                                              0.0,
                                              hint,
                                              glmatrix.vec2.create());

        const atlasRenderer = new ThreeDAtlasRenderer(this.renderContext, atlasGlyphs);
        const baseMeshes = this.renderContext.appController.baseMeshes;
        const expandedMeshes = new PathfinderPackedMeshes(baseMeshes);
        atlasRenderer.attachMeshes([expandedMeshes]);
        atlasRenderer.renderAtlas();
        this.glyphTexCoords = atlasRenderer.glyphTexCoords;
        this.glyphSizes = atlasRenderer.glyphSizes;
    }

    private findNearbyGlyphPositions(transform: glmatrix.mat4, glyphDescriptorIndex: number):
                                     Range {
        const glyphPositionRange = this.glyphPositionRanges[glyphDescriptorIndex];
        const startPosition = this.findFirstGlyphPositionInRange(transform,
                                                                 glyphPositionRange,
                                                                 -MAX_DISTANCE);
        const endPosition = this.findFirstGlyphPositionInRange(transform,
                                                               new Range(startPosition,
                                                                         glyphPositionRange.end),
                                                               MAX_DISTANCE);
        return new Range(startPosition, endPosition);
    }

    private findFirstGlyphPositionInRange(transform: glmatrix.mat4,
                                          range: Range,
                                          maxDistance: number):
                                          number {
        let lo = range.start, hi = range.end;
        while (lo < hi) {
            const mid = lo + ((hi - lo) >> 1);
            const glyphPosition = this.calculateTransformedGlyphPosition(transform, mid);
            const glyphDistance = -glyphPosition[1];
            if (glyphDistance < maxDistance)
                lo = mid + 1;
            else
                hi = mid;
        }
        return lo;
    }

    private calculateTransformedGlyphPosition(transform: glmatrix.mat4,
                                              glyphPositionIndex: number):
                                              glmatrix.vec4 {
        const position = glmatrix.vec4.clone([
            this.glyphPositions[glyphPositionIndex * 2 + 0],
            this.glyphPositions[glyphPositionIndex * 2 + 1],
            0.0,
            1.0,
        ]);
        glmatrix.vec4.transformMat4(position, position, transform);
        return position;
    }
}

class ThreeDAtlasRenderer extends TextRenderer {
    glyphTexCoords: glmatrix.vec4[];
    glyphSizes: glmatrix.vec2[];

    private allAtlasGlyphs: AtlasGlyph[];

    get backgroundColor(): glmatrix.vec4 {
        return glmatrix.vec4.create();
    }

    constructor(renderContext: ThreeDView, atlasGlyphs: AtlasGlyph[]) {
        super(renderContext);
        this.allAtlasGlyphs = atlasGlyphs;
        this.glyphTexCoords = [];
        this.glyphSizes = [];
    }

    renderAtlas(): void {
        this.createAtlasFramebuffer();
        this.buildAtlasGlyphs(this.allAtlasGlyphs);
        this.redraw();
        this.calculateGlyphTexCoords();
    }

    protected compositeIfNecessary(): void {}

    private calculateGlyphTexCoords(): void {
        const pixelsPerUnit = this.pixelsPerUnit;
        const glyphCount = this.renderContext.atlasGlyphs.length;
        const font = this.renderContext.font;
        const hint = this.createHint();

        this.glyphTexCoords = [];
        this.glyphSizes = [];

        for (let glyphIndex = 0; glyphIndex < glyphCount; glyphIndex++) {
            const glyph = this.renderContext.atlasGlyphs[glyphIndex];
            const glyphPixelOrigin = glyph.calculateSubpixelOrigin(pixelsPerUnit);
            const glyphMetrics = font.metricsForGlyph(glyph.glyphKey.id);
            if (glyphMetrics == null)
                continue;

            const glyphUnitMetrics = new UnitMetrics(glyphMetrics, 0.0, glmatrix.vec2.create());
            const atlasGlyphRect = calculatePixelRectForGlyph(glyphUnitMetrics,
                                                              glyphPixelOrigin,
                                                              pixelsPerUnit,
                                                              hint);

            this.glyphSizes.push(glmatrix.vec2.clone([
                glyphUnitMetrics.right - glyphUnitMetrics.left,
                glyphUnitMetrics.ascent - glyphUnitMetrics.descent,
            ]));

            const atlasGlyphBL = atlasGlyphRect.slice(0, 2) as glmatrix.vec2;
            const atlasGlyphTR = atlasGlyphRect.slice(2, 4) as glmatrix.vec2;
            glmatrix.vec2.div(atlasGlyphBL, atlasGlyphBL, ATLAS_SIZE);
            glmatrix.vec2.div(atlasGlyphTR, atlasGlyphTR, ATLAS_SIZE);

            this.glyphTexCoords.push(glmatrix.vec4.clone([
                atlasGlyphBL[0],
                atlasGlyphBL[1],
                atlasGlyphTR[0],
                atlasGlyphTR[1],
            ]));
        }
    }
}

class ThreeDAtlasCameraView implements CameraView {
    get width(): number {
        return ATLAS_SIZE[0];
    }

    get height(): number {
        return ATLAS_SIZE[1];
    }

    get classList(): DOMTokenList | null {
        return null;
    }

    addEventListener<K extends keyof HTMLElementEventMap>(type: K,
                                                          listener: (this: HTMLCanvasElement,
                                                                     ev: HTMLElementEventMap[K]) =>
                                                                     any,
                                                          useCapture?: boolean): void {}

    getBoundingClientRect(): ClientRect {
        return new ClientRect();
    }
}

function main() {
    const controller = new ThreeDController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
