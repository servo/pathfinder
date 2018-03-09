// pathfinder/demo/client/src/xcaa-strategy.ts
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

import {AntialiasingStrategy, DirectRenderingMode, SubpixelAAType} from './aa-strategy';
import PathfinderBufferTexture from './buffer-texture';
import {createFramebuffer, createFramebufferColorTexture} from './gl-utils';
import {createFramebufferDepthTexture, setTextureParameters, UniformMap} from './gl-utils';
import {WebGLVertexArrayObject} from './gl-utils';
import {B_QUAD_LOWER_INDICES_OFFSET, B_QUAD_SIZE, B_QUAD_UPPER_INDICES_OFFSET} from './meshes';
import {Renderer} from './renderer';
import {PathfinderShaderProgram, ShaderMap} from './shader-loader';
import {computeStemDarkeningAmount} from './text';
import {assert, FLOAT32_SIZE, lerp, Range, UINT16_SIZE, UINT32_SIZE, unwrapNull} from './utils';
import {unwrapUndef} from './utils';
import {RenderContext} from './view';

interface FastEdgeVAOs {
    upper: WebGLVertexArrayObject;
    lower: WebGLVertexArrayObject;
}

type Direction = 'upper' | 'lower';

const DIRECTIONS: Direction[] = ['upper', 'lower'];

const PATCH_VERTICES: Float32Array = new Float32Array([
    0.0, 0.0,
    0.5, 0.0,
    1.0, 0.0,
    0.0, 1.0,
    0.5, 1.0,
    1.0, 1.0,
]);

const MCAA_PATCH_INDICES: Uint8Array = new Uint8Array([0, 1, 2, 1, 3, 2]);

export type TransformType = 'affine' | '3d';

export abstract class XCAAStrategy extends AntialiasingStrategy {
    abstract readonly directRenderingMode: DirectRenderingMode;

    protected patchVertexBuffer: WebGLBuffer | null = null;
    protected patchIndexBuffer: WebGLBuffer | null = null;

    get passCount(): number {
        return 1;
    }

    protected abstract get transformType(): TransformType;

    protected abstract get patchIndices(): Uint8Array;

    protected pathBoundsBufferTextures: PathfinderBufferTexture[];

    protected supersampledFramebufferSize: glmatrix.vec2;
    protected destFramebufferSize: glmatrix.vec2;

    protected subpixelAA: SubpixelAAType;

    protected resolveVAO: WebGLVertexArrayObject | null;

    protected aaAlphaTexture: WebGLTexture | null = null;
    protected aaDepthTexture: WebGLTexture | null = null;
    protected aaFramebuffer: WebGLFramebuffer | null = null;

    protected abstract get mightUseAAFramebuffer(): boolean;

    constructor(level: number, subpixelAA: SubpixelAAType) {
        super();

        this.subpixelAA = subpixelAA;

        this.supersampledFramebufferSize = glmatrix.vec2.create();
        this.destFramebufferSize = glmatrix.vec2.create();

        this.pathBoundsBufferTextures = [];
    }

    init(renderer: Renderer): void {
        super.init(renderer);
    }

