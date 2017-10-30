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

import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from './aa-strategy';
import {StemDarkeningMode, SubpixelAAType} from './aa-strategy';
import PathfinderBufferTexture from "./buffer-texture";
import {UniformMap} from './gl-utils';
import {PathfinderMeshBuffers, PathfinderMeshData} from "./meshes";
import {ShaderMap} from './shader-loader';
import {FLOAT32_SIZE, Range, UINT16_SIZE, UINT32_SIZE, unwrapNull} from './utils';
import {RenderContext, Timings} from "./view";
import {MCAAMulticolorStrategy} from './xcaa-strategy';

const MAX_PATHS: number = 65535;

const TIME_INTERVAL_DELAY: number = 32;

const B_LOOP_BLINN_DATA_SIZE: number = 4;
const B_LOOP_BLINN_DATA_TEX_COORD_OFFSET: number = 0;
const B_LOOP_BLINN_DATA_SIGN_OFFSET: number = 2;

export abstract class Renderer {
    readonly renderContext: RenderContext;

    readonly pathTransformBufferTextures: PathfinderBufferTexture[];

    meshes: PathfinderMeshBuffers[];
    meshData: PathfinderMeshData[];

    get emboldenAmount(): glmatrix.vec2 {
        return glmatrix.vec2.create();
    }

    get bgColor(): glmatrix.vec4 | null {
        return null;
    }

    get fgColor(): glmatrix.vec4 | null {
        return null;
    }

    abstract get destFramebuffer(): WebGLFramebuffer | null;
    abstract get destAllocatedSize(): glmatrix.vec2;
    abstract get destUsedSize(): glmatrix.vec2;

    protected antialiasingStrategy: AntialiasingStrategy | null;
    protected lastTimings: Timings;
    protected pathColorsBufferTextures: PathfinderBufferTexture[];

    protected get pathIDsAreInstanced(): boolean {
        return false;
    }

    protected get backgroundColor(): glmatrix.vec4 {
        return glmatrix.vec4.create();
    }

    protected abstract get depthFunction(): GLenum;
    protected abstract get usedSizeFactor(): glmatrix.vec2;
    protected abstract get worldTransform(): glmatrix.mat4;

    private implicitCoverInteriorVAO: WebGLVertexArrayObjectOES | null;
    private implicitCoverCurveVAO: WebGLVertexArrayObjectOES | null;

    private instancedPathIDVBO: WebGLBuffer | null;
    private timerQueryPollInterval: number | null;

    constructor(renderContext: RenderContext) {
        this.renderContext = renderContext;

        this.lastTimings = { rendering: 0, compositing: 0 };

        this.pathTransformBufferTextures = [];
        this.pathColorsBufferTextures = [];

        if (this.pathIDsAreInstanced)
            this.initInstancedPathIDVBO();

        this.antialiasingStrategy = new NoAAStrategy(0, 'none');
        this.antialiasingStrategy.init(this);
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

        // Draw "scenery" (used in the 3D view).
        this.drawSceneryIfNecessary();

        // Antialias.
        const antialiasingStrategy = unwrapNull(this.antialiasingStrategy);
        antialiasingStrategy.antialias(this);

        // Prepare for direct rendering.
        antialiasingStrategy.prepareForDirectRendering(this);

        // Clear.
        this.clearForDirectRendering();

        // Perform direct rendering (Loop-Blinn).
        if (antialiasingStrategy.directRenderingMode !== 'none')
            this.renderDirect(0);

        // End the timer, and start a new one.
        if (this.timerQueryPollInterval == null) {
            renderContext.timerQueryExt.endQueryEXT(renderContext.timerQueryExt.TIME_ELAPSED_EXT);
            renderContext.timerQueryExt.beginQueryEXT(renderContext.timerQueryExt.TIME_ELAPSED_EXT,
                                                      renderContext.compositingTimerQuery);
        }

        antialiasingStrategy.resolve(this);

        if (antialiasingStrategy.directRenderingMode === 'two-pass')
            this.renderDirect(1);

        // Draw the glyphs with the resolved atlas to the default framebuffer.
        this.compositeIfNecessary();

        // Finish timing.
        this.finishTiming();
    }

