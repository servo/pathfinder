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

    protected abstract get usesDilationTransforms(): boolean;

    protected pathBoundsBufferTexture: PathfinderBufferTexture;

    protected supersampledFramebufferSize: glmatrix.vec2;
    protected destFramebufferSize: glmatrix.vec2;

    protected subpixelAA: SubpixelAAType;

    protected resolveVAO: WebGLVertexArrayObject;

    protected aaAlphaTexture: WebGLTexture;
    protected aaDepthTexture: WebGLTexture;
    protected aaFramebuffer: WebGLFramebuffer;

    protected renderTargetColorTextures: WebGLTexture[];
    protected renderTargetDepthTextures: WebGLTexture[];
    protected renderTargetFramebuffers: WebGLFramebuffer[];

    constructor(level: number, subpixelAA: SubpixelAAType) {
        super();

        this.subpixelAA = subpixelAA;

        this.supersampledFramebufferSize = glmatrix.vec2.create();
        this.destFramebufferSize = glmatrix.vec2.create();

        this.renderTargetColorTextures = [];
        this.renderTargetDepthTextures = [];
        this.renderTargetFramebuffers = [];
    }

    init(renderer: Renderer): void {
        super.init(renderer);
    }

    attachMeshes(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        this.createResolveVAO(renderer);
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

    prepareToRenderObject(renderer: Renderer, objectIndex: number): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        this.initResolveFramebufferForObject(renderer, objectIndex);

        const usedSize = this.supersampledUsedSize(renderer);
        gl.scissor(0, 0, usedSize[0], usedSize[1]);
        gl.enable(gl.SCISSOR_TEST);

        // Clear out the color and depth textures.
        gl.clearColor(1.0, 1.0, 1.0, 1.0);
        gl.clearDepth(0.0);
        gl.depthMask(true);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    }

    finishDirectlyRenderingObject(renderer: Renderer, objectIndex: number): void {
        // TODO(pcwalton)
    }

    antialiasObject(renderer: Renderer, objectIndex: number): void {
        // Perform early preparations.
        this.createPathBoundsBufferTextureForObject(renderer, objectIndex);

        // Mask edges if necessary.
        this.maskEdgesOfObjectIfNecessary(renderer, objectIndex);

        // Set up antialiasing.
        this.prepareAA(renderer);

        // Clear.
        this.clearForAA(renderer);
    }

    resolveAAForObject(renderer: Renderer, objectIndex: number): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        // Set state for ECAA resolve.
        const usedSize = renderer.destUsedSize;
        gl.scissor(0, 0, usedSize[0], usedSize[1]);
        gl.enable(gl.SCISSOR_TEST);
        this.setDepthAndBlendModeForResolve(renderContext);

        // Clear out the resolve buffer, if necessary.
        this.clearForResolve(renderer);

        // Resolve.
        const resolveProgram = this.getResolveProgram(renderContext);
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

        // Resolve render target if necessary.
        if (!renderer.usesIntermediateRenderTargets)
            return;

        const compositingOperation = renderer.compositingOperationForObject(objectIndex);
        if (compositingOperation == null)
            return;

        gl.bindFramebuffer(gl.FRAMEBUFFER, renderer.destFramebuffer);
        gl.viewport(0, 0, this.destFramebufferSize[0], this.destFramebufferSize[1]);
        gl.disable(gl.DEPTH_TEST);
        gl.disable(gl.BLEND);

        compositingOperation.composite(renderer, objectIndex, this.renderTargetColorTextures);
    }

    resolve(renderer: Renderer): void {}

    get transform(): glmatrix.mat4 {
        return glmatrix.mat4.create();
    }

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
            renderer.setTransformUniform(uniforms, 0);

        gl.uniform2i(uniforms.uFramebufferSize,
                     this.supersampledFramebufferSize[0],
                     this.supersampledFramebufferSize[1]);
        renderer.pathTransformBufferTextures[0].ext.bind(gl, uniforms, 0);
        renderer.pathTransformBufferTextures[0].st.bind(gl, uniforms, 1);
        this.pathBoundsBufferTexture.bind(gl, uniforms, 2);
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
    protected abstract getResolveProgram(renderContext: RenderContext): PathfinderShaderProgram;
    protected abstract maskEdgesOfObjectIfNecessary(renderer: Renderer, objectIndex: number): void;
    protected abstract setAADepthState(renderer: Renderer): void;
    protected abstract clearForResolve(renderer: Renderer): void;

    private initResolveFramebufferForObject(renderer: Renderer, objectIndex: number): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        if (renderer.usesIntermediateRenderTargets &&
            (renderer.renderTaskTypeForObject(objectIndex) === 'clip' ||
             renderer.compositingOperationForObject(objectIndex) != null)) {
            if (this.renderTargetColorTextures[objectIndex] == null) {
                this.renderTargetColorTextures[objectIndex] =
                    createFramebufferColorTexture(gl,
                                                  this.supersampledFramebufferSize,
                                                  renderContext.colorAlphaFormat);
            }
            if (this.renderTargetDepthTextures[objectIndex] == null) {
                this.renderTargetDepthTextures[objectIndex] =
                    createFramebufferDepthTexture(gl, this.supersampledFramebufferSize);
            }
            if (this.renderTargetFramebuffers[objectIndex] == null) {
                this.renderTargetFramebuffers[objectIndex] =
                    createFramebuffer(gl,
                                      this.renderTargetColorTextures[objectIndex],
                                      this.renderTargetDepthTextures[objectIndex]);
            }
            gl.bindFramebuffer(gl.FRAMEBUFFER, this.renderTargetFramebuffers[objectIndex]);
        } else {
            gl.bindFramebuffer(gl.FRAMEBUFFER, renderer.destFramebuffer);
        }

        gl.viewport(0, 0, this.destFramebufferSize[0], this.destFramebufferSize[1]);
        gl.disable(gl.SCISSOR_TEST);
    }

    private initAAAlphaFramebuffer(renderer: Renderer): void {
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

    private createPathBoundsBufferTextureForObject(renderer: Renderer, objectIndex: number): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const pathBounds = renderer.pathBoundingRects(objectIndex);
        this.pathBoundsBufferTexture = new PathfinderBufferTexture(gl, 'uPathBounds');
        this.pathBoundsBufferTexture.upload(gl, pathBounds);
    }

    private createResolveVAO(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        this.resolveVAO = renderContext.vertexArrayObjectExt.createVertexArrayOES();
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.resolveVAO);

        const resolveProgram = this.getResolveProgram(renderContext);
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