    attachMeshes(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        this.createResolveVAO(renderer);
        this.pathBoundsBufferTextures = [];

        this.patchVertexBuffer = unwrapNull(gl.createBuffer());
        gl.bindBuffer(gl.ARRAY_BUFFER, this.patchVertexBuffer);
        gl.bufferData(gl.ARRAY_BUFFER, PATCH_VERTICES.buffer as ArrayBuffer, gl.STATIC_DRAW);
        gl.bindBuffer(gl.ARRAY_BUFFER, null);

        this.patchIndexBuffer = unwrapNull(gl.createBuffer());
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.patchIndexBuffer);
        gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,
                      this.patchIndices.buffer as ArrayBuffer,
                      gl.STATIC_DRAW);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, null);
    }

    setFramebufferSize(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        this.destFramebufferSize = glmatrix.vec2.clone(renderer.destAllocatedSize);
        glmatrix.vec2.mul(this.supersampledFramebufferSize,
                          this.destFramebufferSize,
                          this.supersampleScale);

        this.initAAAlphaFramebuffer(renderer);
        gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    }

    prepareForRendering(renderer: Renderer): void {}

    prepareForDirectRendering(renderer: Renderer): void {}

    finishAntialiasingObject(renderer: Renderer, objectIndex: number): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        this.initResolveFramebufferForObject(renderer, objectIndex);

        if (!this.usesAAFramebuffer(renderer))
            return;

        const usedSize = this.supersampledUsedSize(renderer);
        gl.scissor(0, 0, usedSize[0], usedSize[1]);
        gl.enable(gl.SCISSOR_TEST);

        // Clear out the color and depth textures.
        gl.clearColor(1.0, 1.0, 1.0, 1.0);
        gl.clearDepth(0.0);
        gl.depthMask(true);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    }

    prepareToRenderObject(renderer: Renderer, objectIndex: number): void {}

    finishDirectlyRenderingObject(renderer: Renderer, objectIndex: number): void {
        // TODO(pcwalton)
    }

    antialiasObject(renderer: Renderer, objectIndex: number): void {
        // Perform early preparations.
        this.createPathBoundsBufferTextureForObjectIfNecessary(renderer, objectIndex);

        // Set up antialiasing.
        this.prepareAA(renderer);

        // Clear.
        this.clearForAA(renderer);
    }

    resolveAAForObject(renderer: Renderer, objectIndex: number): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        if (!this.usesAAFramebuffer(renderer))
            return;

        const resolveProgram = this.getResolveProgram(renderer);
        if (resolveProgram == null)
            return;

        // Set state for XCAA resolve.
        const usedSize = renderer.destUsedSize;
        gl.scissor(0, 0, usedSize[0], usedSize[1]);
        gl.enable(gl.SCISSOR_TEST);
        this.setDepthAndBlendModeForResolve(renderContext);

        // Clear out the resolve buffer, if necessary.
        this.clearForResolve(renderer);

        // Resolve.
        gl.useProgram(resolveProgram.program);
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.resolveVAO);
        gl.uniform2i(resolveProgram.uniforms.uFramebufferSize,
                     this.destFramebufferSize[0],
                     this.destFramebufferSize[1]);
        gl.activeTexture(renderContext.gl.TEXTURE0);
        gl.bindTexture(renderContext.gl.TEXTURE_2D, this.aaAlphaTexture);
        gl.uniform1i(resolveProgram.uniforms.uAAAlpha, 0);
        gl.uniform2i(resolveProgram.uniforms.uAAAlphaDimensions,
                     this.supersampledFramebufferSize[0],
                     this.supersampledFramebufferSize[1]);
        if (renderer.bgColor != null)
            gl.uniform4fv(resolveProgram.uniforms.uBGColor, renderer.bgColor);
        if (renderer.fgColor != null)
            gl.uniform4fv(resolveProgram.uniforms.uFGColor, renderer.fgColor);
        renderer.setTransformSTAndTexScaleUniformsForDest(resolveProgram.uniforms);
        this.setAdditionalStateForResolveIfNecessary(renderer, resolveProgram, 1);
        gl.drawElements(renderContext.gl.TRIANGLES, 6, gl.UNSIGNED_BYTE, 0);
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    resolve(pass: number, renderer: Renderer): void {}

    get transform(): glmatrix.mat4 {
        return glmatrix.mat4.create();
    }

    protected abstract usesAAFramebuffer(renderer: Renderer): boolean;

    protected supersampledUsedSize(renderer: Renderer): glmatrix.vec2 {
        const usedSize = glmatrix.vec2.create();
        glmatrix.vec2.mul(usedSize, renderer.destUsedSize, this.supersampleScale);
        return usedSize;
    }

    protected prepareAA(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        // Set state for antialiasing.
        const usedSize = this.supersampledUsedSize(renderer);
        if (this.usesAAFramebuffer(renderer))
            gl.bindFramebuffer(gl.FRAMEBUFFER, this.aaFramebuffer);
        gl.viewport(0,
                    0,
                    this.supersampledFramebufferSize[0],
                    this.supersampledFramebufferSize[1]);
        gl.scissor(0, 0, usedSize[0], usedSize[1]);
        gl.enable(gl.SCISSOR_TEST);
    }

    protected setAAState(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const usedSize = this.supersampledUsedSize(renderer);
        if (this.usesAAFramebuffer(renderer))
            gl.bindFramebuffer(gl.FRAMEBUFFER, this.aaFramebuffer);
        gl.viewport(0,
                    0,
                    this.supersampledFramebufferSize[0],
                    this.supersampledFramebufferSize[1]);
        gl.scissor(0, 0, usedSize[0], usedSize[1]);
        gl.enable(gl.SCISSOR_TEST);

        this.setAADepthState(renderer);
    }

    protected setAAUniforms(renderer: Renderer, uniforms: UniformMap, objectIndex: number): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        switch (this.transformType) {
        case 'affine':
            renderer.setTransformAffineUniforms(uniforms, 0);
            break;
        case '3d':
            renderer.setTransformUniform(uniforms, 0, 0);
            break;
        }

        gl.uniform2i(uniforms.uFramebufferSize,
                     this.supersampledFramebufferSize[0],
                     this.supersampledFramebufferSize[1]);
        renderer.pathTransformBufferTextures[0].ext.bind(gl, uniforms, 0);
        renderer.pathTransformBufferTextures[0].st.bind(gl, uniforms, 1);
        this.pathBoundsBufferTextures[objectIndex].bind(gl, uniforms, 2);
        renderer.setHintsUniform(uniforms);
    }

    protected setDepthAndBlendModeForResolve(renderContext: RenderContext): void {
        const gl = renderContext.gl;
        gl.disable(gl.DEPTH_TEST);
        gl.disable(gl.BLEND);
    }

    protected setAdditionalStateForResolveIfNecessary(renderer: Renderer,
                                                      resolveProgram: PathfinderShaderProgram,
                                                      firstFreeTextureUnit: number):
                                                      void {}

    protected abstract clearForAA(renderer: Renderer): void;
    protected abstract getResolveProgram(renderer: Renderer): PathfinderShaderProgram | null;
    protected abstract setAADepthState(renderer: Renderer): void;
    protected abstract clearForResolve(renderer: Renderer): void;

    private initResolveFramebufferForObject(renderer: Renderer, objectIndex: number): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.bindFramebuffer(gl.FRAMEBUFFER, renderer.destFramebuffer);
        renderer.setDrawViewport();
        gl.disable(gl.SCISSOR_TEST);
    }

    private initAAAlphaFramebuffer(renderer: Renderer): void {
        if (!this.mightUseAAFramebuffer) {
            this.aaAlphaTexture = null;
            this.aaDepthTexture = null;
            this.aaFramebuffer = null;
            return;
        }

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        this.aaAlphaTexture = unwrapNull(gl.createTexture());
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, this.aaAlphaTexture);
        gl.texImage2D(gl.TEXTURE_2D,
                      0,
                      gl.RGB,
                      this.supersampledFramebufferSize[0],
                      this.supersampledFramebufferSize[1],
                      0,
                      gl.RGB,
                      renderContext.textureHalfFloatExt.HALF_FLOAT_OES,
                      null);
        setTextureParameters(gl, gl.NEAREST);

        this.aaDepthTexture = createFramebufferDepthTexture(gl, this.supersampledFramebufferSize);
        this.aaFramebuffer = createFramebuffer(gl, this.aaAlphaTexture, this.aaDepthTexture);
    }

    private createPathBoundsBufferTextureForObjectIfNecessary(renderer: Renderer,
                                                              objectIndex: number):
                                                              void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const pathBounds = renderer.pathBoundingRects(objectIndex);

        if (this.pathBoundsBufferTextures[objectIndex] == null) {
            this.pathBoundsBufferTextures[objectIndex] =
                new PathfinderBufferTexture(gl, 'uPathBounds');
        }

        this.pathBoundsBufferTextures[objectIndex].upload(gl, pathBounds);
    }

    private createResolveVAO(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const resolveProgram = this.getResolveProgram(renderer);
        if (resolveProgram == null)
            return;

        this.resolveVAO = renderContext.vertexArrayObjectExt.createVertexArrayOES();
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.resolveVAO);

        gl.useProgram(resolveProgram.program);
        renderContext.initQuadVAO(resolveProgram.attributes);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected get directDepthTexture(): WebGLTexture | null {
        return null;
    }

    protected get supersampleScale(): glmatrix.vec2 {
        return glmatrix.vec2.fromValues(this.subpixelAA !== 'none' ? 3.0 : 1.0, 1.0);
    }
}

