// pathfinder/client/src/renderer.ts
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

import {AntialiasingStrategy, AntialiasingStrategyName, GammaCorrectionMode} from './aa-strategy';
import {NoAAStrategy, StemDarkeningMode, SubpixelAAType} from './aa-strategy';
import {AAOptions} from './app-controller';
import PathfinderBufferTexture from "./buffer-texture";
import {UniformMap} from './gl-utils';
import {PathfinderMeshBuffers, PathfinderMeshData} from "./meshes";
import {CompositingOperation, RenderTaskType} from './render-task';
import {ShaderMap} from './shader-loader';
import {FLOAT32_SIZE, Range, UINT16_SIZE, UINT32_SIZE, unwrapNull, unwrapUndef} from './utils';
import {RenderContext, Timings} from "./view";
import {ECAAMulticolorStrategy} from './xcaa-strategy';

const MAX_PATHS: number = 65535;

const TIME_INTERVAL_DELAY: number = 32;

const B_LOOP_BLINN_DATA_SIZE: number = 4;
const B_LOOP_BLINN_DATA_TEX_COORD_OFFSET: number = 0;
const B_LOOP_BLINN_DATA_SIGN_OFFSET: number = 2;

export interface PathTransformBuffers<T> {
    st: T;
    ext: T;
}

export abstract class Renderer {
    readonly renderContext: RenderContext;

    readonly pathTransformBufferTextures: Array<PathTransformBuffers<PathfinderBufferTexture>>;

    meshes: PathfinderMeshBuffers[] | null;
    meshData: PathfinderMeshData[] | null;

    get emboldenAmount(): glmatrix.vec2 {
        return glmatrix.vec2.create();
    }

    get bgColor(): glmatrix.vec4 {
        return glmatrix.vec4.clone([1.0, 1.0, 1.0, 1.0]);
    }

    get fgColor(): glmatrix.vec4 | null {
        return null;
    }

    get backgroundColor(): glmatrix.vec4 {
        return glmatrix.vec4.clone([1.0, 1.0, 1.0, 1.0]);
    }

    get usesIntermediateRenderTargets(): boolean {
        return false;
    }

    get meshesAttached(): boolean {
        return this.meshes != null && this.meshData != null;
    }

    abstract get destFramebuffer(): WebGLFramebuffer | null;
    abstract get destAllocatedSize(): glmatrix.vec2;
    abstract get destUsedSize(): glmatrix.vec2;

    protected antialiasingStrategy: AntialiasingStrategy | null;
    protected lastTimings: Timings;
    protected pathColorsBufferTextures: PathfinderBufferTexture[];

    protected gammaCorrectionMode: GammaCorrectionMode;

    protected get pathIDsAreInstanced(): boolean {
        return false;
    }

    protected abstract get objectCount(): number;
    protected abstract get usedSizeFactor(): glmatrix.vec2;
    protected abstract get worldTransform(): glmatrix.mat4;

    private implicitCoverInteriorVAO: WebGLVertexArrayObjectOES | null;
    private implicitCoverCurveVAO: WebGLVertexArrayObjectOES | null;

    private gammaLUTTexture: WebGLTexture | null;

    private instancedPathIDVBO: WebGLBuffer | null;
    private timerQueryPollInterval: number | null;

    constructor(renderContext: RenderContext) {
        this.renderContext = renderContext;

        this.meshData = null;
        this.meshes = null;

        this.lastTimings = { rendering: 0, compositing: 0 };

        this.gammaCorrectionMode = 'on';

        this.pathTransformBufferTextures = [];
        this.pathColorsBufferTextures = [];

        if (this.pathIDsAreInstanced)
            this.initInstancedPathIDVBO();

        this.initGammaLUTTexture();

        this.antialiasingStrategy = new NoAAStrategy(0, 'none');
        this.antialiasingStrategy.init(this);
        this.antialiasingStrategy.setFramebufferSize(this);
    }

    attachMeshes(meshes: PathfinderMeshData[]): void {
        const renderContext = this.renderContext;
        this.meshData = meshes;
        this.meshes = meshes.map(meshes => new PathfinderMeshBuffers(renderContext.gl, meshes));
        unwrapNull(this.antialiasingStrategy).attachMeshes(this);
    }