export abstract class MCAAStrategy extends XCAAStrategy {
    private coverVAO: WebGLVertexArrayObject;
    private lineVAOs: FastEdgeVAOs;
    private curveVAOs: FastEdgeVAOs;

    attachMeshes(renderer: Renderer): void {
        super.attachMeshes(renderer);

        this.createCoverVAO(renderer);
        this.createLineVAOs(renderer);
        this.createCurveVAOs(renderer);
    }

    antialiasObject(renderer: Renderer, objectIndex: number): void {
        super.antialiasObject(renderer, objectIndex);

        // Conservatively cover.
        this.coverObject(renderer, objectIndex);

        // Antialias.
        this.antialiasLinesOfObject(renderer, objectIndex);
        this.antialiasCurvesOfObject(renderer, objectIndex);
    }

    protected prepareAA(renderer: Renderer): void {
        super.prepareAA(renderer);

        this.setCoverDepthState(renderer);

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.blendEquation(gl.FUNC_ADD);
        gl.blendFunc(gl.ONE, gl.ONE);
        gl.enable(gl.BLEND);

        this.clearForAA(renderer);
    }

    protected setCoverDepthState(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        renderContext.gl.disable(renderContext.gl.DEPTH_TEST);
    }

    protected antialiasLinesOfObjectWithProgram(renderer: Renderer,
                                                objectIndex: number,
                                                lineProgram: PathfinderShaderProgram):
                                                void {
        if (renderer.meshData == null)
            return;

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const pathRange = renderer.pathRangeForObject(objectIndex);
        const meshIndex = renderer.meshIndexForObject(objectIndex);

        this.initLineVAOsForObject(renderer, objectIndex);

        gl.useProgram(lineProgram.program);
        const uniforms = lineProgram.uniforms;
        this.setAAUniforms(renderer, uniforms, objectIndex);

        for (const direction of DIRECTIONS) {
            const vao = this.lineVAOs[direction];
            renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

            this.setBlendModeForAA(renderer, direction);
            gl.uniform1i(uniforms.uWinding, direction === 'upper' ? 1 : 0);

            const indexRanges = {
                lower: renderer.meshData[meshIndex].edgeLowerLineIndexRanges,
                upper: renderer.meshData[meshIndex].edgeUpperLineIndexRanges,
            }[direction];
            const count = calculateCountFromIndexRanges(pathRange, indexRanges);

            renderContext.instancedArraysExt
                         .drawElementsInstancedANGLE(gl.TRIANGLES, 6, gl.UNSIGNED_BYTE, 0, count);
        }

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected antialiasCurvesOfObjectWithProgram(renderer: Renderer,
                                                 objectIndex: number,
                                                 curveProgram: PathfinderShaderProgram):
                                                 void {
        if (renderer.meshData == null)
            return;

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const pathRange = renderer.pathRangeForObject(objectIndex);
        const meshIndex = renderer.meshIndexForObject(objectIndex);

        this.initCurveVAOsForObject(renderer, objectIndex);

        gl.useProgram(curveProgram.program);
        const uniforms = curveProgram.uniforms;
        this.setAAUniforms(renderer, uniforms, objectIndex);

        for (const direction of DIRECTIONS) {
            const vao = this.curveVAOs[direction];
            renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

            this.setBlendModeForAA(renderer, direction);
            gl.uniform1i(uniforms.uWinding, direction === 'upper' ? 1 : 0);

            const indexRanges = {
                lower: renderer.meshData[meshIndex].edgeLowerCurveIndexRanges,
                upper: renderer.meshData[meshIndex].edgeUpperCurveIndexRanges,
            }[direction];
            const count = calculateCountFromIndexRanges(pathRange, indexRanges);

            renderContext.instancedArraysExt
                         .drawElementsInstancedANGLE(gl.TRIANGLES, 6, gl.UNSIGNED_BYTE, 0, count);
        }

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private createCoverVAO(renderer: Renderer): void {
        this.coverVAO = renderer.renderContext.vertexArrayObjectExt.createVertexArrayOES();
    }

    private initCoverVAOForObject(renderer: Renderer, objectIndex: number): void {
        if (renderer.meshes == null || renderer.meshData == null)
            return;

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const pathRange = renderer.pathRangeForObject(objectIndex);
        const meshIndex = renderer.meshIndexForObject(objectIndex);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.coverVAO);

        const bQuadRanges = renderer.meshData[meshIndex].bQuadPathRanges;
        const offset = calculateStartFromIndexRanges(pathRange, bQuadRanges);

        const coverProgram = renderContext.shaderPrograms.mcaaCover;
        const attributes = coverProgram.attributes;
        gl.useProgram(coverProgram.program);
        gl.bindBuffer(gl.ARRAY_BUFFER, renderContext.quadPositionsBuffer);
        gl.vertexAttribPointer(attributes.aQuadPosition, 2, gl.FLOAT, false, 0, 0);
        gl.bindBuffer(gl.ARRAY_BUFFER, renderer.meshes[meshIndex].edgeBoundingBoxVertexPositions);
        gl.vertexAttribPointer(attributes.aUpperLeftPosition,
                               2,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE * 4,
                               FLOAT32_SIZE * 4 * offset);
        gl.vertexAttribPointer(attributes.aLowerRightPosition,
                               2,
                               gl.FLOAT,
                               false,
                               FLOAT32_SIZE * 4,
                               FLOAT32_SIZE * 4 * offset + FLOAT32_SIZE * 2);
        gl.bindBuffer(gl.ARRAY_BUFFER, renderer.meshes[meshIndex].edgeBoundingBoxPathIDs);
        gl.vertexAttribPointer(attributes.aPathID,
                               1,
                               gl.UNSIGNED_SHORT,
                               false,
                               0,
                               UINT16_SIZE * offset);
        gl.enableVertexAttribArray(attributes.aQuadPosition);
        gl.enableVertexAttribArray(attributes.aUpperLeftPosition);
        gl.enableVertexAttribArray(attributes.aLowerRightPosition);
        gl.enableVertexAttribArray(attributes.aPathID);
        renderContext.instancedArraysExt
                     .vertexAttribDivisorANGLE(attributes.aUpperLeftPosition, 1);
        renderContext.instancedArraysExt
                     .vertexAttribDivisorANGLE(attributes.aLowerRightPosition, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, renderContext.quadElementsBuffer);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private createLineVAOs(renderer: Renderer): void {
        const renderContext = renderer.renderContext;

        const vaos: Partial<FastEdgeVAOs> = {};
        for (const direction of DIRECTIONS)
            vaos[direction] = renderContext.vertexArrayObjectExt.createVertexArrayOES();
        this.lineVAOs = vaos as FastEdgeVAOs;
    }

    private initLineVAOsForObject(renderer: Renderer, objectIndex: number): void {
        if (renderer.meshes == null || renderer.meshData == null)
            return;

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const pathRange = renderer.pathRangeForObject(objectIndex);
        const meshIndex = renderer.meshIndexForObject(objectIndex);

        const lineProgram = renderContext.shaderPrograms.mcaaLine;
        const attributes = lineProgram.attributes;

        for (const direction of DIRECTIONS) {
            const vao = this.lineVAOs[direction];
            renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

            const lineVertexPositionsBuffer = {
                lower: renderer.meshes[meshIndex].edgeLowerLineVertexPositions,
                upper: renderer.meshes[meshIndex].edgeUpperLineVertexPositions,
            }[direction];
            const linePathIDsBuffer = {
                lower: renderer.meshes[meshIndex].edgeLowerLinePathIDs,
                upper: renderer.meshes[meshIndex].edgeUpperLinePathIDs,
            }[direction];
            const lineIndexRanges = {
                lower: renderer.meshData[meshIndex].edgeLowerLineIndexRanges,
                upper: renderer.meshData[meshIndex].edgeUpperLineIndexRanges,
            }[direction];

            const offset = calculateStartFromIndexRanges(pathRange, lineIndexRanges);

            gl.useProgram(lineProgram.program);
            gl.bindBuffer(gl.ARRAY_BUFFER, renderContext.quadPositionsBuffer);
            gl.vertexAttribPointer(attributes.aQuadPosition, 2, gl.FLOAT, false, 0, 0);
            gl.bindBuffer(gl.ARRAY_BUFFER, lineVertexPositionsBuffer);
            gl.vertexAttribPointer(attributes.aLeftPosition,
                                   2,
                                   gl.FLOAT,
                                   false,
                                   FLOAT32_SIZE * 4,
                                   offset * FLOAT32_SIZE * 4);
            gl.vertexAttribPointer(attributes.aRightPosition,
                                   2,
                                   gl.FLOAT,
                                   false,
                                   FLOAT32_SIZE * 4,
                                   offset * FLOAT32_SIZE * 4 + FLOAT32_SIZE * 2);
            gl.bindBuffer(gl.ARRAY_BUFFER, linePathIDsBuffer);
            gl.vertexAttribPointer(attributes.aPathID,
                                   1,
                                   gl.UNSIGNED_SHORT,
                                   false,
                                   0,
                                   offset * UINT16_SIZE);

            gl.enableVertexAttribArray(attributes.aQuadPosition);
            gl.enableVertexAttribArray(attributes.aLeftPosition);
            gl.enableVertexAttribArray(attributes.aRightPosition);
            gl.enableVertexAttribArray(attributes.aPathID);

            renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
            renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightPosition,
                                                                      1);
            renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);

            gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, renderContext.quadElementsBuffer);
        }

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private createCurveVAOs(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const vaos: Partial<FastEdgeVAOs> = {};
        for (const direction of DIRECTIONS)
            vaos[direction] = renderContext.vertexArrayObjectExt.createVertexArrayOES();
        this.curveVAOs = vaos as FastEdgeVAOs;
    }

