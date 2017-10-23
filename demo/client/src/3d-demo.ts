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
import {PathfinderMeshData} from "./meshes";
import {Renderer} from './renderer';
import {ShaderMap, ShaderProgramSource} from "./shader-loader";
import SSAAStrategy from "./ssaa-strategy";
import {BUILTIN_FONT_URI, ExpandedMeshData} from "./text";
import {calculatePixelRectForGlyph, GlyphStore, Hint, PathfinderFont} from "./text";
import {SimpleTextLayout, TextFrame, TextRun, UnitMetrics} from "./text";
import {TextRenderContext, TextRenderer} from './text-renderer';
import {assert, FLOAT32_SIZE, panic, PathfinderError, Range, unwrapNull} from "./utils";
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
    font: PathfinderFont;
    textFrames: TextFrame[];
    glyphStore: GlyphStore;
    meshDescriptors: MeshDescriptor[];

    atlasGlyphs: AtlasGlyph[];
    atlas: Atlas;

    baseMeshes: PathfinderMeshData;
    private expandedMeshes: PathfinderMeshData[];

    private monumentPromise: Promise<MonumentSide[]>;

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
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(commonShaderSource, shaderSources);

        this.cameraView = new ThreeDAtlasCameraView;

        this.appController = appController;
        this.renderer = new ThreeDRenderer(this);

        this.resizeToFit(true);
    }

    newTimingsReceived(timings: Timings): void {}
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

    protected get depthFunction(): GLenum {
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

    private glyphPositionsBuffer: WebGLBuffer;
    private glyphPositions: number[];
    private glyphPositionRanges: Range[];
    private glyphTexCoords: glmatrix.vec4[];
    private glyphSizes: glmatrix.vec2[];

    private distantGlyphVAO: WebGLVertexArrayObjectOES | null;

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

    protected drawSceneryIfNecessary(): void {
        const gl = this.renderContext.gl;

        this.drawMonument();

        // Clear to avoid Z-fighting.
        gl.clearDepth(1.0);
        gl.clear(gl.DEPTH_BUFFER_BIT);

        this.drawDistantGlyphs();
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
        const gl = this.renderContext.gl;
        gl.clearColor(1.0, 1.0, 1.0, 1.0);
        gl.clearDepth(1.0);
        gl.depthMask(true);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
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
            const glyphUnitMetrics = new UnitMetrics(glyphMetrics, glmatrix.vec2.create());

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
        const gl = this.renderContext.gl;

        // Set up the cube VBO.
        const monumentProgram = this.renderContext.shaderPrograms.demo3DMonument;
        this.renderContext.gl.useProgram(monumentProgram.program);
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ARRAY_BUFFER,
                                         this.cubeVertexPositionBuffer);
        this.renderContext.gl.bindBuffer(this.renderContext.gl.ELEMENT_ARRAY_BUFFER,
                                         this.cubeIndexBuffer);
        this.renderContext.gl.vertexAttribPointer(monumentProgram.attributes.aPosition,
                                                  3,
                                                  this.renderContext.gl.FLOAT,
                                                  false,
                                                  0,
                                                  0);
        this.renderContext.gl.enableVertexAttribArray(monumentProgram.attributes.aPosition);

        // Set uniforms for the monument.
        const transform = this.calculateWorldTransform(MONUMENT_TRANSLATION, MONUMENT_SCALE);
        gl.uniformMatrix4fv(monumentProgram.uniforms.uTransform, false, transform);
        gl.uniform4f(monumentProgram.uniforms.uColor,
                     MONUMENT_COLOR[0],
                     MONUMENT_COLOR[1],
                     MONUMENT_COLOR[2],
                     1.0);

        // Set state for the monument.
        gl.enable(gl.DEPTH_TEST);
        gl.depthFunc(this.depthFunction);
        gl.depthMask(true);
        gl.disable(gl.SCISSOR_TEST);
        gl.disable(gl.BLEND);

        // Draw the monument!
        gl.drawElements(gl.TRIANGLES, CUBE_INDICES.length, gl.UNSIGNED_SHORT, 0);
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
        gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE);

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
                                              hint,
                                              glmatrix.vec2.create());

        const atlasRenderer = new ThreeDAtlasRenderer(this.renderContext, atlasGlyphs);
        atlasRenderer.attachMeshes([this.renderContext.appController.baseMeshes]);
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

    constructor(renderContext: ThreeDView, atlasGlyphs: AtlasGlyph[]) {
        super(renderContext);
        this.allAtlasGlyphs = atlasGlyphs;
    }

    renderAtlas(): void {
        this.createAtlasFramebuffer();
        this.buildAtlasGlyphs(this.allAtlasGlyphs);
        this.redraw();
        this.calculateGlyphTexCoords();
    }

    protected compositeIfNecessary(): void {}

    private calculateGlyphTexCoords(): void {
        const displayPixelsPerUnit = this.displayPixelsPerUnit;
        const glyphCount = this.renderContext.atlasGlyphs.length;
        const font = this.renderContext.font;
        const hint = this.createHint();

        this.glyphTexCoords = [];
        this.glyphSizes = [];

        for (let glyphIndex = 0; glyphIndex < glyphCount; glyphIndex++) {
            const glyph = this.renderContext.atlasGlyphs[glyphIndex];
            const glyphPixelOrigin = glyph.calculateSubpixelOrigin(displayPixelsPerUnit);
            const glyphMetrics = font.metricsForGlyph(glyph.glyphKey.id);
            if (glyphMetrics == null)
                continue;

            const glyphUnitMetrics = new UnitMetrics(glyphMetrics, glmatrix.vec2.create());
            const atlasGlyphRect = calculatePixelRectForGlyph(glyphUnitMetrics,
                                                              glyphPixelOrigin,
                                                              displayPixelsPerUnit,
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