export class MCAAStrategy extends XCAAStrategy {
    protected vao: WebGLVertexArrayObject | null;

    protected get patchIndices(): Uint8Array {
        return MCAA_PATCH_INDICES;
    }

    protected get transformType(): TransformType {
        return 'affine';
    }

    protected get mightUseAAFramebuffer(): boolean {
        return true;
    }

    attachMeshes(renderer: Renderer): void {
        super.attachMeshes(renderer);

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        this.vao = renderContext.vertexArrayObjectExt.createVertexArrayOES();
    }

    antialiasObject(renderer: Renderer, objectIndex: number): void {
        super.antialiasObject(renderer, objectIndex);

        const shaderProgram = this.edgeProgram(renderer);
        this.antialiasEdgesOfObjectWithProgram(renderer, objectIndex, shaderProgram);
    }

    protected usesAAFramebuffer(renderer: Renderer): boolean {
        return !renderer.isMulticolor;
    }

    protected getResolveProgram(renderer: Renderer): PathfinderShaderProgram | null {
        const renderContext = renderer.renderContext;
        if (renderer.isMulticolor)
            return null;
        if (this.subpixelAA !== 'none')
            return renderContext.shaderPrograms.xcaaMonoSubpixelResolve;
        return renderContext.shaderPrograms.xcaaMonoResolve;
    }