    private initCurveVAOsForObject(renderer: Renderer, objectIndex: number): void {
        if (renderer.meshes == null || renderer.meshData == null)
            return;

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const pathRange = renderer.pathRangeForObject(objectIndex);
        const meshIndex = renderer.meshIndexForObject(objectIndex);

        const curveProgram = renderContext.shaderPrograms.mcaaCurve;
        const attributes = curveProgram.attributes;

        for (const direction of DIRECTIONS) {
            const vao = this.curveVAOs[direction];
            renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

            const curveVertexPositionsBuffer = {
                lower: renderer.meshes[meshIndex].edgeLowerCurveVertexPositions,
                upper: renderer.meshes[meshIndex].edgeUpperCurveVertexPositions,
            }[direction];
            const curvePathIDsBuffer = {
                lower: renderer.meshes[meshIndex].edgeLowerCurvePathIDs,
                upper: renderer.meshes[meshIndex].edgeUpperCurvePathIDs,
            }[direction];
            const curveIndexRanges = {
                lower: renderer.meshData[meshIndex].edgeLowerCurveIndexRanges,
                upper: renderer.meshData[meshIndex].edgeUpperCurveIndexRanges,
            }[direction];

            const offset = calculateStartFromIndexRanges(pathRange, curveIndexRanges);

            gl.useProgram(curveProgram.program);
            gl.bindBuffer(gl.ARRAY_BUFFER, renderContext.quadPositionsBuffer);
            gl.vertexAttribPointer(attributes.aQuadPosition, 2, gl.FLOAT, false, 0, 0);
            gl.bindBuffer(gl.ARRAY_BUFFER, curveVertexPositionsBuffer);
            gl.vertexAttribPointer(attributes.aLeftPosition,
                                   2,
                                   gl.FLOAT,
                                   false,
                                   FLOAT32_SIZE * 6,
                                   FLOAT32_SIZE * 6 * offset);
            gl.vertexAttribPointer(attributes.aControlPointPosition,
                                   2,
                                   gl.FLOAT,
                                   false,
                                   FLOAT32_SIZE * 6,
                                   FLOAT32_SIZE * 6 * offset + FLOAT32_SIZE * 2);
            gl.vertexAttribPointer(attributes.aRightPosition,
                                   2,
                                   gl.FLOAT,
                                   false,
                                   FLOAT32_SIZE * 6,
                                   FLOAT32_SIZE * 6 * offset + FLOAT32_SIZE * 4);
            gl.bindBuffer(gl.ARRAY_BUFFER, curvePathIDsBuffer);
            gl.vertexAttribPointer(attributes.aPathID,
                                   1,
                                   gl.UNSIGNED_SHORT,
                                   false,
                                   0,
                                   UINT16_SIZE * offset);

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

            gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, renderContext.quadElementsBuffer);
        }

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private coverObject(renderer: Renderer, objectIndex: number): void {
        if (renderer.meshes == null || renderer.meshData == null)
            return;

        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const pathRange = renderer.pathRangeForObject(objectIndex);
        const meshIndex = renderer.meshIndexForObject(objectIndex);

        this.initCoverVAOForObject(renderer, objectIndex);

        // Conservatively cover.
        const coverProgram = renderContext.shaderPrograms.mcaaCover;
        gl.useProgram(coverProgram.program);
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.coverVAO);
        this.setAAUniforms(renderer, coverProgram.uniforms, objectIndex);