    setAntialiasingOptions(aaType: AntialiasingStrategyName,
                           aaLevel: number,
                           subpixelAA: SubpixelAAType,
                           stemDarkening: StemDarkeningMode) {
        this.antialiasingStrategy = this.createAAStrategy(aaType,
                                                          aaLevel,
                                                          subpixelAA,
                                                          stemDarkening);

        this.antialiasingStrategy.init(this);
        if (this.meshData != null)
            this.antialiasingStrategy.attachMeshes(this);

        this.renderContext.setDirty();
    }

    canvasResized() {
        if (this.antialiasingStrategy != null)
            this.antialiasingStrategy.init(this);
    }

    setFramebufferSizeUniform(uniforms: UniformMap) {
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

    setTransformSTUniform(uniforms: UniformMap, objectIndex: number) {
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

    uploadPathColors(objectCount: number) {
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

    uploadPathTransforms(objectCount: number) {
        const renderContext = this.renderContext;
        for (let objectIndex = 0; objectIndex < objectCount; objectIndex++) {
            const pathTransforms = this.pathTransformsForObject(objectIndex);

            let pathTransformBufferTexture;
            if (objectIndex >= this.pathTransformBufferTextures.length) {
                pathTransformBufferTexture = new PathfinderBufferTexture(renderContext.gl,
                                                                         'uPathTransform');
                this.pathTransformBufferTextures[objectIndex] = pathTransformBufferTexture;
            } else {
                pathTransformBufferTexture = this.pathTransformBufferTextures[objectIndex];
            }

            pathTransformBufferTexture.upload(renderContext.gl, pathTransforms);
        }
    }

    setPathColorsUniform(objectIndex: number, uniforms: UniformMap, textureUnit: number): void {
        const gl = this.renderContext.gl;
        this.pathColorsBufferTextures[objectIndex].bind(gl, uniforms, textureUnit);
    }

    protected abstract createAAStrategy(aaType: AntialiasingStrategyName,
                                        aaLevel: number,
                                        subpixelAA: SubpixelAAType,
                                        stemDarkening: StemDarkeningMode):
                                        AntialiasingStrategy;
    protected abstract compositeIfNecessary(): void;
    protected abstract pathColorsForObject(objectIndex: number): Uint8Array;
    protected abstract pathTransformsForObject(objectIndex: number): Float32Array;

    protected abstract directCurveProgramNameForPass(pass: number): keyof ShaderMap<void>;
    protected abstract directInteriorProgramNameForPass(pass: number): keyof ShaderMap<void>;

    protected drawSceneryIfNecessary(): void {}

    protected clearForDirectRendering(): void {
        const renderingMode = unwrapNull(this.antialiasingStrategy).directRenderingMode;
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const clearColor = this.backgroundColor;
        gl.clearColor(clearColor[0], clearColor[1], clearColor[2], clearColor[3]);

        switch (renderingMode) {
        case 'color':
            gl.clearDepth(0.0);
            gl.depthMask(true);
            gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
            break;
        case 'two-pass':
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

    private renderDirect(pass: number): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const antialiasingStrategy = unwrapNull(this.antialiasingStrategy);
        const renderingMode = antialiasingStrategy.directRenderingMode;

        for (let objectIndex = 0; objectIndex < this.meshes.length; objectIndex++) {
            const instanceRange = this.instanceRangeForObject(objectIndex);
            if (instanceRange.isEmpty)
                continue;

            const meshData = this.meshData[objectIndex];

            // Set up implicit cover state.
            gl.depthFunc(this.depthFunction);
            gl.depthMask(true);
            gl.enable(gl.DEPTH_TEST);
            gl.disable(gl.BLEND);

            // Set up the implicit cover interior VAO.
            const directInteriorProgramName = this.directInteriorProgramNameForPass(pass);
            const directInteriorProgram = renderContext.shaderPrograms[directInteriorProgramName];
            if (this.implicitCoverInteriorVAO == null) {
                this.implicitCoverInteriorVAO = renderContext.vertexArrayObjectExt
                                                             .createVertexArrayOES();
            }
            renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.implicitCoverInteriorVAO);
            this.initImplicitCoverInteriorVAO(pass, objectIndex, instanceRange);

            // Draw direct interior parts.
            this.setTransformUniform(directInteriorProgram.uniforms, objectIndex);
            this.setFramebufferSizeUniform(directInteriorProgram.uniforms);
            this.setHintsUniform(directInteriorProgram.uniforms);
            this.setPathColorsUniform(objectIndex, directInteriorProgram.uniforms, 0);
            this.pathTransformBufferTextures[objectIndex]
                .bind(gl, directInteriorProgram.uniforms, 1);
            if (renderingMode === 'two-pass') {
                const strategy = antialiasingStrategy as MCAAMulticolorStrategy;
                strategy.bindEdgeDepthTexture(gl, directInteriorProgram.uniforms, 2);
            }
            let indexCount = meshData.coverInteriorIndices.byteLength / UINT32_SIZE;
            if (!this.pathIDsAreInstanced) {
                gl.drawElements(gl.TRIANGLES, indexCount, gl.UNSIGNED_INT, 0);
            } else {
                renderContext.instancedArraysExt.drawElementsInstancedANGLE(gl.TRIANGLES,
                                                                            indexCount,
                                                                            gl.UNSIGNED_INT,
                                                                            0,
                                                                            instanceRange.length);
            }

            // Set up direct curve state.
            gl.depthMask(false);
            gl.enable(gl.BLEND);
            gl.blendEquation(gl.FUNC_ADD);
            gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE);

            // Set up the direct curve VAO.
            //
            // TODO(pcwalton): Cache these.
            const directCurveProgramName = this.directCurveProgramNameForPass(pass);
            const directCurveProgram = renderContext.shaderPrograms[directCurveProgramName];
            if (this.implicitCoverCurveVAO == null) {
                this.implicitCoverCurveVAO = renderContext.vertexArrayObjectExt
                                                          .createVertexArrayOES();
            }
            renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.implicitCoverCurveVAO);
            this.initImplicitCoverCurveVAO(pass, objectIndex, instanceRange);

            // Draw direct curve parts.
            this.setTransformUniform(directCurveProgram.uniforms, objectIndex);
            this.setFramebufferSizeUniform(directCurveProgram.uniforms);
            this.setHintsUniform(directCurveProgram.uniforms);
            this.setPathColorsUniform(objectIndex, directCurveProgram.uniforms, 0);
            this.pathTransformBufferTextures[objectIndex].bind(gl, directCurveProgram.uniforms, 1);
            if (renderingMode === 'two-pass') {
                const strategy = antialiasingStrategy as MCAAMulticolorStrategy;
                strategy.bindEdgeDepthTexture(gl, directCurveProgram.uniforms, 2);
            }
            indexCount = meshData.coverCurveIndices.byteLength / UINT32_SIZE;
            if (!this.pathIDsAreInstanced) {
                gl.drawElements(gl.TRIANGLES, indexCount, gl.UNSIGNED_INT, 0);
            } else {
                renderContext.instancedArraysExt.drawElementsInstancedANGLE(gl.TRIANGLES,
                                                                            indexCount,
                                                                            gl.UNSIGNED_INT,
                                                                            0,
                                                                            instanceRange.length);
            }

            renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
        }
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

    private initImplicitCoverCurveVAO(pass: number, objectIndex: number, instanceRange: Range):
                                      void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;
        const meshes = this.meshes[objectIndex];

        const directCurveProgramName = this.directCurveProgramNameForPass(pass);
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
        gl.enableVertexAttribArray(directCurveProgram.attributes.aPosition);
        gl.enableVertexAttribArray(directCurveProgram.attributes.aTexCoord);
        gl.enableVertexAttribArray(directCurveProgram.attributes.aPathID);
        gl.enableVertexAttribArray(directCurveProgram.attributes.aSign);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, meshes.coverCurveIndices);
    }

    private initImplicitCoverInteriorVAO(pass: number,
                                         objectIndex: number,
                                         instanceRange: Range):
                                         void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;
        const meshes = this.meshes[objectIndex];

        const directInteriorProgramName = this.directInteriorProgramNameForPass(pass);
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

        gl.enableVertexAttribArray(directInteriorProgram.attributes.aPosition);
        gl.enableVertexAttribArray(directInteriorProgram.attributes.aPathID);
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

    private setTransformUniform(uniforms: UniformMap, objectIndex: number) {
        const transform = glmatrix.mat4.clone(this.worldTransform);
        glmatrix.mat4.mul(transform, transform, this.getModelviewTransform(objectIndex));
        this.renderContext.gl.uniformMatrix4fv(uniforms.uTransform, false, transform);
    }
}