    protected clearForAA(renderer: Renderer): void {
        if (!this.usesAAFramebuffer(renderer))
            return;

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.clearColor(0.0, 0.0, 0.0, 0.0);
        gl.clearDepth(0.0);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    }

    protected setAADepthState(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        if (this.directRenderingMode !== 'conservative') {
            gl.disable(gl.DEPTH_TEST);
            return;
        }

        gl.depthFunc(gl.GREATER);
        gl.depthMask(false);
        gl.enable(gl.DEPTH_TEST);
        gl.disable(gl.CULL_FACE);
    }

    protected clearForResolve(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        if (!renderer.isMulticolor) {
            gl.clearColor(0.0, 0.0, 0.0, 1.0);
            gl.clear(gl.COLOR_BUFFER_BIT);
        }
    }

    protected setBlendModeForAA(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        if (renderer.isMulticolor)
            gl.blendFuncSeparate(gl.ONE, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE);
        else
            gl.blendFunc(gl.ONE, gl.ONE);

        gl.blendEquation(gl.FUNC_ADD);
        gl.enable(gl.BLEND);
    }

    protected prepareAA(renderer: Renderer): void {
        super.prepareAA(renderer);

        this.setBlendModeForAA(renderer);
    }

    protected initVAOForObject(renderer: Renderer, objectIndex: number): void {
        if (renderer.meshBuffers == null || renderer.meshes == null)
            return;

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const pathRange = renderer.pathRangeForObject(objectIndex);
        const meshIndex = renderer.meshIndexForObject(objectIndex);

        const shaderProgram = this.edgeProgram(renderer);
        const attributes = shaderProgram.attributes;

        // FIXME(pcwalton): Refactor.
        const vao = this.vao;
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

        const bBoxRanges = renderer.meshes[meshIndex].bBoxPathRanges;
        const offset = calculateStartFromIndexRanges(pathRange, bBoxRanges);

        gl.useProgram(shaderProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, renderer.renderContext.quadPositionsBuffer);
        gl.vertexAttribPointer(attributes.aTessCoord, 2, gl.FLOAT, false, FLOAT32_SIZE * 2, 0);
        gl.bindBuffer(gl.ARRAY_BUFFER, renderer.meshBuffers[meshIndex].bBoxes);
        gl.vertexAttribPointer(attributes.aRect,
                               4,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE * 20,
                               FLOAT32_SIZE * 0 + offset * FLOAT32_SIZE * 20);
        gl.vertexAttribPointer(attributes.aUV,
                               4,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE * 20,
                               FLOAT32_SIZE * 4 + offset * FLOAT32_SIZE * 20);
        gl.vertexAttribPointer(attributes.aDUVDX,
                               4,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE * 20,
                               FLOAT32_SIZE * 8 + offset * FLOAT32_SIZE * 20);
        gl.vertexAttribPointer(attributes.aDUVDY,
                               4,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE * 20,
                               FLOAT32_SIZE * 12 + offset * FLOAT32_SIZE * 20);
        gl.vertexAttribPointer(attributes.aSignMode,
                               4,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE * 20,
                               FLOAT32_SIZE * 16 + offset * FLOAT32_SIZE * 20);

        gl.enableVertexAttribArray(attributes.aTessCoord);
        gl.enableVertexAttribArray(attributes.aRect);
        gl.enableVertexAttribArray(attributes.aUV);
        gl.enableVertexAttribArray(attributes.aDUVDX);
        gl.enableVertexAttribArray(attributes.aDUVDY);
        gl.enableVertexAttribArray(attributes.aSignMode);

        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRect, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aUV, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aDUVDX, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aDUVDY, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aSignMode, 1);