        const bQuadRange = renderer.meshData[meshIndex].bQuadPathRanges;
        const count = calculateCountFromIndexRanges(pathRange, bQuadRange);

        renderContext.instancedArraysExt
                     .drawElementsInstancedANGLE(gl.TRIANGLES, 6, gl.UNSIGNED_BYTE, 0, count);
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private setBlendModeForAA(renderer: Renderer, direction: 'upper' | 'lower'): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.blendEquation(gl.FUNC_ADD);
        gl.blendFunc(gl.ONE, gl.ONE);
        gl.enable(gl.BLEND);
    }

    private antialiasLinesOfObject(renderer: Renderer, objectIndex: number): void {
        const renderContext = renderer.renderContext;
        this.setAAState(renderer);

        const lineProgram = renderContext.shaderPrograms.mcaaLine;
        renderContext.gl.useProgram(lineProgram.program);

        // FIXME(pcwalton): Refactor.
        this.antialiasLinesOfObjectWithProgram(renderer, objectIndex, lineProgram);
    }

    private antialiasCurvesOfObject(renderer: Renderer, objectIndex: number): void {
        const renderContext = renderer.renderContext;
        this.setAAState(renderer);

        const curveProgram = renderContext.shaderPrograms.mcaaCurve;
        renderContext.gl.useProgram(curveProgram.program);

        this.antialiasCurvesOfObjectWithProgram(renderer, objectIndex, curveProgram);
    }
}

