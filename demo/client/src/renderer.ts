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

import {AntialiasingStrategy, AntialiasingStrategyName, DirectRenderingMode} from './aa-strategy';
import {GammaCorrectionMode} from './aa-strategy';
import {TileInfo} from './aa-strategy';
import {NoAAStrategy, StemDarkeningMode, SubpixelAAType} from './aa-strategy';
import {AAOptions} from './app-controller';
import PathfinderBufferTexture from "./buffer-texture";
import {UniformMap, WebGLQuery} from './gl-utils';
import {PathfinderPackedMeshBuffers, PathfinderPackedMeshes} from "./meshes";
import {ShaderMap} from './shader-loader';
import {FLOAT32_SIZE, Range, UINT16_SIZE, UINT32_SIZE, unwrapNull, unwrapUndef} from './utils';
import {RenderContext, Timings} from "./view";

const MAX_PATHS: number = 65535;

const MAX_VERTICES: number = 4 * 1024 * 1024;

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

    meshBuffers: PathfinderPackedMeshBuffers[] | null;
    meshes: PathfinderPackedMeshes[] | null;

    lastTimings: Timings;

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

    get meshesAttached(): boolean {
        return this.meshBuffers != null && this.meshes != null;
    }

    abstract get isMulticolor(): boolean;
    abstract get needsStencil(): boolean;
    abstract get allowSubpixelAA(): boolean;

    abstract get destFramebuffer(): WebGLFramebuffer | null;
    abstract get destAllocatedSize(): glmatrix.vec2;
    abstract get destUsedSize(): glmatrix.vec2;

    protected antialiasingStrategy: AntialiasingStrategy | null;
    protected pathColorsBufferTextures: PathfinderBufferTexture[];

    protected gammaCorrectionMode: GammaCorrectionMode;

    protected get pathIDsAreInstanced(): boolean {
        return false;
    }

    protected abstract get objectCount(): number;
    protected abstract get usedSizeFactor(): glmatrix.vec2;
    protected abstract get worldTransform(): glmatrix.mat4;

    private implicitCoverInteriorVAO: WebGLVertexArrayObjectOES | null = null;
    private implicitCoverCurveVAO: WebGLVertexArrayObjectOES | null = null;

    private gammaLUTTexture: WebGLTexture | null = null;
    private areaLUTTexture: WebGLTexture | null = null;

    private instancedPathIDVBO: WebGLBuffer | null = null;
    private vertexIDVBO: WebGLBuffer | null = null;
    private timerQueryPollInterval: number | null = null;

    constructor(renderContext: RenderContext) {
        this.renderContext = renderContext;

        this.meshes = null;
        this.meshBuffers = null;

        this.lastTimings = { rendering: 0, compositing: 0 };

        this.gammaCorrectionMode = 'on';

        this.pathTransformBufferTextures = [];
        this.pathColorsBufferTextures = [];

        if (this.pathIDsAreInstanced)
            this.initInstancedPathIDVBO();

        this.initVertexIDVBO();
        this.initLUTTexture('gammaLUT', 'gammaLUTTexture');
        this.initLUTTexture('areaLUT', 'areaLUTTexture');

        this.antialiasingStrategy = new NoAAStrategy(0, 'none');
        this.antialiasingStrategy.init(this);
        this.antialiasingStrategy.setFramebufferSize(this);
    }

    attachMeshes(meshes: PathfinderPackedMeshes[]): void {
        const renderContext = this.renderContext;
        this.meshes = meshes;
        this.meshBuffers = meshes.map(meshes => {
            return new PathfinderPackedMeshBuffers(renderContext.gl, meshes);
        });
        unwrapNull(this.antialiasingStrategy).attachMeshes(this);
    }

    abstract pathBoundingRects(objectIndex: number): Float32Array;
    abstract setHintsUniform(uniforms: UniformMap): void;
    abstract pathTransformsForObject(objectIndex: number): PathTransformBuffers<Float32Array>;

    redraw(): void {
        const renderContext = this.renderContext;

        if (this.meshBuffers == null)
            return;

        this.clearDestFramebuffer();

        // Start timing rendering.
        if (this.timerQueryPollInterval == null &&
            renderContext.timerQueryExt != null &&
            renderContext.atlasRenderingTimerQuery != null) {
            renderContext.timerQueryExt.beginQueryEXT(renderContext.timerQueryExt.TIME_ELAPSED_EXT,
                                                      renderContext.atlasRenderingTimerQuery);
        }

        const antialiasingStrategy = unwrapNull(this.antialiasingStrategy);
        antialiasingStrategy.prepareForRendering(this);

        // Draw "scenery" (used in the 3D view).
        this.drawSceneryIfNecessary();

        const passCount = antialiasingStrategy.passCount;
        for (let pass = 0; pass < passCount; pass++) {
            if (antialiasingStrategy.directRenderingMode !== 'none')
                antialiasingStrategy.prepareForDirectRendering(this);

            const objectCount = this.objectCount;
            for (let objectIndex = 0; objectIndex < objectCount; objectIndex++) {
                if (antialiasingStrategy.directRenderingMode !== 'none') {
                    // Prepare for direct rendering.
                    antialiasingStrategy.prepareToRenderObject(this, objectIndex);

                    // Clear.
                    this.clearForDirectRendering(objectIndex);

                    // Perform direct rendering (Loop-Blinn).
                    this.directlyRenderObject(pass, objectIndex);
                }

                // Antialias.
                antialiasingStrategy.antialiasObject(this, objectIndex);

                // End the timer, and start a new one.
                // FIXME(pcwalton): This is kinda bogus for multipass.
                if (this.timerQueryPollInterval == null &&
                    objectIndex === objectCount - 1 &&
                    pass === passCount - 1 &&
                    renderContext.timerQueryExt != null &&
                    renderContext.compositingTimerQuery != null) {
                    renderContext.timerQueryExt
                                 .endQueryEXT(renderContext.timerQueryExt.TIME_ELAPSED_EXT);
                    renderContext.timerQueryExt
                                .beginQueryEXT(renderContext.timerQueryExt.TIME_ELAPSED_EXT,
                                               renderContext.compositingTimerQuery);
                }

                // Perform post-antialiasing tasks.
                antialiasingStrategy.finishAntialiasingObject(this, objectIndex);

                antialiasingStrategy.resolveAAForObject(this, objectIndex);
            }

            antialiasingStrategy.resolve(pass, this);
        }

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
        if (this.meshes != null)
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

    setTransformAndTexScaleUniformsForDest(uniforms: UniformMap, tileInfo?: TileInfo): void {
        const renderContext = this.renderContext;
        const usedSize = this.usedSizeFactor;

        let tileSize, tilePosition;
        if (tileInfo == null) {
            tileSize = glmatrix.vec2.clone([1.0, 1.0]);
            tilePosition = glmatrix.vec2.create();
        } else {
            tileSize = tileInfo.size;
            tilePosition = tileInfo.position;
        }

        const transform = glmatrix.mat4.create();
        glmatrix.mat4.fromTranslation(transform, [
            -1.0 + tilePosition[0] / tileSize[0] * 2.0,
            -1.0 + tilePosition[1] / tileSize[1] * 2.0,
            0.0,
        ]);
        glmatrix.mat4.scale(transform, transform, [2.0 * usedSize[0], 2.0 * usedSize[1], 1.0]);
        glmatrix.mat4.scale(transform, transform, [1.0 / tileSize[0], 1.0 / tileSize[1], 1.0]);
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

    setTransformUniform(uniforms: UniformMap, pass: number, objectIndex: number): void {
        const transform = this.computeTransform(pass, objectIndex);
        this.renderContext.gl.uniformMatrix4fv(uniforms.uTransform, false, transform);
    }

    setTransformSTUniform(uniforms: UniformMap, objectIndex: number): void {
        // FIXME(pcwalton): Lossy conversion from a 4x4 matrix to an ST matrix is ugly and fragile.
        // Refactor.
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const transform = this.computeTransform(0, objectIndex);

        gl.uniform4f(uniforms.uTransformST,
                     transform[0],
                     transform[5],
                     transform[12],
                     transform[13]);
    }

    affineTransform(objectIndex: number): glmatrix.mat2d {
        // FIXME(pcwalton): Lossy conversion from a 4x4 matrix to an affine matrix is ugly and
        // fragile. Refactor.
        const transform = this.computeTransform(0, objectIndex);
        return glmatrix.mat2d.fromValues(transform[0], transform[1],
                                         transform[4], transform[5],
                                         transform[12], transform[13]);

    }

    setTransformAffineUniforms(uniforms: UniformMap, objectIndex: number): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const transform = this.affineTransform(objectIndex);
        gl.uniform4f(uniforms.uTransformST,
                     transform[0],
                     transform[3],
                     transform[4],
                     transform[5]);
        gl.uniform2f(uniforms.uTransformExt, transform[1], transform[2]);
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
        const gl = renderContext.gl;

        for (let objectIndex = 0; objectIndex < objectCount; objectIndex++) {
            const pathTransforms = this.pathTransformsForObject(objectIndex);

            let pathTransformBufferTextures;
            if (objectIndex >= this.pathTransformBufferTextures.length) {
                pathTransformBufferTextures = {
                    ext: new PathfinderBufferTexture(gl, 'uPathTransformExt'),
                    st: new PathfinderBufferTexture(gl, 'uPathTransformST'),
                };
                this.pathTransformBufferTextures[objectIndex] = pathTransformBufferTextures;
            } else {
                pathTransformBufferTextures = this.pathTransformBufferTextures[objectIndex];
            }

            pathTransformBufferTextures.st.upload(gl, pathTransforms.st);
            pathTransformBufferTextures.ext.upload(gl, pathTransforms.ext);
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

    meshIndexForObject(objectIndex: number): number {
        return objectIndex;
    }

    pathRangeForObject(objectIndex: number): Range {
        if (this.meshBuffers == null)
            return new Range(0, 0);
        const bVertexPathRanges = this.meshBuffers[objectIndex].bQuadVertexPositionPathRanges;
        return new Range(1, bVertexPathRanges.length + 1);
    }

    bindAreaLUT(textureUnit: number, uniforms: UniformMap): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        gl.activeTexture(gl.TEXTURE0 + textureUnit);
        gl.bindTexture(gl.TEXTURE_2D, this.areaLUTTexture);
        gl.uniform1i(uniforms.uAreaLUT, textureUnit);
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

    protected abstract directCurveProgramName(): keyof ShaderMap<void>;
    protected abstract directInteriorProgramName(renderingMode: DirectRenderingMode):
                                                 keyof ShaderMap<void>;

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
        gl.clearDepth(0.0);
        gl.depthMask(true);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
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

    private directlyRenderObject(pass: number, objectIndex: number): void {
        if (this.meshBuffers == null || this.meshes == null)
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

        const meshes = this.meshBuffers![meshIndex];
        const meshData = this.meshes![meshIndex];

        // Set up implicit cover state.
        gl.depthFunc(gl.GREATER);
        gl.depthMask(true);
        gl.enable(gl.DEPTH_TEST);
        gl.disable(gl.BLEND);
        gl.cullFace(gl.BACK);
        gl.frontFace(gl.CCW);
        gl.enable(gl.CULL_FACE);

        // Set up the implicit cover interior VAO.
        const directInteriorProgramName = this.directInteriorProgramName(renderingMode);
        const directInteriorProgram = renderContext.shaderPrograms[directInteriorProgramName];
        if (this.implicitCoverInteriorVAO == null) {
            this.implicitCoverInteriorVAO = renderContext.vertexArrayObjectExt
                                                         .createVertexArrayOES();
        }
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.implicitCoverInteriorVAO);
        this.initImplicitCoverInteriorVAO(objectIndex, instanceRange, renderingMode);

        // Draw direct interior parts.
        if (renderingMode === 'conservative')
            this.setTransformAffineUniforms(directInteriorProgram.uniforms, objectIndex);
        else
            this.setTransformUniform(directInteriorProgram.uniforms, pass, objectIndex);
        this.setFramebufferSizeUniform(directInteriorProgram.uniforms);
        this.setHintsUniform(directInteriorProgram.uniforms);
        this.setPathColorsUniform(objectIndex, directInteriorProgram.uniforms, 0);
        this.setEmboldenAmountUniform(objectIndex, directInteriorProgram.uniforms);
        this.pathTransformBufferTextures[meshIndex].st.bind(gl, directInteriorProgram.uniforms, 1);
        this.pathTransformBufferTextures[meshIndex]
            .ext
            .bind(gl, directInteriorProgram.uniforms, 2);
        const bQuadInteriorRange = getMeshIndexRange(meshes.bQuadVertexInteriorIndexPathRanges,
                                                     pathRange);
        if (!this.pathIDsAreInstanced) {
            gl.drawElements(gl.TRIANGLES,
                            bQuadInteriorRange.length,
                            gl.UNSIGNED_INT,
                            bQuadInteriorRange.start * UINT32_SIZE);
        } else {
            renderContext.instancedArraysExt
                         .drawElementsInstancedANGLE(gl.TRIANGLES,
                                                     bQuadInteriorRange.length,
                                                     gl.UNSIGNED_INT,
                                                     0,
                                                     instanceRange.length);
        }

        gl.disable(gl.CULL_FACE);

        // Render curves, if applicable.
        if (renderingMode !== 'conservative') {
            // Set up direct curve state.
            gl.depthMask(false);
            gl.enable(gl.BLEND);
            gl.blendEquation(gl.FUNC_ADD);
            gl.blendFuncSeparate(gl.ONE, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE);

            // Set up the direct curve VAO.
            //
            // TODO(pcwalton): Cache these.
            const directCurveProgramName = this.directCurveProgramName();
            const directCurveProgram = renderContext.shaderPrograms[directCurveProgramName];
            if (this.implicitCoverCurveVAO == null) {
                this.implicitCoverCurveVAO = renderContext.vertexArrayObjectExt
                                                          .createVertexArrayOES();
            }
            renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.implicitCoverCurveVAO);
            this.initImplicitCoverCurveVAO(objectIndex, instanceRange);

            // Draw direct curve parts.
            this.setTransformUniform(directCurveProgram.uniforms, pass, objectIndex);
            this.setFramebufferSizeUniform(directCurveProgram.uniforms);
            this.setHintsUniform(directCurveProgram.uniforms);
            this.setPathColorsUniform(objectIndex, directCurveProgram.uniforms, 0);
            this.setEmboldenAmountUniform(objectIndex, directCurveProgram.uniforms);
            this.pathTransformBufferTextures[meshIndex]
                .st
                .bind(gl, directCurveProgram.uniforms, 1);
            this.pathTransformBufferTextures[meshIndex]
                .ext
                .bind(gl, directCurveProgram.uniforms, 2);
            const coverCurveRange = getMeshIndexRange(meshes.bQuadVertexPositionPathRanges,
                                                      pathRange);
            if (!this.pathIDsAreInstanced) {
                gl.drawArrays(gl.TRIANGLES, coverCurveRange.start * 6, coverCurveRange.length * 6);
            } else {
                renderContext.instancedArraysExt
                             .drawArraysInstancedANGLE(gl.TRIANGLES,
                                                       0,
                                                       coverCurveRange.length * 6,
                                                       instanceRange.length);
            }
        }

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);

        // Finish direct rendering. Right now, this performs compositing if necessary.
        antialiasingStrategy.finishDirectlyRenderingObject(this, objectIndex);
    }

    private finishTiming(): void {
        const renderContext = this.renderContext;

        if (this.timerQueryPollInterval != null ||
            renderContext.timerQueryExt == null ||
            renderContext.atlasRenderingTimerQuery == null ||
            renderContext.compositingTimerQuery == null) {
            return;
        }

        renderContext.timerQueryExt.endQueryEXT(renderContext.timerQueryExt.TIME_ELAPSED_EXT);

        this.timerQueryPollInterval = window.setInterval(() => {
            if (renderContext.timerQueryExt == null ||
                renderContext.atlasRenderingTimerQuery == null ||
                renderContext.compositingTimerQuery == null) {
                return;
            }

            for (const queryName of ['atlasRenderingTimerQuery', 'compositingTimerQuery'] as
                    Array<'atlasRenderingTimerQuery' | 'compositingTimerQuery'>) {
                if (renderContext.timerQueryExt
                                 .getQueryObjectEXT(renderContext[queryName] as WebGLQuery,
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

    private initLUTTexture(imageName: 'gammaLUT' | 'areaLUT',
                           textureName: 'gammaLUTTexture' | 'areaLUTTexture'):
                           void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const image = renderContext[imageName];
        const texture = unwrapNull(gl.createTexture());
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.LUMINANCE, gl.LUMINANCE, gl.UNSIGNED_BYTE, image);
        const filter = imageName === 'gammaLUT' ? gl.NEAREST : gl.LINEAR;
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

        this[textureName] = texture;
    }

    private initImplicitCoverCurveVAO(objectIndex: number, instanceRange: Range): void {
        if (this.meshBuffers == null)
            return;

        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const meshIndex = this.meshIndexForObject(objectIndex);
        const meshes = this.meshBuffers[meshIndex];
        const meshData = unwrapNull(this.meshes)[meshIndex];

        const directCurveProgramName = this.directCurveProgramName();
        const directCurveProgram = renderContext.shaderPrograms[directCurveProgramName];
        gl.useProgram(directCurveProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, meshes.bQuadVertexPositions);
        gl.vertexAttribPointer(directCurveProgram.attributes.aPosition, 2, gl.FLOAT, false, 0, 0);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.vertexIDVBO);
        gl.vertexAttribPointer(directCurveProgram.attributes.aVertexID, 1, gl.FLOAT, false, 0, 0);

        if (this.pathIDsAreInstanced)
            gl.bindBuffer(gl.ARRAY_BUFFER, this.instancedPathIDVBO);
        else
            gl.bindBuffer(gl.ARRAY_BUFFER, meshes.bQuadVertexPositionPathIDs);
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

        gl.enableVertexAttribArray(directCurveProgram.attributes.aPosition);
        gl.enableVertexAttribArray(directCurveProgram.attributes.aVertexID);
        gl.enableVertexAttribArray(directCurveProgram.attributes.aPathID);
    }

    private initImplicitCoverInteriorVAO(objectIndex: number,
                                         instanceRange: Range,
                                         renderingMode: DirectRenderingMode):
                                         void {
        if (this.meshBuffers == null)
            return;

        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const meshIndex = this.meshIndexForObject(objectIndex);
        const meshes = this.meshBuffers[meshIndex];

        const directInteriorProgramName = this.directInteriorProgramName(renderingMode);
        const directInteriorProgram = renderContext.shaderPrograms[directInteriorProgramName];
        gl.useProgram(directInteriorProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, meshes.bQuadVertexPositions);
        gl.vertexAttribPointer(directInteriorProgram.attributes.aPosition,
                               2,
                               gl.FLOAT,
                               false,
                               0,
                               0);

        if (this.pathIDsAreInstanced)
            gl.bindBuffer(gl.ARRAY_BUFFER, this.instancedPathIDVBO);
        else
            gl.bindBuffer(gl.ARRAY_BUFFER, meshes.bQuadVertexPositionPathIDs);
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

        if (directInteriorProgramName === 'conservativeInterior') {
            gl.bindBuffer(gl.ARRAY_BUFFER, this.vertexIDVBO);
            gl.vertexAttribPointer(directInteriorProgram.attributes.aVertexID,
                                1,
                                gl.FLOAT,
                                false,
                                0,
                                0);
        }

        gl.enableVertexAttribArray(directInteriorProgram.attributes.aPosition);
        gl.enableVertexAttribArray(directInteriorProgram.attributes.aPathID);
        if (directInteriorProgramName === 'conservativeInterior')
            gl.enableVertexAttribArray(directInteriorProgram.attributes.aVertexID);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, meshes.bQuadVertexInteriorIndices);
    }

    private initInstancedPathIDVBO(): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const pathIDs = new Uint16Array(MAX_PATHS);
        for (let pathIndex = 0; pathIndex < MAX_PATHS; pathIndex++)
            pathIDs[pathIndex] = pathIndex + 1;

        this.instancedPathIDVBO = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, this.instancedPathIDVBO);
        gl.bufferData(gl.ARRAY_BUFFER, pathIDs, gl.STATIC_DRAW);
        gl.bindBuffer(gl.ARRAY_BUFFER, null);
    }

    private initVertexIDVBO(): void {
        const renderContext = this.renderContext;
        const gl = renderContext.gl;

        const vertexIDs = new Float32Array(MAX_VERTICES);
        for (let vertexID = 0; vertexID < MAX_VERTICES; vertexID++)
            vertexIDs[vertexID] = vertexID;

        this.vertexIDVBO = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, this.vertexIDVBO);
        gl.bufferData(gl.ARRAY_BUFFER, vertexIDs, gl.STATIC_DRAW);
        gl.bindBuffer(gl.ARRAY_BUFFER, null);
    }

    private computeTransform(pass: number, objectIndex: number): glmatrix.mat4 {
        let transform;
        if (this.antialiasingStrategy == null)
            transform = glmatrix.mat4.create();
        else
            transform = this.antialiasingStrategy.worldTransformForPass(this, pass);

        glmatrix.mat4.mul(transform, transform, this.worldTransform);
        glmatrix.mat4.mul(transform, transform, this.getModelviewTransform(objectIndex));
        return transform;
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
    if (descending)
        endIndex = indexRanges[pathRange.end - 1].start;
    else if (pathRange.end >= indexRanges.length)
        endIndex = lastIndexRange.end;
    else
        endIndex = indexRanges[pathRange.end].start;

    if (descending) {
        const tmp = startIndex;
        startIndex = endIndex;
        endIndex = tmp;
    }

    return new Range(startIndex, endIndex);
}