    abstract pathBoundingRects(objectIndex: number): Float32Array;
    abstract setHintsUniform(uniforms: UniformMap): void;

    redraw(): void {
        const renderContext = this.renderContext;

        if (this.meshes == null)
            return;

        // Start timing rendering.
        if (this.timerQueryPollInterval == null) {
            renderContext.timerQueryExt.beginQueryEXT(renderContext.timerQueryExt.TIME_ELAPSED_EXT,
                                                      renderContext.atlasRenderingTimerQuery);
        }

        this.clearDestFramebuffer();

        const antialiasingStrategy = unwrapNull(this.antialiasingStrategy);
        antialiasingStrategy.prepareForRendering(this);

        // Draw "scenery" (used in the 3D view).
        this.drawSceneryIfNecessary();

        if (antialiasingStrategy.directRenderingMode !== 'none')
            antialiasingStrategy.prepareForDirectRendering(this);

        const objectCount = this.objectCount;
        for (let objectIndex = 0; objectIndex < objectCount; objectIndex++) {
            // Antialias.
            antialiasingStrategy.antialiasObject(this, objectIndex);

            // Prepare for direct rendering.
            antialiasingStrategy.prepareToRenderObject(this, objectIndex);

            // Perform direct rendering (Loop-Blinn).
            if (antialiasingStrategy.directRenderingMode !== 'none') {
                // Clear.
                this.clearForDirectRendering(objectIndex);

                this.directlyRenderObject(objectIndex);
            }

            antialiasingStrategy.resolveAAForObject(this, objectIndex);
        }

        // End the timer, and start a new one.
        if (this.timerQueryPollInterval == null) {
            renderContext.timerQueryExt.endQueryEXT(renderContext.timerQueryExt.TIME_ELAPSED_EXT);
            renderContext.timerQueryExt.beginQueryEXT(renderContext.timerQueryExt.TIME_ELAPSED_EXT,
                                                      renderContext.compositingTimerQuery);
        }

        antialiasingStrategy.resolve(this);

        // Draw the glyphs with the resolved atlas to the default framebuffer.
        this.compositeIfNecessary();

        // Finish timing.
        this.finishTiming();
    }

    setAntialiasingOptions(aaType: AntialiasingStrategyName,
                           aaLevel: number,
                           aaOptions: AAOptions):
                           void {
        this.gammaCorrectionMode = aaOptions.gammaCorrection;

        this.antialiasingStrategy = this.createAAStrategy(aaType,
                                                          aaLevel,
                                                          aaOptions.subpixelAA,
                                                          aaOptions.stemDarkening);

        this.antialiasingStrategy.init(this);
        if (this.meshData != null)
            this.antialiasingStrategy.attachMeshes(this);
        this.antialiasingStrategy.setFramebufferSize(this);

        this.renderContext.setDirty();
    }

    canvasResized(): void {
        if (this.antialiasingStrategy != null)
            this.antialiasingStrategy.init(this);
    }

    setFramebufferSizeUniform(uniforms: UniformMap): void {
        const gl = this.renderContext.gl;
        gl.uniform2i(uniforms.uFramebufferSize,
                     this.destAllocatedSize[0],
                     this.destAllocatedSize[1]);
    }

    setTransformAndTexScaleUniformsForDest(uniforms: UniformMap): void {
        const renderContext = this.renderContext;
        const usedSize = this.usedSizeFactor;

        const transform = glmatrix.mat4.create();
        glmatrix.mat4.fromTranslation(transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform, transform, [2.0 * usedSize[0], 2.0 * usedSize[1], 1.0]);
        renderContext.gl.uniformMatrix4fv(uniforms.uTransform, false, transform);

        renderContext.gl.uniform2f(uniforms.uTexScale, usedSize[0], usedSize[1]);
    }