export abstract class ECAAStrategy extends XCAAStrategy {
    protected abstract get lineShaderProgramNames(): Array<keyof ShaderMap<void>>;
    protected abstract get curveShaderProgramNames(): Array<keyof ShaderMap<void>>;

    private lineVAOs: Partial<ShaderMap<WebGLVertexArrayObject>>;
    private curveVAOs: Partial<ShaderMap<WebGLVertexArrayObject>>;

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
        this.antialiasCurvesOfObjectWithProgram(renderer,
                                                objectIndex,
                                                this.curveShaderProgramNames[0]);
    }

    protected setAAUniforms(renderer: Renderer, uniforms: UniformMap, objectIndex: number): void {
        super.setAAUniforms(renderer, uniforms, objectIndex);
        renderer.setEmboldenAmountUniform(objectIndex, uniforms);
    }

    protected getResolveProgram(renderContext: RenderContext): PathfinderShaderProgram {
        if (this.subpixelAA !== 'none')
            return renderContext.shaderPrograms.xcaaMonoSubpixelResolve;
        return renderContext.shaderPrograms.xcaaMonoResolve;
    }

    protected maskEdgesOfObjectIfNecessary(renderer: Renderer, objectIndex: number): void {}

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

    protected antialiasCurvesOfObjectWithProgram(renderer: Renderer,
                                                 objectIndex: number,
                                                 programName: keyof ShaderMap<void>):
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