        gl.bindBuffer(gl.ARRAY_BUFFER, renderer.meshBuffers[meshIndex].bBoxPathIDs);
        gl.vertexAttribPointer(attributes.aPathID,
                               1,
                               gl.UNSIGNED_SHORT,
                               false,
                               UINT16_SIZE,
                               offset * UINT16_SIZE);
        gl.enableVertexAttribArray(attributes.aPathID);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);

        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, renderer.renderContext.quadElementsBuffer);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected edgeProgram(renderer: Renderer): PathfinderShaderProgram {
        return renderer.renderContext.shaderPrograms.mcaa;
    }

    protected antialiasEdgesOfObjectWithProgram(renderer: Renderer,
                                                objectIndex: number,
                                                shaderProgram: PathfinderShaderProgram):
                                                void {
        if (renderer.meshBuffers == null || renderer.meshes == null)
            return;

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const pathRange = renderer.pathRangeForObject(objectIndex);
        const meshIndex = renderer.meshIndexForObject(objectIndex);

        this.initVAOForObject(renderer, objectIndex);

        gl.useProgram(shaderProgram.program);
        const uniforms = shaderProgram.uniforms;
        this.setAAUniforms(renderer, uniforms, objectIndex);

        // FIXME(pcwalton): Refactor.
        const vao = this.vao;
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

        this.setBlendModeForAA(renderer);
        this.setAADepthState(renderer);

        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, renderContext.quadElementsBuffer);

        const bBoxRanges = renderer.meshes[meshIndex].bBoxPathRanges;
        const count = calculateCountFromIndexRanges(pathRange, bBoxRanges);

        renderContext.instancedArraysExt
                     .drawElementsInstancedANGLE(gl.TRIANGLES, 6, gl.UNSIGNED_BYTE, 0, count);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
        gl.disable(gl.DEPTH_TEST);
        gl.disable(gl.CULL_FACE);
    }

    get directRenderingMode(): DirectRenderingMode {
        // FIXME(pcwalton): Only in multicolor mode?
        return 'conservative';
    }

    protected setAAUniforms(renderer: Renderer, uniforms: UniformMap, objectIndex: number): void {
        super.setAAUniforms(renderer, uniforms, objectIndex);

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        renderer.setPathColorsUniform(0, uniforms, 3);

        gl.uniform1i(uniforms.uMulticolor, renderer.isMulticolor ? 1 : 0);
    }
}

export class StencilAAAStrategy extends XCAAStrategy {
    directRenderingMode: DirectRenderingMode = 'none';

    protected transformType: TransformType = 'affine';
    protected patchIndices: Uint8Array = MCAA_PATCH_INDICES;
    protected mightUseAAFramebuffer: boolean = true;

    private vao: WebGLVertexArrayObject;

    attachMeshes(renderer: Renderer): void {
        super.attachMeshes(renderer);
        this.createVAO(renderer);
    }

