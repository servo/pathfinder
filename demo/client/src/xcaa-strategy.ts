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

export abstract class XCAAStrategy extends AntialiasingStrategy {
    abstract readonly directRenderingMode: DirectRenderingMode;

    get passCount(): number {
        return 1;
    }

    protected abstract get usesDilationTransforms(): boolean;

    protected pathBoundsBufferTextures: PathfinderBufferTexture[];

    protected supersampledFramebufferSize: glmatrix.vec2;
    protected destFramebufferSize: glmatrix.vec2;

    protected subpixelAA: SubpixelAAType;

    protected resolveVAO: WebGLVertexArrayObject | null;

    protected aaAlphaTexture: WebGLTexture | null;
    protected aaDepthTexture: WebGLTexture | null;
    protected aaFramebuffer: WebGLFramebuffer | null;

    protected abstract get mightUseAAFramebuffer(): boolean;

    constructor(level: number, subpixelAA: SubpixelAAType) {
        super();

        this.subpixelAA = subpixelAA;

        this.supersampledFramebufferSize = glmatrix.vec2.create();
        this.destFramebufferSize = glmatrix.vec2.create();
    }

    init(renderer: Renderer): void {
        super.init(renderer);
    }

    attachMeshes(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        this.createResolveVAO(renderer);
        this.pathBoundsBufferTextures = [];
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

        // Set state for ECAA resolve.
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

        if (this.usesDilationTransforms)
            renderer.setTransformSTUniform(uniforms, 0);
        else
            renderer.setTransformUniform(uniforms, 0, 0);

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
        gl.viewport(0, 0, this.destFramebufferSize[0], this.destFramebufferSize[1]);
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

    protected get usesDilationTransforms(): boolean {
        return true;
    }

    protected get mightUseAAFramebuffer(): boolean {
        return true;
    }

    attachMeshes(renderer: Renderer): void {
        super.attachMeshes(renderer);

        const renderContext = renderer.renderContext;
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
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        if (renderer.isMulticolor)
            gl.clearColor(1.0, 1.0, 1.0, 1.0);
        else
            gl.clearColor(0.0, 0.0, 0.0, 0.0);

        gl.clearDepth(0.0);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    }

    protected setAADepthState(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.disable(gl.DEPTH_TEST);
    }

    protected clearForResolve(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        if (!renderer.isMulticolor) {
            gl.clearColor(1.0, 1.0, 0.0, 1.0);
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
        if (renderer.meshes == null || renderer.meshData == null)
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

        const bQuadRanges = renderer.meshData[meshIndex].bQuadPathRanges;
        const offset = calculateStartFromIndexRanges(pathRange, bQuadRanges);

        gl.useProgram(shaderProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, renderContext.quadPositionsBuffer);
        gl.vertexAttribPointer(attributes.aQuadPosition, 2, gl.FLOAT, false, 0, 0);
        gl.bindBuffer(gl.ARRAY_BUFFER, renderer.meshes[meshIndex].bQuadVertexPositions);
        gl.vertexAttribPointer(attributes.aUpperEndpointPositions,
                               4,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE * 12,
                               FLOAT32_SIZE * 12 * offset);
        gl.vertexAttribPointer(attributes.aLowerEndpointPositions,
                               4,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE * 12,
                               FLOAT32_SIZE * 12 * offset + FLOAT32_SIZE * 4);
        gl.vertexAttribPointer(attributes.aControlPointPositions,
                               4,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE * 12,
                               FLOAT32_SIZE * 12 * offset + FLOAT32_SIZE * 8);
        renderContext.instancedArraysExt
                     .vertexAttribDivisorANGLE(attributes.aUpperEndpointPositions, 1);
        renderContext.instancedArraysExt
                     .vertexAttribDivisorANGLE(attributes.aLowerEndpointPositions, 1);
        renderContext.instancedArraysExt
                     .vertexAttribDivisorANGLE(attributes.aControlPointPositions, 1);

        gl.bindBuffer(gl.ARRAY_BUFFER, renderer.meshes[meshIndex].edgeBoundingBoxPathIDs);
        gl.vertexAttribPointer(attributes.aPathID,
                               1,
                               gl.UNSIGNED_SHORT,
                               false,
                               0,
                               UINT16_SIZE * offset);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);

        gl.enableVertexAttribArray(attributes.aQuadPosition);
        gl.enableVertexAttribArray(attributes.aUpperEndpointPositions);
        gl.enableVertexAttribArray(attributes.aLowerEndpointPositions);
        gl.enableVertexAttribArray(attributes.aControlPointPositions);
        gl.enableVertexAttribArray(attributes.aPathID);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, renderContext.quadElementsBuffer);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected edgeProgram(renderer: Renderer): PathfinderShaderProgram {
        return renderer.renderContext.shaderPrograms.mcaa;
    }

    protected antialiasEdgesOfObjectWithProgram(renderer: Renderer,
                                                objectIndex: number,
                                                shaderProgram: PathfinderShaderProgram):
                                                void {
        if (renderer.meshData == null)
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

        const bQuadRanges = renderer.meshData[meshIndex].bQuadPathRanges;
        const count = calculateCountFromIndexRanges(pathRange, bQuadRanges);

        renderContext.instancedArraysExt
                     .drawElementsInstancedANGLE(gl.TRIANGLES, 6, gl.UNSIGNED_BYTE, 0, count);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    get directRenderingMode(): DirectRenderingMode {
        return 'none';
    }

    protected setAAUniforms(renderer: Renderer, uniforms: UniformMap, objectIndex: number):
                            void {
        super.setAAUniforms(renderer, uniforms, objectIndex);

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        renderer.setPathColorsUniform(0, uniforms, 3);

        gl.uniform1i(uniforms.uMulticolor, renderer.isMulticolor ? 1 : 0);
    }
}

export class ECAAStrategy extends XCAAStrategy {
    protected get lineShaderProgramNames(): Array<keyof ShaderMap<void>> {
        return ['ecaaLine'];
    }

    protected get curveShaderProgramNames(): Array<keyof ShaderMap<void>> {
        return ['ecaaCurve', 'ecaaTransformedCurve'];
    }

    protected get mightUseAAFramebuffer(): boolean {
        return true;
    }

    private lineVAOs: Partial<ShaderMap<WebGLVertexArrayObject>>;
    private curveVAOs: Partial<ShaderMap<WebGLVertexArrayObject>>;

    protected get usesDilationTransforms(): boolean {
        return false;
    }

    get directRenderingMode(): DirectRenderingMode {
        return 'none';
    }

    attachMeshes(renderer: Renderer): void {
        super.attachMeshes(renderer);

        this.createLineVAOs(renderer);
        this.createCurveVAOs(renderer);
    }

    antialiasObject(renderer: Renderer, objectIndex: number): void {
        super.antialiasObject(renderer, objectIndex);

        // Antialias.
        const shaderPrograms = renderer.renderContext.shaderPrograms;
        this.setAAState(renderer);
        this.setBlendModeForAA(renderer);
        this.antialiasLinesOfObjectWithProgram(renderer,
                                               objectIndex,
                                               this.lineShaderProgramNames[0]);
        this.antialiasCurvesOfObjectWithPrograms(renderer,
                                                 objectIndex,
                                                 this.curveShaderProgramNames[0],
                                                 this.curveShaderProgramNames[1]);
    }

    protected usesAAFramebuffer(): boolean {
        return true;
    }

    protected setAAUniforms(renderer: Renderer, uniforms: UniformMap, objectIndex: number):
                            void {
        super.setAAUniforms(renderer, uniforms, objectIndex);
        renderer.setEmboldenAmountUniform(objectIndex, uniforms);
    }

    protected getResolveProgram(renderer: Renderer): PathfinderShaderProgram {
        const renderContext = renderer.renderContext;

        if (this.subpixelAA !== 'none')
            return renderContext.shaderPrograms.xcaaMonoSubpixelResolve;
        return renderContext.shaderPrograms.xcaaMonoResolve;
    }

    protected clearForAA(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.clearColor(0.0, 0.0, 0.0, 0.0);
        gl.clearDepth(0.0);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    }

    protected setAADepthState(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        renderContext.gl.disable(renderContext.gl.DEPTH_TEST);
    }

    protected clearForResolve(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.clearColor(0.0, 0.0, 0.0, 0.0);
        gl.clear(gl.COLOR_BUFFER_BIT);
    }

    protected antialiasLinesOfObjectWithProgram(renderer: Renderer,
                                                objectIndex: number,
                                                programName: keyof ShaderMap<void>):
                                                void {
        if (renderer.meshData == null)
            return;

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const pathRange = renderer.pathRangeForObject(objectIndex);
        const meshIndex = renderer.meshIndexForObject(objectIndex);

        const lineProgram = renderContext.shaderPrograms[programName];
        gl.useProgram(lineProgram.program);
        const uniforms = lineProgram.uniforms;
        this.setAAUniforms(renderer, uniforms, objectIndex);

        const vao = this.lineVAOs[programName];
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

        // FIXME(pcwalton): Only render the appropriate instances.
        const count = renderer.meshData[meshIndex].segmentLineCount;
        renderContext.instancedArraysExt
                     .drawElementsInstancedANGLE(gl.TRIANGLES, 6, gl.UNSIGNED_BYTE, 0, count);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected antialiasCurvesOfObjectWithPrograms(renderer: Renderer,
                                                  objectIndex: number,
                                                  stProgram: keyof ShaderMap<void>,
                                                  transformedProgram: keyof ShaderMap<void>):
                                                  void {
        if (renderer.usesSTTransform) {
            this.antialiasCurvesOfObjectWithProgram(renderer, objectIndex, stProgram, 0);
            return;
        }

        this.antialiasCurvesOfObjectWithProgram(renderer, objectIndex, transformedProgram, 0);
        this.antialiasCurvesOfObjectWithProgram(renderer, objectIndex, transformedProgram, 1);
    }

    private antialiasCurvesOfObjectWithProgram(renderer: Renderer,
                                               objectIndex: number,
                                               programName: keyof ShaderMap<void>,
                                               passIndex: number):
                                               void {
        if (renderer.meshData == null)
            return;

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const pathRange = renderer.pathRangeForObject(objectIndex);
        const meshIndex = renderer.meshIndexForObject(objectIndex);

        const curveProgram = renderContext.shaderPrograms[programName];
        gl.useProgram(curveProgram.program);
        const uniforms = curveProgram.uniforms;
        this.setAAUniforms(renderer, uniforms, objectIndex);
        gl.uniform1i(uniforms.uPassIndex, passIndex);

        const vao = this.curveVAOs[programName];
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

        // FIXME(pcwalton): Only render the appropriate instances.
        const count = renderer.meshData[meshIndex].segmentCurveCount;
        renderContext.instancedArraysExt
                     .drawElementsInstancedANGLE(gl.TRIANGLES, 6, gl.UNSIGNED_BYTE, 0, count);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private setBlendModeForAA(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.blendEquation(gl.FUNC_ADD);
        gl.blendFunc(gl.ONE, gl.ONE);
        gl.enable(gl.BLEND);
    }

    private createLineVAOs(renderer: Renderer): void {
        if (renderer.meshes == null || renderer.meshData == null)
            return;

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        this.lineVAOs = {};

        for (const programName of this.lineShaderProgramNames) {
            const lineProgram = renderContext.shaderPrograms[programName];
            const attributes = lineProgram.attributes;

            const vao = renderContext.vertexArrayObjectExt.createVertexArrayOES();
            renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

            const lineVertexPositionsBuffer = renderer.meshes[0].segmentLines;
            const linePathIDsBuffer = renderer.meshes[0].segmentLinePathIDs;
            const lineNormalsBuffer = renderer.meshes[0].segmentLineNormals;

            gl.useProgram(lineProgram.program);
            gl.bindBuffer(gl.ARRAY_BUFFER, renderContext.quadPositionsBuffer);
            gl.vertexAttribPointer(attributes.aQuadPosition, 2, gl.FLOAT, false, 0, 0);
            gl.bindBuffer(gl.ARRAY_BUFFER, lineVertexPositionsBuffer);
            gl.vertexAttribPointer(attributes.aLeftPosition,
                                   2,
                                   gl.FLOAT,
                                   false,
                                   FLOAT32_SIZE * 4,
                                   0);
            gl.vertexAttribPointer(attributes.aRightPosition,
                                   2,
                                   gl.FLOAT,
                                   false,
                                   FLOAT32_SIZE * 4,
                                   FLOAT32_SIZE * 2);
            gl.bindBuffer(gl.ARRAY_BUFFER, linePathIDsBuffer);
            gl.vertexAttribPointer(attributes.aPathID, 1, gl.UNSIGNED_SHORT, false, 0, 0);

            gl.enableVertexAttribArray(attributes.aQuadPosition);
            gl.enableVertexAttribArray(attributes.aLeftPosition);
            gl.enableVertexAttribArray(attributes.aRightPosition);
            gl.enableVertexAttribArray(attributes.aPathID);

            renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
            renderContext.instancedArraysExt
                         .vertexAttribDivisorANGLE(attributes.aRightPosition, 1);
            renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);

            if (renderer.meshData[0].segmentLineNormals.byteLength > 0) {
                gl.bindBuffer(gl.ARRAY_BUFFER, lineNormalsBuffer);
                gl.vertexAttribPointer(attributes.aLeftNormalAngle,
                                       1,
                                       gl.FLOAT,
                                       false,
                                       FLOAT32_SIZE * 2,
                                       0);
                gl.vertexAttribPointer(attributes.aRightNormalAngle,
                                       1,
                                       gl.FLOAT,
                                       false,
                                       FLOAT32_SIZE * 2,
                                       FLOAT32_SIZE);

                gl.enableVertexAttribArray(attributes.aLeftNormalAngle);
                gl.enableVertexAttribArray(attributes.aRightNormalAngle);

                renderContext.instancedArraysExt
                             .vertexAttribDivisorANGLE(attributes.aLeftNormalAngle, 1);
                renderContext.instancedArraysExt
                             .vertexAttribDivisorANGLE(attributes.aRightNormalAngle, 1);
            }

            gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, renderContext.quadElementsBuffer);

            renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);

            this.lineVAOs[programName] = vao;
        }
    }

    private createCurveVAOs(renderer: Renderer): void {
        if (renderer.meshes == null || renderer.meshData == null)
            return;

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        this.curveVAOs = {};

        for (const programName of this.curveShaderProgramNames) {
            const curveProgram = renderContext.shaderPrograms[programName];
            const attributes = curveProgram.attributes;

            const vao = renderContext.vertexArrayObjectExt.createVertexArrayOES();
            renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

            const curveVertexPositionsBuffer = renderer.meshes[0].segmentCurves;
            const curvePathIDsBuffer = renderer.meshes[0].segmentCurvePathIDs;
            const curveNormalsBuffer = renderer.meshes[0].segmentCurveNormals;

            gl.useProgram(curveProgram.program);
            gl.bindBuffer(gl.ARRAY_BUFFER, renderContext.quadPositionsBuffer);
            gl.vertexAttribPointer(attributes.aQuadPosition, 2, gl.FLOAT, false, 0, 0);
            gl.bindBuffer(gl.ARRAY_BUFFER, curveVertexPositionsBuffer);
            gl.vertexAttribPointer(attributes.aLeftPosition,
                                   2,
                                   gl.FLOAT,
                                   false,
                                   FLOAT32_SIZE * 6,
                                   0);
            gl.vertexAttribPointer(attributes.aControlPointPosition,
                                   2,
                                   gl.FLOAT,
                                   false,
                                   FLOAT32_SIZE * 6,
                                   FLOAT32_SIZE * 2);
            gl.vertexAttribPointer(attributes.aRightPosition,
                                   2,
                                   gl.FLOAT,
                                   false,
                                   FLOAT32_SIZE * 6,
                                   FLOAT32_SIZE * 4);
            gl.bindBuffer(gl.ARRAY_BUFFER, curvePathIDsBuffer);
            gl.vertexAttribPointer(attributes.aPathID, 1, gl.UNSIGNED_SHORT, false, 0, 0);

            gl.enableVertexAttribArray(attributes.aQuadPosition);
            gl.enableVertexAttribArray(attributes.aLeftPosition);
            gl.enableVertexAttribArray(attributes.aControlPointPosition);
            gl.enableVertexAttribArray(attributes.aRightPosition);
            gl.enableVertexAttribArray(attributes.aPathID);

            renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
            renderContext.instancedArraysExt
                         .vertexAttribDivisorANGLE(attributes.aControlPointPosition, 1);
            renderContext.instancedArraysExt
                         .vertexAttribDivisorANGLE(attributes.aRightPosition, 1);
            renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);

            if (renderer.meshData[0].segmentCurveNormals.byteLength > 0) {
                gl.bindBuffer(gl.ARRAY_BUFFER, curveNormalsBuffer);
                gl.vertexAttribPointer(attributes.aNormalAngles,
                                       3,
                                       gl.FLOAT,
                                       false,
                                       FLOAT32_SIZE * 3,
                                       0);

                gl.enableVertexAttribArray(attributes.aNormalAngles);

                renderContext.instancedArraysExt
                             .vertexAttribDivisorANGLE(attributes.aNormalAngles, 1);
            }

            gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, renderContext.quadElementsBuffer);

            renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);