    setTransformSTAndTexScaleUniformsForDest(uniforms: UniformMap): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const usedSize = this.usedSizeFactor;
        gl.uniform4f(uniforms.uTransformST, 2.0 * usedSize[0], 2.0 * usedSize[1], -1.0, -1.0);
        gl.uniform2f(uniforms.uTexScale, usedSize[0], usedSize[1]);
    }

    setTransformUniform(uniforms: UniformMap, objectIndex: number) {
        const transform = glmatrix.mat4.clone(this.worldTransform);
        glmatrix.mat4.mul(transform, transform, this.getModelviewTransform(objectIndex));
        this.renderContext.gl.uniformMatrix4fv(uniforms.uTransform, false, transform);
    }

    setTransformSTUniform(uniforms: UniformMap, objectIndex: number): void {
        // FIXME(pcwalton): Lossy conversion from a 4x4 matrix to an ST matrix is ugly and fragile.
        // Refactor.
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const transform = glmatrix.mat4.clone(this.worldTransform);
        glmatrix.mat4.mul(transform, transform, this.getModelviewTransform(objectIndex));

        const translation = glmatrix.vec4.clone([transform[12], transform[13], 0.0, 1.0]);

        gl.uniform4f(uniforms.uTransformST,
                     transform[0],
                     transform[5],
                     transform[12],
                     transform[13]);
    }

    uploadPathColors(objectCount: number): void {
        const renderContext = this.renderContext;
        for (let objectIndex = 0; objectIndex < objectCount; objectIndex++) {
            const pathColors = this.pathColorsForObject(objectIndex);

            let pathColorsBufferTexture;
            if (objectIndex >= this.pathColorsBufferTextures.length) {
                pathColorsBufferTexture = new PathfinderBufferTexture(renderContext.gl,
                                                                      'uPathColors');
                this.pathColorsBufferTextures[objectIndex] = pathColorsBufferTexture;
            } else {
                pathColorsBufferTexture = this.pathColorsBufferTextures[objectIndex];
            }

            pathColorsBufferTexture.upload(renderContext.gl, pathColors);
        }
    }

    uploadPathTransforms(objectCount: number): void {
        const renderContext = this.renderContext;
        for (let objectIndex = 0; objectIndex < objectCount; objectIndex++) {
            const pathTransforms = this.pathTransformsForObject(objectIndex);

            let pathTransformBufferTextures;
            if (objectIndex >= this.pathTransformBufferTextures.length) {
                pathTransformBufferTextures = {
                    ext: new PathfinderBufferTexture(renderContext.gl, 'uPathTransformExt'),
                    st: new PathfinderBufferTexture(renderContext.gl, 'uPathTransformST'),
                };
                this.pathTransformBufferTextures[objectIndex] = pathTransformBufferTextures;
            } else {
                pathTransformBufferTextures = this.pathTransformBufferTextures[objectIndex];
            }

            pathTransformBufferTextures.st.upload(renderContext.gl, pathTransforms.st);
            pathTransformBufferTextures.ext.upload(renderContext.gl, pathTransforms.ext);
        }
    }

    setPathColorsUniform(objectIndex: number, uniforms: UniformMap, textureUnit: number): void {
        const gl = this.renderContext.gl;
        const meshIndex = this.meshIndexForObject(objectIndex);
        this.pathColorsBufferTextures[meshIndex].bind(gl, uniforms, textureUnit);
    }

    setEmboldenAmountUniform(objectIndex: number, uniforms: UniformMap): void {
        const gl = this.renderContext.gl;
        const emboldenAmount = this.emboldenAmount;
        gl.uniform2f(uniforms.uEmboldenAmount, emboldenAmount[0], emboldenAmount[1]);
    }

    renderTaskTypeForObject(objectIndex: number): RenderTaskType {
        return 'color';
    }

    compositingOperationForObject(objectIndex: number): CompositingOperation | null {
        return null;
    }

    meshIndexForObject(objectIndex: number): number {
        return objectIndex;
    }

    pathRangeForObject(objectIndex: number): Range {
        if (this.meshes == null)
            return new Range(0, 0);
        const bVertexPathRanges = this.meshes[objectIndex].bVertexPathRanges;
        return new Range(1, bVertexPathRanges.length + 1);
    }

    protected clearColorForObject(objectIndex: number): glmatrix.vec4 | null {
        return null;
    }

    protected bindGammaLUT(bgColor: glmatrix.vec3, textureUnit: number, uniforms: UniformMap):
                           void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        gl.activeTexture(gl.TEXTURE0 + textureUnit);
        gl.bindTexture(gl.TEXTURE_2D, this.gammaLUTTexture);
        gl.uniform1i(uniforms.uGammaLUT, textureUnit);

        gl.uniform3f(uniforms.uBGColor, bgColor[0], bgColor[1], bgColor[2]);
    }

    protected abstract createAAStrategy(aaType: AntialiasingStrategyName,
                                        aaLevel: number,
                                        subpixelAA: SubpixelAAType,
                                        stemDarkening: StemDarkeningMode):
                                        AntialiasingStrategy;
    protected abstract compositeIfNecessary(): void;
    protected abstract pathColorsForObject(objectIndex: number): Uint8Array;
    protected abstract pathTransformsForObject(objectIndex: number):
                                               PathTransformBuffers<Float32Array>;

    protected abstract directCurveProgramName(): keyof ShaderMap<void>;
    protected abstract directInteriorProgramName(): keyof ShaderMap<void>;

    protected drawSceneryIfNecessary(): void {}

    protected clearDestFramebuffer(): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const clearColor = this.backgroundColor;
        gl.bindFramebuffer(gl.FRAMEBUFFER, this.destFramebuffer);
        gl.depthMask(true);
        gl.viewport(0, 0, this.destAllocatedSize[0], this.destAllocatedSize[1]);
        gl.clearColor(clearColor[0], clearColor[1], clearColor[2], clearColor[3]);
        gl.clearDepth(0.0);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    }

    protected clearForDirectRendering(objectIndex: number): void {
        const renderingMode = unwrapNull(this.antialiasingStrategy).directRenderingMode;
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const clearColor = this.clearColorForObject(objectIndex);
        if (clearColor == null)
            return;

        gl.clearColor(clearColor[0], clearColor[1], clearColor[2], clearColor[3]);

        switch (renderingMode) {
        case 'color':
            gl.clearDepth(0.0);
            gl.depthMask(true);
            gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
            break;
        case 'color-depth':
            gl.clear(gl.COLOR_BUFFER_BIT);
            break;
        case 'none':
            // Nothing to do.
            break;
        }
    }

    protected getModelviewTransform(pathIndex: number): glmatrix.mat4 {
        return glmatrix.mat4.create();
    }

    /// If non-instanced, returns instance 0. An empty range skips rendering the object entirely.
    protected instanceRangeForObject(objectIndex: number): Range {
        return new Range(0, 1);
    }

    /// Called whenever new GPU timing statistics are available.
    protected newTimingsReceived(): void {}

    protected createPathTransformBuffers(pathCount: number): PathTransformBuffers<Float32Array> {
        pathCount += 1;
        return {
            ext: new Float32Array((pathCount + (pathCount & 1)) * 2),
            st: new Float32Array(pathCount * 4),
        };
    }

    private directlyRenderObject(objectIndex: number): void {
        if (this.meshes == null || this.meshData == null)
            return;

        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const antialiasingStrategy = unwrapNull(this.antialiasingStrategy);
        const renderingMode = antialiasingStrategy.directRenderingMode;
        const objectCount = this.objectCount;

        const instanceRange = this.instanceRangeForObject(objectIndex);
        if (instanceRange.isEmpty)
            return;

        const pathRange = this.pathRangeForObject(objectIndex);
        const meshIndex = this.meshIndexForObject(objectIndex);

        const meshes = this.meshes[meshIndex];
        const meshData = this.meshData[meshIndex];

        // Set up implicit cover state.
        gl.depthFunc(gl.GREATER);
        gl.depthMask(true);
        gl.enable(gl.DEPTH_TEST);
        gl.disable(gl.BLEND);

        // Set up the implicit cover interior VAO.
        const directInteriorProgramName = this.directInteriorProgramName();
        const directInteriorProgram = renderContext.shaderPrograms[directInteriorProgramName];
        if (this.implicitCoverInteriorVAO == null) {
            this.implicitCoverInteriorVAO = renderContext.vertexArrayObjectExt
                                                         .createVertexArrayOES();
        }
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.implicitCoverInteriorVAO);
        this.initImplicitCoverInteriorVAO(objectIndex, instanceRange);

        // Draw direct interior parts.
        this.setTransformUniform(directInteriorProgram.uniforms, objectIndex);
        this.setFramebufferSizeUniform(directInteriorProgram.uniforms);
        this.setHintsUniform(directInteriorProgram.uniforms);
        this.setPathColorsUniform(objectIndex, directInteriorProgram.uniforms, 0);
        this.setEmboldenAmountUniform(objectIndex, directInteriorProgram.uniforms);
        this.pathTransformBufferTextures[meshIndex].st.bind(gl, directInteriorProgram.uniforms, 1);
        this.pathTransformBufferTextures[meshIndex]
            .ext
            .bind(gl, directInteriorProgram.uniforms, 2);
        if (renderingMode === 'color-depth') {
            const strategy = antialiasingStrategy as ECAAMulticolorStrategy;
            strategy.bindEdgeDepthTexture(gl, directInteriorProgram.uniforms, 3);
        }
        const coverInteriorRange = getMeshIndexRange(meshes.coverInteriorIndexRanges, pathRange);
        if (!this.pathIDsAreInstanced) {
            gl.drawElements(gl.TRIANGLES,
                            coverInteriorRange.length,
                            gl.UNSIGNED_INT,
                            coverInteriorRange.start * UINT32_SIZE);
        } else {
            renderContext.instancedArraysExt
                            .drawElementsInstancedANGLE(gl.TRIANGLES,
                                                        coverInteriorRange.length,
                                                        gl.UNSIGNED_INT,
                                                        0,
                                                        instanceRange.length);
        }

        // Set up direct curve state.
        gl.depthMask(renderingMode === 'color-depth');
        gl.enable(gl.BLEND);
        gl.blendEquation(gl.FUNC_ADD);
        gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE);

        // Set up the direct curve VAO.
        //
        // TODO(pcwalton): Cache these.
        const directCurveProgramName = this.directCurveProgramName();
        const directCurveProgram = renderContext.shaderPrograms[directCurveProgramName];
        if (this.implicitCoverCurveVAO == null)
            this.implicitCoverCurveVAO = renderContext.vertexArrayObjectExt.createVertexArrayOES();
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.implicitCoverCurveVAO);
        this.initImplicitCoverCurveVAO(objectIndex, instanceRange);

        // Draw direct curve parts.
        this.setTransformUniform(directCurveProgram.uniforms, objectIndex);
        this.setFramebufferSizeUniform(directCurveProgram.uniforms);
        this.setHintsUniform(directCurveProgram.uniforms);
        this.setPathColorsUniform(objectIndex, directCurveProgram.uniforms, 0);
        this.setEmboldenAmountUniform(objectIndex, directCurveProgram.uniforms);
        this.pathTransformBufferTextures[meshIndex].st.bind(gl, directCurveProgram.uniforms, 1);
        this.pathTransformBufferTextures[meshIndex].ext.bind(gl, directCurveProgram.uniforms, 2);
        if (renderingMode === 'color-depth') {
            const strategy = antialiasingStrategy as ECAAMulticolorStrategy;
            strategy.bindEdgeDepthTexture(gl, directCurveProgram.uniforms, 3);
        }
        const coverCurveRange = getMeshIndexRange(meshes.coverCurveIndexRanges, pathRange);
        if (!this.pathIDsAreInstanced) {
            gl.drawElements(gl.TRIANGLES,
                            coverCurveRange.length,
                            gl.UNSIGNED_INT,
                            coverCurveRange.start * UINT32_SIZE);
        } else {
            renderContext.instancedArraysExt.drawElementsInstancedANGLE(gl.TRIANGLES,
                                                                        coverCurveRange.length,
                                                                        gl.UNSIGNED_INT,
                                                                        0,
                                                                        instanceRange.length);
        }

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);

        // Finish direct rendering. Right now, this performs compositing if necessary.
        antialiasingStrategy.finishDirectlyRenderingObject(this, objectIndex);
    }

    private finishTiming() {
        const renderContext = this.renderContext;

        if (this.timerQueryPollInterval != null)
            return;

        renderContext.timerQueryExt.endQueryEXT(renderContext.timerQueryExt.TIME_ELAPSED_EXT);

        this.timerQueryPollInterval = window.setInterval(() => {
            for (const queryName of ['atlasRenderingTimerQuery', 'compositingTimerQuery'] as
                    Array<'atlasRenderingTimerQuery' | 'compositingTimerQuery'>) {
                if (renderContext.timerQueryExt
                                 .getQueryObjectEXT(renderContext[queryName],
                                                    renderContext.timerQueryExt
                                                                 .QUERY_RESULT_AVAILABLE_EXT) ===
                        0) {
                    return;
                }
            }

            const atlasRenderingTime =
                renderContext.timerQueryExt
                             .getQueryObjectEXT(renderContext.atlasRenderingTimerQuery,
                                                renderContext.timerQueryExt.QUERY_RESULT_EXT);
            const compositingTime =
                renderContext.timerQueryExt
                             .getQueryObjectEXT(renderContext.compositingTimerQuery,
                                                renderContext.timerQueryExt.QUERY_RESULT_EXT);
            this.lastTimings = {
                compositing: compositingTime / 1000000.0,
                rendering: atlasRenderingTime / 1000000.0,
            };

            this.newTimingsReceived();

            window.clearInterval(this.timerQueryPollInterval!);
            this.timerQueryPollInterval = null;
        }, TIME_INTERVAL_DELAY);
    }

    private initGammaLUTTexture(): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const gammaLUT = renderContext.gammaLUT;
        const texture = unwrapNull(gl.createTexture());
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.LUMINANCE, gl.LUMINANCE, gl.UNSIGNED_BYTE, gammaLUT);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

        this.gammaLUTTexture = texture;
    }

    private initImplicitCoverCurveVAO(objectIndex: number, instanceRange: Range): void {
        if (this.meshes == null)
            return;

        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const meshIndex = this.meshIndexForObject(objectIndex);
        const meshes = this.meshes[meshIndex];

        const directCurveProgramName = this.directCurveProgramName();
        const directCurveProgram = renderContext.shaderPrograms[directCurveProgramName];
        gl.useProgram(directCurveProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, meshes.bVertexPositions);
        gl.vertexAttribPointer(directCurveProgram.attributes.aPosition, 2, gl.FLOAT, false, 0, 0);

        if (this.pathIDsAreInstanced)
            gl.bindBuffer(gl.ARRAY_BUFFER, this.instancedPathIDVBO);
        else
            gl.bindBuffer(gl.ARRAY_BUFFER, meshes.bVertexPathIDs);
        gl.vertexAttribPointer(directCurveProgram.attributes.aPathID,
                               1,
                               gl.UNSIGNED_SHORT,
                               false,
                               0,
                               instanceRange.start * UINT16_SIZE);
        if (this.pathIDsAreInstanced) {
            renderContext.instancedArraysExt
                         .vertexAttribDivisorANGLE(directCurveProgram.attributes.aPathID, 1);
        }

        gl.bindBuffer(gl.ARRAY_BUFFER, meshes.bVertexLoopBlinnData);
        gl.vertexAttribPointer(directCurveProgram.attributes.aTexCoord,
                               2,
                               gl.UNSIGNED_BYTE,
                               false,
                               B_LOOP_BLINN_DATA_SIZE,
                               B_LOOP_BLINN_DATA_TEX_COORD_OFFSET);
        gl.vertexAttribPointer(directCurveProgram.attributes.aSign,
                               1,
                               gl.BYTE,
                               false,
                               B_LOOP_BLINN_DATA_SIZE,
                               B_LOOP_BLINN_DATA_SIGN_OFFSET);
        gl.bindBuffer(gl.ARRAY_BUFFER, meshes.bVertexNormals);
        gl.vertexAttribPointer(directCurveProgram.attributes.aNormalAngle,
                               1,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE,
                               0);
        gl.enableVertexAttribArray(directCurveProgram.attributes.aPosition);
        gl.enableVertexAttribArray(directCurveProgram.attributes.aTexCoord);
        gl.enableVertexAttribArray(directCurveProgram.attributes.aPathID);
        gl.enableVertexAttribArray(directCurveProgram.attributes.aSign);
        gl.enableVertexAttribArray(directCurveProgram.attributes.aNormalAngle);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, meshes.coverCurveIndices);
    }

    private initImplicitCoverInteriorVAO(objectIndex: number, instanceRange: Range): void {
        if (this.meshes == null)
            return;

        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const meshIndex = this.meshIndexForObject(objectIndex);
        const meshes = this.meshes[meshIndex];

        const directInteriorProgramName = this.directInteriorProgramName();
        const directInteriorProgram = renderContext.shaderPrograms[directInteriorProgramName];
        gl.useProgram(directInteriorProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, meshes.bVertexPositions);
        gl.vertexAttribPointer(directInteriorProgram.attributes.aPosition,
                               2,
                               gl.FLOAT,
                               false,
                               0,
                               0);

        if (this.pathIDsAreInstanced)
            gl.bindBuffer(gl.ARRAY_BUFFER, this.instancedPathIDVBO);
        else
            gl.bindBuffer(gl.ARRAY_BUFFER, meshes.bVertexPathIDs);
        gl.vertexAttribPointer(directInteriorProgram.attributes.aPathID,
                               1,
                               gl.UNSIGNED_SHORT,
                               false,
                               0,
                               instanceRange.start * UINT16_SIZE);
        if (this.pathIDsAreInstanced) {
            renderContext.instancedArraysExt
                         .vertexAttribDivisorANGLE(directInteriorProgram.attributes.aPathID, 1);
        }

        gl.bindBuffer(gl.ARRAY_BUFFER, meshes.bVertexNormals);
        gl.vertexAttribPointer(directInteriorProgram.attributes.aNormalAngle,
                               1,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE,
                               0);

        gl.enableVertexAttribArray(directInteriorProgram.attributes.aPosition);
        gl.enableVertexAttribArray(directInteriorProgram.attributes.aPathID);
        gl.enableVertexAttribArray(directInteriorProgram.attributes.aNormalAngle);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, meshes.coverInteriorIndices);
    }

    private initInstancedPathIDVBO(): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const pathIDs = new Uint16Array(MAX_PATHS);
        for (let pathIndex = 0; pathIndex < MAX_PATHS; pathIndex++)
            pathIDs[pathIndex] = pathIndex + 1;

        this.instancedPathIDVBO = renderContext.gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, this.instancedPathIDVBO);
        gl.bufferData(gl.ARRAY_BUFFER, pathIDs, gl.STATIC_DRAW);
        gl.bindBuffer(gl.ARRAY_BUFFER, null);
    }
}

function getMeshIndexRange(indexRanges: Range[], pathRange: Range): Range {
    if (indexRanges.length === 0)
        return new Range(0, 0);

    const lastIndexRange = unwrapUndef(_.last(indexRanges));
    const descending = indexRanges[0].start > lastIndexRange.start;

    pathRange = new Range(pathRange.start - 1, pathRange.end - 1);

    let startIndex;
    if (pathRange.start >= indexRanges.length)
        startIndex = lastIndexRange.end;
    else if (!descending)
        startIndex = indexRanges[pathRange.start].start;
    else
        startIndex = indexRanges[pathRange.start].end;

    let endIndex;
    if (pathRange.end >= indexRanges.length)
        endIndex = lastIndexRange.end;
    else if (!descending)
        endIndex = indexRanges[pathRange.end].start;
    else
        endIndex = indexRanges[pathRange.end - 1].start;

    if (descending) {
        const tmp = startIndex;
        startIndex = endIndex;
        endIndex = tmp;
    }

    return new Range(startIndex, endIndex);
}