    antialiasObject(renderer: Renderer, objectIndex: number): void {
        super.antialiasObject(renderer, objectIndex);

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        if (renderer.meshes == null)
            return;

        // Antialias.
        const shaderPrograms = renderer.renderContext.shaderPrograms;
        this.setAAState(renderer);
        this.setBlendModeForAA(renderer);

        const program = renderContext.shaderPrograms.stencilAAA;
        gl.useProgram(program.program);
        const uniforms = program.uniforms;
        this.setAAUniforms(renderer, uniforms, objectIndex);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.vao);

        // FIXME(pcwalton): Only render the appropriate instances.
        const count = renderer.meshes[0].count('stencilSegments');
        renderContext.instancedArraysExt
                     .drawElementsInstancedANGLE(gl.TRIANGLES, 6, gl.UNSIGNED_BYTE, 0, count);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected usesAAFramebuffer(renderer: Renderer): boolean {
        return true;
    }

    protected clearForAA(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.clearColor(0.0, 0.0, 0.0, 0.0);
        gl.clearDepth(0.0);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    }

    protected getResolveProgram(renderer: Renderer): PathfinderShaderProgram | null {
        const renderContext = renderer.renderContext;

        if (this.subpixelAA !== 'none')
            return renderContext.shaderPrograms.xcaaMonoSubpixelResolve;
        return renderContext.shaderPrograms.xcaaMonoResolve;
    }

    protected setAADepthState(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.disable(gl.DEPTH_TEST);
        gl.disable(gl.CULL_FACE);
    }

    protected setAAUniforms(renderer: Renderer, uniforms: UniformMap, objectIndex: number):
                            void {
        super.setAAUniforms(renderer, uniforms, objectIndex);
        renderer.setEmboldenAmountUniform(objectIndex, uniforms);
    }

    protected clearForResolve(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.clearColor(0.0, 0.0, 0.0, 0.0);
        gl.clear(gl.COLOR_BUFFER_BIT);
    }

    private createVAO(renderer: Renderer): void {
        if (renderer.meshBuffers == null || renderer.meshes == null)
            return;

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const program = renderContext.shaderPrograms.stencilAAA;
        const attributes = program.attributes;

        this.vao = renderContext.vertexArrayObjectExt.createVertexArrayOES();
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.vao);

        const vertexPositionsBuffer = renderer.meshBuffers[0].stencilSegments;
        const vertexNormalsBuffer = renderer.meshBuffers[0].stencilNormals;
        const pathIDsBuffer = renderer.meshBuffers[0].stencilSegmentPathIDs;

        gl.useProgram(program.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, renderContext.quadPositionsBuffer);
        gl.vertexAttribPointer(attributes.aTessCoord, 2, gl.FLOAT, false, 0, 0);
        gl.bindBuffer(gl.ARRAY_BUFFER, vertexPositionsBuffer);
        gl.vertexAttribPointer(attributes.aFromPosition, 2, gl.FLOAT, false, FLOAT32_SIZE * 6, 0);
        gl.vertexAttribPointer(attributes.aCtrlPosition,
                               2,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE * 6,
                               FLOAT32_SIZE * 2);
        gl.vertexAttribPointer(attributes.aToPosition,
                               2,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE * 6,
                               FLOAT32_SIZE * 4);
        gl.bindBuffer(gl.ARRAY_BUFFER, vertexNormalsBuffer);
        gl.vertexAttribPointer(attributes.aFromNormal, 2, gl.FLOAT, false, FLOAT32_SIZE * 6, 0);
        gl.vertexAttribPointer(attributes.aCtrlNormal,
                               2,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE * 6,
                               FLOAT32_SIZE * 2);
        gl.vertexAttribPointer(attributes.aToNormal,
                               2,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE * 6,
                               FLOAT32_SIZE * 4);
        gl.bindBuffer(gl.ARRAY_BUFFER, pathIDsBuffer);
        gl.vertexAttribPointer(attributes.aPathID, 1, gl.UNSIGNED_SHORT, false, 0, 0);