            this.curveVAOs[programName] = vao;
        }
    }
}

/// Switches between the mesh-based MCAA and ECAA depending on whether stem darkening is enabled.
///
/// FIXME(pcwalton): Share textures and FBOs between the two strategies.
export class AdaptiveMonochromeXCAAStrategy implements AntialiasingStrategy {
    private mcaaStrategy: MCAAStrategy;
    private ecaaStrategy: ECAAStrategy;

    get directRenderingMode(): DirectRenderingMode {
        return 'none';
    }

    get passCount(): number {
        return 1;
    }

    constructor(level: number, subpixelAA: SubpixelAAType) {
        this.mcaaStrategy = new MCAAStrategy(level, subpixelAA);
        this.ecaaStrategy = new ECAAStrategy(level, subpixelAA);
    }

    init(renderer: Renderer): void {
        this.mcaaStrategy.init(renderer);
        this.ecaaStrategy.init(renderer);
    }

    attachMeshes(renderer: Renderer): void {
        this.mcaaStrategy.attachMeshes(renderer);
        this.ecaaStrategy.attachMeshes(renderer);
    }

    setFramebufferSize(renderer: Renderer): void {
        this.mcaaStrategy.setFramebufferSize(renderer);
        this.ecaaStrategy.setFramebufferSize(renderer);
    }

    get transform(): glmatrix.mat4 {
        return this.mcaaStrategy.transform;
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
        if (glmatrix.vec2.equals(renderer.emboldenAmount, [0.0, 0.0]) &&
            renderer.usesSTTransform) {
            return this.mcaaStrategy;
        }

        return this.ecaaStrategy;
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