export class ECAAMonochromeStrategy extends ECAAStrategy {
    protected get usesDilationTransforms(): boolean {
        return false;
    }

    protected get lineShaderProgramNames(): Array<keyof ShaderMap<void>> {
        return ['ecaaLine'];
    }

    protected get curveShaderProgramNames(): Array<keyof ShaderMap<void>> {
        return ['ecaaCurve'];
    }
}

export class MCAAMonochromeStrategy extends MCAAStrategy {
    protected get usesDilationTransforms(): boolean {
        return true;
    }

    protected getResolveProgram(renderContext: RenderContext): PathfinderShaderProgram {
        if (this.subpixelAA !== 'none')
            return renderContext.shaderPrograms.xcaaMonoSubpixelResolve;
        return renderContext.shaderPrograms.xcaaMonoResolve;
    }

    protected maskEdgesOfObjectIfNecessary(renderer: Renderer, objectIndex: number): void {}

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

    get directRenderingMode(): DirectRenderingMode {
        return 'none';
    }
}

/// Switches between the mesh-based MCAA and ECAA depending on whether stem darkening is enabled.
///
/// FIXME(pcwalton): Share textures and FBOs between the two strategies.
export class AdaptiveMonochromeXCAAStrategy implements AntialiasingStrategy {
    private mcaaStrategy: MCAAMonochromeStrategy;
    private ecaaStrategy: ECAAStrategy;

    get directRenderingMode(): DirectRenderingMode {
        return 'none';
    }

    constructor(level: number, subpixelAA: SubpixelAAType) {
        this.mcaaStrategy = new MCAAMonochromeStrategy(level, subpixelAA);
        this.ecaaStrategy = new ECAAMonochromeStrategy(level, subpixelAA);
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

    resolve(renderer: Renderer): void {
        this.getAppropriateStrategy(renderer).resolve(renderer);
    }

    private getAppropriateStrategy(renderer: Renderer): AntialiasingStrategy {
        if (glmatrix.vec2.equals(renderer.emboldenAmount, [0.0, 0.0]) &&
            renderer.usesSTTransform) {
            return this.mcaaStrategy;
        }

        return this.ecaaStrategy;
    }
}

export class ECAAMulticolorStrategy extends ECAAStrategy {
    protected get usesDilationTransforms(): boolean {
        return false;
    }