        gl.enableVertexAttribArray(attributes.aTessCoord);
        gl.enableVertexAttribArray(attributes.aFromPosition);
        gl.enableVertexAttribArray(attributes.aCtrlPosition);
        gl.enableVertexAttribArray(attributes.aToPosition);
        gl.enableVertexAttribArray(attributes.aFromNormal);
        gl.enableVertexAttribArray(attributes.aCtrlNormal);
        gl.enableVertexAttribArray(attributes.aToNormal);
        gl.enableVertexAttribArray(attributes.aPathID);

        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aFromPosition, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aCtrlPosition, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aToPosition, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aFromNormal, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aCtrlNormal, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aToNormal, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);

        // TODO(pcwalton): Normals.

        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, renderContext.quadElementsBuffer);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private setBlendModeForAA(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.blendEquation(gl.FUNC_ADD);
        gl.blendFunc(gl.ONE, gl.ONE);
        gl.enable(gl.BLEND);
    }
}

/// Switches between mesh-based and stencil-based analytic antialiasing depending on whether stem
/// darkening is enabled.
///
/// FIXME(pcwalton): Share textures and FBOs between the two strategies.
export class AdaptiveStencilMeshAAAStrategy implements AntialiasingStrategy {
    private meshStrategy: MCAAStrategy;
    private stencilStrategy: StencilAAAStrategy;

    get directRenderingMode(): DirectRenderingMode {
        return 'none';
    }

    get passCount(): number {
        return 1;
    }

    constructor(level: number, subpixelAA: SubpixelAAType) {
        this.meshStrategy = new MCAAStrategy(level, subpixelAA);
        this.stencilStrategy = new StencilAAAStrategy(level, subpixelAA);
    }

    init(renderer: Renderer): void {
        this.meshStrategy.init(renderer);
        this.stencilStrategy.init(renderer);
    }

    attachMeshes(renderer: Renderer): void {
        this.meshStrategy.attachMeshes(renderer);
        this.stencilStrategy.attachMeshes(renderer);
    }

    setFramebufferSize(renderer: Renderer): void {
        this.meshStrategy.setFramebufferSize(renderer);
        this.stencilStrategy.setFramebufferSize(renderer);
    }

    get transform(): glmatrix.mat4 {
        return this.meshStrategy.transform;
    }

    prepareForRendering(renderer: Renderer): void {
        this.getAppropriateStrategy(renderer).prepareForRendering(renderer);
    }

    prepareForDirectRendering(renderer: Renderer): void {
        this.getAppropriateStrategy(renderer).prepareForDirectRendering(renderer);
    }

    finishAntialiasingObject(renderer: Renderer, objectIndex: number): void {
        this.getAppropriateStrategy(renderer).finishAntialiasingObject(renderer, objectIndex);
    }

    prepareToRenderObject(renderer: Renderer, objectIndex: number): void {
        this.getAppropriateStrategy(renderer).prepareToRenderObject(renderer, objectIndex);
    }

    finishDirectlyRenderingObject(renderer: Renderer, objectIndex: number): void {
        this.getAppropriateStrategy(renderer).finishDirectlyRenderingObject(renderer, objectIndex);
    }

    antialiasObject(renderer: Renderer, objectIndex: number): void {
        this.getAppropriateStrategy(renderer).antialiasObject(renderer, objectIndex);
    }

    resolveAAForObject(renderer: Renderer, objectIndex: number): void {
        this.getAppropriateStrategy(renderer).resolveAAForObject(renderer, objectIndex);
    }

    resolve(pass: number, renderer: Renderer): void {
        this.getAppropriateStrategy(renderer).resolve(pass, renderer);
    }

    worldTransformForPass(renderer: Renderer, pass: number): glmatrix.mat4 {
        return glmatrix.mat4.create();
    }

    private getAppropriateStrategy(renderer: Renderer): AntialiasingStrategy {
        return renderer.needsStencil ? this.stencilStrategy : this.meshStrategy;
    }
}

function calculateStartFromIndexRanges(pathRange: Range, indexRanges: Range[]): number {
    return indexRanges.length === 0 ? 0 : indexRanges[pathRange.start - 1].start;
}

function calculateCountFromIndexRanges(pathRange: Range, indexRanges: Range[]): number {
    if (indexRanges.length === 0)
        return 0;

    let lastIndex;
    if (pathRange.end - 1 >= indexRanges.length)
        lastIndex = unwrapUndef(_.last(indexRanges)).end;
    else
        lastIndex = indexRanges[pathRange.end - 1].start;

    const firstIndex = indexRanges[pathRange.start - 1].start;

    return lastIndex - firstIndex;
}