    protected get lineShaderProgramNames(): Array<keyof ShaderMap<void>> {
        return ['ecaaLine', 'xcaaMultiEdgeMaskLine'];
    }

    protected get curveShaderProgramNames(): Array<keyof ShaderMap<void>> {
        return ['ecaaCurve', 'xcaaMultiEdgeMaskCurve'];
    }

    private edgeMaskVAO: WebGLVertexArrayObject;

    bindEdgeDepthTexture(gl: WebGLRenderingContext, uniforms: UniformMap, textureUnit: number):
                         void {
        gl.activeTexture(gl.TEXTURE0 + textureUnit);
        gl.bindTexture(gl.TEXTURE_2D, this.aaDepthTexture);
        gl.uniform1i(uniforms.uEdgeDepth, textureUnit);
        gl.activeTexture(gl.TEXTURE0 + textureUnit + 1);
        gl.bindTexture(gl.TEXTURE_2D, this.aaAlphaTexture);
        gl.uniform1i(uniforms.uEdgeAlpha, textureUnit);
    }

    protected getResolveProgram(renderContext: RenderContext): PathfinderShaderProgram {
        return renderContext.shaderPrograms.xcaaMultiResolve;
    }

    protected initDirectFramebufferIfNecessary(renderer: Renderer): void {}

    protected maskEdgesOfObjectIfNecessary(renderer: Renderer, objectIndex: number): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        // Set state for edge masking.
        gl.bindFramebuffer(gl.FRAMEBUFFER, this.aaFramebuffer);
        gl.viewport(0,
                    0,
                    this.supersampledFramebufferSize[0],
                    this.supersampledFramebufferSize[1]);

        gl.colorMask(true, true, true, true);
        gl.depthMask(true);
        gl.depthFunc(gl.GREATER);
        gl.enable(gl.DEPTH_TEST);
        gl.disable(gl.BLEND);

        gl.clearDepth(0.0);
        gl.clearColor(0.0, 0.0, 0.0, 0.0);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

        // Perform edge masking.
        gl.colorMask(false, false, false, false);
        this.antialiasLinesOfObjectWithProgram(renderer, objectIndex, 'xcaaMultiEdgeMaskLine');
        this.antialiasCurvesOfObjectWithProgram(renderer, objectIndex, 'xcaaMultiEdgeMaskCurve');

        gl.colorMask(true, true, true, true);
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected setCoverDepthState(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.depthMask(false);
        gl.depthFunc(gl.EQUAL);
        gl.enable(gl.DEPTH_TEST);
    }

    protected clearForAA(renderer: Renderer): void {
        /*const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.clearColor(0.0, 0.0, 0.0, 0.0);
        gl.clear(gl.COLOR_BUFFER_BIT);*/
    }

    protected setAADepthState(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.depthMask(false);
        gl.depthFunc(gl.EQUAL);
        gl.enable(gl.DEPTH_TEST);
    }

    protected setDepthAndBlendModeForResolve(renderContext: RenderContext): void {
        const gl = renderContext.gl;

        gl.depthMask(true);
        gl.depthFunc(gl.GREATER);
        gl.enable(gl.DEPTH_TEST);

        gl.blendEquation(gl.FUNC_ADD);
        gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE);
        gl.enable(gl.BLEND);
    }

    protected clearForResolve(renderer: Renderer): void {}

    protected setAdditionalStateForResolveIfNecessary(renderer: Renderer,
                                                      resolveProgram: PathfinderShaderProgram,
                                                      firstFreeTextureUnit: number):
                                                      void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.activeTexture(gl.TEXTURE0 + firstFreeTextureUnit + 0);
        gl.bindTexture(gl.TEXTURE_2D, this.aaDepthTexture);
        gl.uniform1i(resolveProgram.uniforms.uAADepth, firstFreeTextureUnit + 0);

        renderer.setPathColorsUniform(0, resolveProgram.uniforms, firstFreeTextureUnit + 1);
    }

    get directRenderingMode(): DirectRenderingMode {
        return 'color-depth';
    }

    protected get directDepthTexture(): WebGLTexture | null {
        return null;
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
