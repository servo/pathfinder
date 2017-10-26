// pathfinder/client/src/xcaa-strategy.ts
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
import {PathfinderShaderProgram} from './shader-loader';
import {computeStemDarkeningAmount} from './text';
import {assert, FLOAT32_SIZE, lerp, UINT32_SIZE, unwrapNull} from './utils';
import {RenderContext} from './view';

interface FastEdgeVAOs {
    upper: WebGLVertexArrayObject;
    lower: WebGLVertexArrayObject;
}

type Direction = 'upper' | 'lower';

const DIRECTIONS: Direction[] = ['upper', 'lower'];

export abstract class XCAAStrategy extends AntialiasingStrategy {
    abstract readonly directRenderingMode: DirectRenderingMode;

    protected directTexture: WebGLTexture;
    protected aaDepthTexture: WebGLTexture;

    protected pathBoundsBufferTexture: PathfinderBufferTexture;

    protected supersampledFramebufferSize: glmatrix.vec2;
    protected destFramebufferSize: glmatrix.vec2;

    protected subpixelAA: SubpixelAAType;

    protected resolveVAO: WebGLVertexArrayObject;
    protected aaAlphaTexture: WebGLTexture;

    private directFramebuffer: WebGLFramebuffer;
    private aaFramebuffer: WebGLFramebuffer;

    constructor(level: number, subpixelAA: SubpixelAAType) {
        super();

        this.subpixelAA = subpixelAA;

        this.supersampledFramebufferSize = glmatrix.vec2.create();
        this.destFramebufferSize = glmatrix.vec2.create();
    }

    init(renderer: Renderer) {
        super.init(renderer);
    }

    attachMeshes(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        this.createEdgeDetectVAO(renderContext);
        this.createResolveVAO(renderer);
    }

    setFramebufferSize(renderer: Renderer) {
        const renderContext = renderer.renderContext;

        this.destFramebufferSize = glmatrix.vec2.clone(renderer.destAllocatedSize);
        glmatrix.vec2.mul(this.supersampledFramebufferSize,
                          this.destFramebufferSize,
                          this.supersampleScale);

        this.initDirectFramebuffer(renderer);
        this.initAAAlphaFramebuffer(renderer);
        this.initEdgeDetectFramebuffer(renderer);
        renderContext.gl.bindFramebuffer(renderContext.gl.FRAMEBUFFER, null);
    }

    prepare(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const usedSize = this.supersampledUsedSize(renderer);
        gl.bindFramebuffer(gl.FRAMEBUFFER, this.directFramebuffer);
        gl.viewport(0,
                    0,
                    this.supersampledFramebufferSize[0],
                    this.supersampledFramebufferSize[1]);
        gl.scissor(0, 0, usedSize[0], usedSize[1]);
        gl.enable(gl.SCISSOR_TEST);

        // Clear out the color and depth textures.
        gl.clearColor(1.0, 1.0, 1.0, 1.0);
        gl.clearDepth(0.0);
        gl.depthMask(true);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    }

    antialias(renderer: Renderer) {
        // Detect edges if necessary.
        this.detectEdgesIfNecessary(renderer);

        // Set up antialiasing.
        this.prepareAA(renderer);

        // Clear.
        this.clear(renderer);
    }

    resolve(renderer: Renderer) {
        // Resolve the antialiasing.
        this.resolveAA(renderer);
    }

    get transform(): glmatrix.mat4 {
        return glmatrix.mat4.create();
    }

    protected initDirectFramebuffer(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        let textureFormat;
        if (this.directRenderingMode === 'pathID')
            textureFormat = gl.RGBA;
        else
            textureFormat = renderContext.colorAlphaFormat;

        this.directTexture = createFramebufferColorTexture(gl,
                                                           this.destFramebufferSize,
                                                           textureFormat);
        this.directFramebuffer = createFramebuffer(renderContext.gl,
                                                   this.directTexture,
                                                   this.directDepthTexture);
    }

    protected supersampledUsedSize(renderer: Renderer): glmatrix.vec2 {
        const usedSize = glmatrix.vec2.create();
        glmatrix.vec2.mul(usedSize, renderer.destUsedSize, this.supersampleScale);
        return usedSize;
    }

    protected prepareAA(renderer: Renderer): void {
        // Set state for antialiasing.
        const renderContext = renderer.renderContext;
        const usedSize = this.supersampledUsedSize(renderer);
        renderContext.gl.bindFramebuffer(renderContext.gl.FRAMEBUFFER, this.aaFramebuffer);
        renderContext.gl.viewport(0,
                                  0,
                                  this.supersampledFramebufferSize[0],
                                  this.supersampledFramebufferSize[1]);
        renderContext.gl.scissor(0, 0, usedSize[0], usedSize[1]);
        renderContext.gl.enable(renderContext.gl.SCISSOR_TEST);

        this.createPathBoundsBufferTexture(renderer);
    }

    protected setAAState(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        const usedSize = this.supersampledUsedSize(renderer);
        renderContext.gl.bindFramebuffer(renderContext.gl.FRAMEBUFFER, this.aaFramebuffer);
        renderContext.gl.viewport(0,
                                  0,
                                  this.supersampledFramebufferSize[0],
                                  this.supersampledFramebufferSize[1]);
        renderContext.gl.scissor(0, 0, usedSize[0], usedSize[1]);
        renderContext.gl.enable(renderContext.gl.SCISSOR_TEST);

        this.setAADepthState(renderer);
    }

    protected setAAUniforms(renderer: Renderer, uniforms: UniformMap) {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        renderer.setTransformSTUniform(uniforms, 0);
        gl.uniform2i(uniforms.uFramebufferSize,
                     this.supersampledFramebufferSize[0],
                     this.supersampledFramebufferSize[1]);
        renderer.pathTransformBufferTextures[0].bind(renderContext.gl, uniforms, 0);
        this.pathBoundsBufferTexture.bind(renderContext.gl, uniforms, 1);
        renderer.setHintsUniform(uniforms);
    }

    protected resolveAA(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        // Set state for ECAA resolve.
        const usedSize = renderer.destUsedSize;
        gl.bindFramebuffer(gl.FRAMEBUFFER, renderer.destFramebuffer);
        gl.viewport(0, 0, this.destFramebufferSize[0], this.destFramebufferSize[1]);
        gl.scissor(0, 0, usedSize[0], usedSize[1]);
        gl.enable(gl.SCISSOR_TEST);
        gl.disable(gl.DEPTH_TEST);
        gl.disable(gl.BLEND);

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
        gl.drawElements(renderContext.gl.TRIANGLES, 6, renderContext.gl.UNSIGNED_BYTE, 0);
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected abstract clear(renderer: Renderer): void;
    protected abstract getResolveProgram(renderContext: RenderContext): PathfinderShaderProgram;
    protected abstract initEdgeDetectFramebuffer(renderer: Renderer): void;
    protected abstract createEdgeDetectVAO(renderContext: RenderContext): void;
    protected abstract detectEdgesIfNecessary(renderer: Renderer): void;
    protected abstract setAADepthState(renderer: Renderer): void;
    protected abstract clearForResolve(renderer: Renderer): void;

    private initAAAlphaFramebuffer(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        this.aaAlphaTexture = unwrapNull(renderContext.gl.createTexture());
        renderContext.gl.activeTexture(renderContext.gl.TEXTURE0);
        renderContext.gl.bindTexture(renderContext.gl.TEXTURE_2D, this.aaAlphaTexture);
        renderContext.gl.texImage2D(renderContext.gl.TEXTURE_2D,
                                    0,
                                    renderContext.gl.RGB,
                                    this.supersampledFramebufferSize[0],
                                    this.supersampledFramebufferSize[1],
                                    0,
                                    renderContext.gl.RGB,
                                    renderContext.textureHalfFloatExt.HALF_FLOAT_OES,
                                    null);
        setTextureParameters(renderContext.gl, renderContext.gl.NEAREST);

        this.aaDepthTexture = createFramebufferDepthTexture(renderContext.gl,
                                                            this.supersampledFramebufferSize);

        this.aaFramebuffer = createFramebuffer(renderContext.gl,
                                               this.aaAlphaTexture,
                                               this.aaDepthTexture);
    }

    private createPathBoundsBufferTexture(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        const pathBounds = renderer.pathBoundingRects(0);
        this.pathBoundsBufferTexture = new PathfinderBufferTexture(renderContext.gl,
                                                                   'uPathBounds');
        this.pathBoundsBufferTexture.upload(renderContext.gl, pathBounds);
    }

    private createResolveVAO(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        this.resolveVAO = renderContext.vertexArrayObjectExt.createVertexArrayOES();
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.resolveVAO);

        const resolveProgram = this.getResolveProgram(renderContext);
        renderContext.gl.useProgram(resolveProgram.program);
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

    attachMeshes(renderer: Renderer) {
        super.attachMeshes(renderer);

        this.createCoverVAO(renderer);
        this.createLineVAOs(renderer);
        this.createCurveVAOs(renderer);
    }

    antialias(renderer: Renderer) {
        super.antialias(renderer);

        // Conservatively cover.
        this.cover(renderer);

        // Antialias.
        this.antialiasLines(renderer);
        this.antialiasCurves(renderer);
    }

    protected prepareAA(renderer: Renderer): void {
        super.prepareAA(renderer);

        this.setCoverDepthState(renderer);

        const renderContext = renderer.renderContext;
        renderContext.gl.blendEquation(renderContext.gl.FUNC_ADD);
        renderContext.gl.blendFunc(renderContext.gl.ONE, renderContext.gl.ONE);
        renderContext.gl.enable(renderContext.gl.BLEND);

        this.clear(renderer);
    }

    protected setCoverDepthState(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        renderContext.gl.disable(renderContext.gl.DEPTH_TEST);
    }

    private createCoverVAO(renderer: Renderer) {
        const renderContext = renderer.renderContext;

        this.coverVAO = renderContext.vertexArrayObjectExt.createVertexArrayOES();
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.coverVAO);

        const coverProgram = renderContext.shaderPrograms.mcaaCover;
        const attributes = coverProgram.attributes;
        renderContext.gl.useProgram(coverProgram.program);
        renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER,
                                    renderContext.quadPositionsBuffer);
        renderContext.gl.vertexAttribPointer(attributes.aQuadPosition,
                                             2,
                                             renderContext.gl.FLOAT,
                                             false,
                                             0,
                                             0);
        renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER,
                                    renderer.meshes[0].edgeBoundingBoxVertexPositions);
        renderContext.gl.vertexAttribPointer(attributes.aUpperLeftPosition,
                                             2,
                                             renderContext.gl.FLOAT,
                                             false,
                                             FLOAT32_SIZE * 4,
                                             0);
        renderContext.gl.vertexAttribPointer(attributes.aLowerRightPosition,
                                             2,
                                             renderContext.gl.FLOAT,
                                             false,
                                             FLOAT32_SIZE * 4,
                                             FLOAT32_SIZE * 2);
        renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER,
                                    renderer.meshes[0].edgeBoundingBoxPathIDs);
        renderContext.gl.vertexAttribPointer(attributes.aPathID,
                                             1,
                                             renderContext.gl.UNSIGNED_SHORT,
                                             false,
                                             0,
                                             0);
        renderContext.gl.enableVertexAttribArray(attributes.aQuadPosition);
        renderContext.gl.enableVertexAttribArray(attributes.aUpperLeftPosition);
        renderContext.gl.enableVertexAttribArray(attributes.aLowerRightPosition);
        renderContext.gl.enableVertexAttribArray(attributes.aPathID);
        renderContext.instancedArraysExt
                     .vertexAttribDivisorANGLE(attributes.aUpperLeftPosition, 1);
        renderContext.instancedArraysExt
                     .vertexAttribDivisorANGLE(attributes.aLowerRightPosition, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);
        renderContext.gl.bindBuffer(renderContext.gl.ELEMENT_ARRAY_BUFFER,
                                    renderContext.quadElementsBuffer);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private createLineVAOs(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        const vaos: Partial<FastEdgeVAOs> = {};
        const lineProgram = renderContext.shaderPrograms.mcaaLine;
        const attributes = lineProgram.attributes;

        for (const direction of DIRECTIONS) {
            const vao = renderContext.vertexArrayObjectExt.createVertexArrayOES();
            renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

            const lineVertexPositionsBuffer = {
                lower: renderer.meshes[0].edgeLowerLineVertexPositions,
                upper: renderer.meshes[0].edgeUpperLineVertexPositions,
            }[direction];
            const linePathIDsBuffer = {
                lower: renderer.meshes[0].edgeLowerLinePathIDs,
                upper: renderer.meshes[0].edgeUpperLinePathIDs,
            }[direction];

            renderContext.gl.useProgram(lineProgram.program);
            renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER,
                                        renderContext.quadPositionsBuffer);
            renderContext.gl.vertexAttribPointer(attributes.aQuadPosition,
                                                 2,
                                                 renderContext.gl.FLOAT,
                                                 false,
                                                 0,
                                                 0);
            renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER, lineVertexPositionsBuffer);
            renderContext.gl.vertexAttribPointer(attributes.aLeftPosition,
                                                 2,
                                                 renderContext.gl.FLOAT,
                                                 false,
                                                 FLOAT32_SIZE * 4,
                                                 0);
            renderContext.gl.vertexAttribPointer(attributes.aRightPosition,
                                                 2,
                                                 renderContext.gl.FLOAT,
                                                 false,
                                                 FLOAT32_SIZE * 4,
                                                 FLOAT32_SIZE * 2);
            renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER, linePathIDsBuffer);
            renderContext.gl.vertexAttribPointer(attributes.aPathID,
                                                 1,
                                                 renderContext.gl.UNSIGNED_SHORT,
                                                 false,
                                                 0,
                                                 0);

            renderContext.gl.enableVertexAttribArray(attributes.aQuadPosition);
            renderContext.gl.enableVertexAttribArray(attributes.aLeftPosition);
            renderContext.gl.enableVertexAttribArray(attributes.aRightPosition);
            renderContext.gl.enableVertexAttribArray(attributes.aPathID);

            renderContext.instancedArraysExt
                         .vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
            renderContext.instancedArraysExt
                         .vertexAttribDivisorANGLE(attributes.aRightPosition, 1);
            renderContext.instancedArraysExt
                         .vertexAttribDivisorANGLE(attributes.aPathID, 1);

            renderContext.gl.bindBuffer(renderContext.gl.ELEMENT_ARRAY_BUFFER,
                                        renderContext.quadElementsBuffer);

            vaos[direction] = vao;
        }

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.lineVAOs = vaos as FastEdgeVAOs;
    }

    private createCurveVAOs(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        const vaos: Partial<FastEdgeVAOs> = {};
        const curveProgram = renderContext.shaderPrograms.mcaaCurve;
        const attributes = curveProgram.attributes;

        for (const direction of DIRECTIONS) {
            const vao = renderContext.vertexArrayObjectExt.createVertexArrayOES();
            renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

            const curveVertexPositionsBuffer = {
                lower: renderer.meshes[0].edgeLowerCurveVertexPositions,
                upper: renderer.meshes[0].edgeUpperCurveVertexPositions,
            }[direction];
            const curvePathIDsBuffer = {
                lower: renderer.meshes[0].edgeLowerCurvePathIDs,
                upper: renderer.meshes[0].edgeUpperCurvePathIDs,
            }[direction];

            renderContext.gl.useProgram(curveProgram.program);
            renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER,
                                        renderContext.quadPositionsBuffer);
            renderContext.gl.vertexAttribPointer(attributes.aQuadPosition,
                                                 2,
                                                 renderContext.gl.FLOAT,
                                                 false,
                                                 0,
                                                 0);
            renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER, curveVertexPositionsBuffer);
            renderContext.gl.vertexAttribPointer(attributes.aLeftPosition,
                                                 2,
                                                 renderContext.gl.FLOAT,
                                                 false,
                                                 FLOAT32_SIZE * 6,
                                                 0);
            renderContext.gl.vertexAttribPointer(attributes.aControlPointPosition,
                                                 2,
                                                 renderContext.gl.FLOAT,
                                                 false,
                                                 FLOAT32_SIZE * 6,
                                                 FLOAT32_SIZE * 2);
            renderContext.gl.vertexAttribPointer(attributes.aRightPosition,
                                                 2,
                                                 renderContext.gl.FLOAT,
                                                 false,
                                                 FLOAT32_SIZE * 6,
                                                 FLOAT32_SIZE * 4);
            renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER, curvePathIDsBuffer);
            renderContext.gl.vertexAttribPointer(attributes.aPathID,
                                                 1,
                                                 renderContext.gl.UNSIGNED_SHORT,
                                                 false,
                                                 0,
                                                 0);

            renderContext.gl.enableVertexAttribArray(attributes.aQuadPosition);
            renderContext.gl.enableVertexAttribArray(attributes.aLeftPosition);
            renderContext.gl.enableVertexAttribArray(attributes.aControlPointPosition);
            renderContext.gl.enableVertexAttribArray(attributes.aRightPosition);
            renderContext.gl.enableVertexAttribArray(attributes.aPathID);

            renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
            renderContext.instancedArraysExt
                         .vertexAttribDivisorANGLE(attributes.aControlPointPosition, 1);
            renderContext.instancedArraysExt
                         .vertexAttribDivisorANGLE(attributes.aRightPosition, 1);
            renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);

            renderContext.gl.bindBuffer(renderContext.gl.ELEMENT_ARRAY_BUFFER,
                                        renderContext.quadElementsBuffer);

            vaos[direction] = vao;
        }

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.curveVAOs = vaos as FastEdgeVAOs;
    }

    private cover(renderer: Renderer): void {
        // Conservatively cover.
        const renderContext = renderer.renderContext;
        const coverProgram = renderContext.shaderPrograms.mcaaCover;
        renderContext.gl.useProgram(coverProgram.program);
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.coverVAO);
        this.setAAUniforms(renderer, coverProgram.uniforms);
        renderContext.instancedArraysExt
                     .drawElementsInstancedANGLE(renderContext.gl.TRIANGLES,
                                                 6,
                                                 renderContext.gl.UNSIGNED_BYTE,
                                                 0,
                                                 renderer.meshData[0].bQuadCount);
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private setBlendModeForAA(renderer: Renderer, direction: 'upper' | 'lower') {
        const renderContext = renderer.renderContext;
        renderContext.gl.blendEquation(renderContext.gl.FUNC_ADD);
        renderContext.gl.blendFunc(renderContext.gl.ONE, renderContext.gl.ONE);
        renderContext.gl.enable(renderContext.gl.BLEND);
    }

    private antialiasLines(renderer: Renderer) {
        const renderContext = renderer.renderContext;

        this.setAAState(renderer);

        const lineProgram = renderContext.shaderPrograms.mcaaLine;

        renderContext.gl.useProgram(lineProgram.program);
        const uniforms = lineProgram.uniforms;
        this.setAAUniforms(renderer, uniforms);

        for (const direction of DIRECTIONS) {
            const vao = this.lineVAOs[direction];
            renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

            this.setBlendModeForAA(renderer, direction);
            renderContext.gl.uniform1i(uniforms.uWinding, direction === 'upper' ? 1 : 0);

            const count = {
                lower: renderer.meshData[0].edgeLowerLineCount,
                upper: renderer.meshData[0].edgeUpperLineCount,
            }[direction];
            renderContext.instancedArraysExt
                         .drawElementsInstancedANGLE(renderContext.gl.TRIANGLES,
                                                     6,
                                                     renderContext.gl.UNSIGNED_BYTE,
                                                     0,
                                                     count);
        }

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private antialiasCurves(renderer: Renderer) {
        const renderContext = renderer.renderContext;

        this.setAAState(renderer);

        const curveProgram = renderContext.shaderPrograms.mcaaCurve;

        renderContext.gl.useProgram(curveProgram.program);
        const uniforms = curveProgram.uniforms;
        this.setAAUniforms(renderer, uniforms);

        for (const direction of DIRECTIONS) {
            const vao = this.curveVAOs[direction];
            renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

            this.setBlendModeForAA(renderer, direction);
            renderContext.gl.uniform1i(uniforms.uWinding, direction === 'upper' ? 1 : 0);

            const count = {
                lower: renderer.meshData[0].edgeLowerCurveCount,
                upper: renderer.meshData[0].edgeUpperCurveCount,
            }[direction];
            renderContext.instancedArraysExt
                         .drawElementsInstancedANGLE(renderContext.gl.TRIANGLES,
                                                     6,
                                                     renderContext.gl.UNSIGNED_BYTE,
                                                     0,
                                                     count);
        }

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }
}

export class ECAAStrategy extends XCAAStrategy {
    private lineVAO: WebGLVertexArrayObject;
    private curveVAO: WebGLVertexArrayObject;

    get directRenderingMode(): DirectRenderingMode {
        return 'none';
    }

    attachMeshes(renderer: Renderer) {
        super.attachMeshes(renderer);

        this.createLineVAO(renderer);
        this.createCurveVAO(renderer);
    }

    antialias(renderer: Renderer) {
        super.antialias(renderer);

        // Antialias.
        this.antialiasLines(renderer);
        this.antialiasCurves(renderer);
    }

    protected setAAUniforms(renderer: Renderer, uniforms: UniformMap) {
        super.setAAUniforms(renderer, uniforms);

        const renderContext = renderer.renderContext;
        const emboldenAmount = renderer.emboldenAmount;
        renderContext.gl.uniform2f(uniforms.uEmboldenAmount, emboldenAmount[0], emboldenAmount[1]);
    }

    protected getResolveProgram(renderContext: RenderContext): PathfinderShaderProgram {
        if (this.subpixelAA !== 'none')
            return renderContext.shaderPrograms.xcaaMonoSubpixelResolve;
        return renderContext.shaderPrograms.xcaaMonoResolve;
    }

    protected initEdgeDetectFramebuffer(renderer: Renderer) {}

    protected createEdgeDetectVAO(renderContext: RenderContext) {}

    protected detectEdgesIfNecessary(renderer: Renderer) {}

    protected clear(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        renderContext.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        renderContext.gl.clearDepth(0.0);
        renderContext.gl.clear(renderContext.gl.COLOR_BUFFER_BIT |
                               renderContext.gl.DEPTH_BUFFER_BIT);
    }

    protected setAADepthState(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        renderContext.gl.disable(renderContext.gl.DEPTH_TEST);
    }

    protected clearForResolve(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        renderContext.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        renderContext.gl.clear(renderContext.gl.COLOR_BUFFER_BIT);
    }

    private setBlendModeForAA(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        renderContext.gl.blendEquation(renderContext.gl.FUNC_ADD);
        renderContext.gl.blendFunc(renderContext.gl.ONE, renderContext.gl.ONE);
        renderContext.gl.enable(renderContext.gl.BLEND);
    }

    private createLineVAO(renderer: Renderer) {
        const renderContext = renderer.renderContext;

        const lineProgram = renderContext.shaderPrograms.ecaaLine;
        const attributes = lineProgram.attributes;

        const vao = renderContext.vertexArrayObjectExt.createVertexArrayOES();
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

        const lineVertexPositionsBuffer = renderer.meshes[0].segmentLines;
        const linePathIDsBuffer = renderer.meshes[0].segmentLinePathIDs;
        const lineNormalsBuffer = renderer.meshes[0].segmentLineNormals;

        renderContext.gl.useProgram(lineProgram.program);
        renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER,
                                    renderContext.quadPositionsBuffer);
        renderContext.gl.vertexAttribPointer(attributes.aQuadPosition,
                                             2,
                                             renderContext.gl.FLOAT,
                                             false,
                                             0,
                                             0);
        renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER, lineVertexPositionsBuffer);
        renderContext.gl.vertexAttribPointer(attributes.aLeftPosition,
                                             2,
                                             renderContext.gl.FLOAT,
                                             false,
                                             FLOAT32_SIZE * 4,
                                             0);
        renderContext.gl.vertexAttribPointer(attributes.aRightPosition,
                                             2,
                                             renderContext.gl.FLOAT,
                                             false,
                                             FLOAT32_SIZE * 4,
                                             FLOAT32_SIZE * 2);
        renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER, linePathIDsBuffer);
        renderContext.gl.vertexAttribPointer(attributes.aPathID,
                                             1,
                                             renderContext.gl.UNSIGNED_SHORT,
                                             false,
                                             0,
                                             0);
        renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER, lineNormalsBuffer);
        renderContext.gl.vertexAttribPointer(attributes.aLeftNormalAngle,
                                             1,
                                             renderContext.gl.FLOAT,
                                             false,
                                             FLOAT32_SIZE * 2,
                                             0);
        renderContext.gl.vertexAttribPointer(attributes.aRightNormalAngle,
                                             1,
                                             renderContext.gl.FLOAT,
                                             false,
                                             FLOAT32_SIZE * 2,
                                             FLOAT32_SIZE);

        renderContext.gl.enableVertexAttribArray(attributes.aQuadPosition);
        renderContext.gl.enableVertexAttribArray(attributes.aLeftPosition);
        renderContext.gl.enableVertexAttribArray(attributes.aRightPosition);
        renderContext.gl.enableVertexAttribArray(attributes.aPathID);
        renderContext.gl.enableVertexAttribArray(attributes.aLeftNormalAngle);
        renderContext.gl.enableVertexAttribArray(attributes.aRightNormalAngle);

        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightPosition, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftNormalAngle, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightNormalAngle, 1);

        renderContext.gl.bindBuffer(renderContext.gl.ELEMENT_ARRAY_BUFFER,
                                    renderContext.quadElementsBuffer);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.lineVAO = vao;
    }

    private createCurveVAO(renderer: Renderer) {
        const renderContext = renderer.renderContext;

        const curveProgram = renderContext.shaderPrograms.ecaaCurve;
        const attributes = curveProgram.attributes;

        const vao = renderContext.vertexArrayObjectExt.createVertexArrayOES();
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

        const curveVertexPositionsBuffer = renderer.meshes[0].segmentCurves;
        const curvePathIDsBuffer = renderer.meshes[0].segmentCurvePathIDs;
        const curveNormalsBuffer = renderer.meshes[0].segmentCurveNormals;

        renderContext.gl.useProgram(curveProgram.program);
        renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER,
                                    renderContext.quadPositionsBuffer);
        renderContext.gl.vertexAttribPointer(attributes.aQuadPosition,
                                             2,
                                             renderContext.gl.FLOAT,
                                             false,
                                             0,
                                             0);
        renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER, curveVertexPositionsBuffer);
        renderContext.gl.vertexAttribPointer(attributes.aLeftPosition,
                                             2,
                                             renderContext.gl.FLOAT,
                                             false,
                                             FLOAT32_SIZE * 6,
                                             0);
        renderContext.gl.vertexAttribPointer(attributes.aControlPointPosition,
                                             2,
                                             renderContext.gl.FLOAT,
                                             false,
                                             FLOAT32_SIZE * 6,
                                             FLOAT32_SIZE * 2);
        renderContext.gl.vertexAttribPointer(attributes.aRightPosition,
                                             2,
                                             renderContext.gl.FLOAT,
                                             false,
                                             FLOAT32_SIZE * 6,
                                             FLOAT32_SIZE * 4);
        renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER, curvePathIDsBuffer);
        renderContext.gl.vertexAttribPointer(attributes.aPathID,
                                             1,
                                             renderContext.gl.UNSIGNED_SHORT,
                                             false,
                                             0,
                                             0);
        renderContext.gl.bindBuffer(renderContext.gl.ARRAY_BUFFER, curveNormalsBuffer);
        renderContext.gl.vertexAttribPointer(attributes.aNormalAngles,
                                             3,
                                             renderContext.gl.FLOAT,
                                             false,
                                             FLOAT32_SIZE * 3,
                                             0);

        renderContext.gl.enableVertexAttribArray(attributes.aQuadPosition);
        renderContext.gl.enableVertexAttribArray(attributes.aLeftPosition);
        renderContext.gl.enableVertexAttribArray(attributes.aControlPointPosition);
        renderContext.gl.enableVertexAttribArray(attributes.aRightPosition);
        renderContext.gl.enableVertexAttribArray(attributes.aPathID);
        renderContext.gl.enableVertexAttribArray(attributes.aNormalAngles);

        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
        renderContext.instancedArraysExt
                     .vertexAttribDivisorANGLE(attributes.aControlPointPosition, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightPosition, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);
        renderContext.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aNormalAngles, 1);

        renderContext.gl.bindBuffer(renderContext.gl.ELEMENT_ARRAY_BUFFER,
                                    renderContext.quadElementsBuffer);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.curveVAO = vao;
    }

    private antialiasLines(renderer: Renderer) {
        const renderContext = renderer.renderContext;

        this.setAAState(renderer);

        const lineProgram = renderContext.shaderPrograms.ecaaLine;

        renderContext.gl.useProgram(lineProgram.program);
        const uniforms = lineProgram.uniforms;
        this.setAAUniforms(renderer, uniforms);

        const vao = this.lineVAO;
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

        this.setBlendModeForAA(renderer);

        const count = renderer.meshData[0].segmentLineCount;
        renderContext.instancedArraysExt.drawElementsInstancedANGLE(renderContext.gl.TRIANGLES,
                                                                    6,
                                                                    renderContext.gl.UNSIGNED_BYTE,
                                                                    0,
                                                                    count);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private antialiasCurves(renderer: Renderer) {
        const renderContext = renderer.renderContext;

        this.setAAState(renderer);

        const curveProgram = renderContext.shaderPrograms.ecaaCurve;

        renderContext.gl.useProgram(curveProgram.program);
        const uniforms = curveProgram.uniforms;
        this.setAAUniforms(renderer, uniforms);

        const vao = this.curveVAO;
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(vao);

        this.setBlendModeForAA(renderer);

        const count = renderer.meshData[0].segmentCurveCount;
        renderContext.instancedArraysExt.drawElementsInstancedANGLE(renderContext.gl.TRIANGLES,
                                                                    6,
                                                                    renderContext.gl.UNSIGNED_BYTE,
                                                                    0,
                                                                    count);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }
}

export class MCAAMonochromeStrategy extends MCAAStrategy {
    protected getResolveProgram(renderContext: RenderContext): PathfinderShaderProgram {
        if (this.subpixelAA !== 'none')
            return renderContext.shaderPrograms.xcaaMonoSubpixelResolve;
        return renderContext.shaderPrograms.xcaaMonoResolve;
    }

    protected initEdgeDetectFramebuffer(renderer: Renderer) {}

    protected createEdgeDetectVAO(renderContext: RenderContext) {}

    protected detectEdgesIfNecessary(renderer: Renderer) {}

    protected clear(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        renderContext.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        renderContext.gl.clearDepth(0.0);
        renderContext.gl.clear(renderContext.gl.COLOR_BUFFER_BIT |
                               renderContext.gl.DEPTH_BUFFER_BIT);
    }

    protected setAADepthState(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        renderContext.gl.disable(renderContext.gl.DEPTH_TEST);
    }

    protected clearForResolve(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        renderContext.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        renderContext.gl.clear(renderContext.gl.COLOR_BUFFER_BIT);
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

    prepare(renderer: Renderer): void {
        this.getAppropriateStrategy(renderer).prepare(renderer);
    }

    antialias(renderer: Renderer): void {
        this.getAppropriateStrategy(renderer).antialias(renderer);
    }

    resolve(renderer: Renderer): void {
        this.getAppropriateStrategy(renderer).resolve(renderer);
    }

    private getAppropriateStrategy(renderer: Renderer): AntialiasingStrategy {
        if (glmatrix.vec2.equals(renderer.emboldenAmount, [0.0, 0.0]))
            return this.mcaaStrategy;
        return this.ecaaStrategy;
    }
}

export class MCAAMulticolorStrategy extends MCAAStrategy {
    private _directDepthTexture: WebGLTexture;

    private edgeDetectFramebuffer: WebGLFramebuffer;
    private edgeDetectVAO: WebGLVertexArrayObject;
    private edgePathIDTexture: WebGLTexture;

    protected getResolveProgram(renderContext: RenderContext): PathfinderShaderProgram {
        return renderContext.shaderPrograms.xcaaMultiResolve;
    }

    protected initDirectFramebuffer(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        this._directDepthTexture = createFramebufferDepthTexture(renderContext.gl,
                                                                 this.supersampledFramebufferSize);
        super.initDirectFramebuffer(renderer);
    }

    protected initEdgeDetectFramebuffer(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        this.edgePathIDTexture = createFramebufferColorTexture(renderContext.gl,
                                                               this.supersampledFramebufferSize,
                                                               renderContext.gl.RGBA);
        this.edgeDetectFramebuffer = createFramebuffer(renderContext.gl,
                                                       this.edgePathIDTexture,
                                                       this.aaDepthTexture);
    }

    protected createEdgeDetectVAO(renderContext: RenderContext) {
        this.edgeDetectVAO = renderContext.vertexArrayObjectExt.createVertexArrayOES();
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.edgeDetectVAO);

        const edgeDetectProgram = renderContext.shaderPrograms.xcaaEdgeDetect;
        renderContext.gl.useProgram(edgeDetectProgram.program);
        renderContext.initQuadVAO(edgeDetectProgram.attributes);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected detectEdgesIfNecessary(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        // Set state for edge detection.
        const edgeDetectProgram = renderContext.shaderPrograms.xcaaEdgeDetect;
        gl.bindFramebuffer(gl.FRAMEBUFFER, this.edgeDetectFramebuffer);
        gl.viewport(0,
                    0,
                    this.supersampledFramebufferSize[0],
                    this.supersampledFramebufferSize[1]);

        gl.depthMask(true);
        gl.depthFunc(renderContext.gl.ALWAYS);
        gl.enable(renderContext.gl.DEPTH_TEST);
        gl.disable(renderContext.gl.BLEND);

        gl.clearDepth(0.0);
        gl.clearColor(0.0, 0.0, 0.0, 0.0);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

        // Perform edge detection.
        gl.useProgram(edgeDetectProgram.program);
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.edgeDetectVAO);
        renderer.setFramebufferSizeUniform(edgeDetectProgram.uniforms);
        renderer.setTransformSTAndTexScaleUniformsForDest(edgeDetectProgram.uniforms);
        renderer.setPathColorsUniform(0, edgeDetectProgram.uniforms, 0);
        gl.activeTexture(renderContext.gl.TEXTURE1);
        gl.bindTexture(renderContext.gl.TEXTURE_2D, this.directTexture);
        gl.uniform1i(edgeDetectProgram.uniforms.uPathID, 1);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, renderContext.quadElementsBuffer);
        gl.drawElements(gl.TRIANGLES, 6, gl.UNSIGNED_BYTE, 0);

        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected resolveAA(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        // Set state for ECAA resolve.
        const usedSize = renderer.destUsedSize;
        renderContext.gl.bindFramebuffer(renderContext.gl.FRAMEBUFFER, renderer.destFramebuffer);
        renderContext.gl.viewport(0, 0, this.destFramebufferSize[0], this.destFramebufferSize[1]);
        renderContext.gl.scissor(0, 0, usedSize[0], usedSize[1]);
        renderContext.gl.enable(renderContext.gl.SCISSOR_TEST);
        renderContext.gl.disable(renderContext.gl.DEPTH_TEST);
        renderContext.gl.disable(renderContext.gl.BLEND);

        // Resolve.
        const resolveProgram = this.getResolveProgram(renderContext);
        gl.useProgram(resolveProgram.program);
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(this.resolveVAO);
        gl.uniform2i(resolveProgram.uniforms.uFramebufferSize,
                     this.destFramebufferSize[0],
                     this.destFramebufferSize[1]);
        renderContext.gl.activeTexture(renderContext.gl.TEXTURE0);
        renderContext.gl.bindTexture(renderContext.gl.TEXTURE_2D, this.aaAlphaTexture);
        renderContext.gl.uniform1i(resolveProgram.uniforms.uAAAlpha, 0);
        renderContext.gl.uniform2i(resolveProgram.uniforms.uAAAlphaDimensions,
                                   this.supersampledFramebufferSize[0],
                                   this.supersampledFramebufferSize[1]);
        renderContext.gl.activeTexture(renderContext.gl.TEXTURE1);
        renderContext.gl.bindTexture(renderContext.gl.TEXTURE_2D, this.edgePathIDTexture);
        renderContext.gl.uniform1i(resolveProgram.uniforms.uBGFGPathID, 1);
        renderer.setPathColorsUniform(0, resolveProgram.uniforms, 2);
        renderer.setTransformSTAndTexScaleUniformsForDest(resolveProgram.uniforms);
        renderContext.gl.drawElements(renderContext.gl.TRIANGLES,
                                      6,
                                      renderContext.gl.UNSIGNED_BYTE,
                                      0);
        renderContext.vertexArrayObjectExt.bindVertexArrayOES(null);

        renderContext.gl.bindFramebuffer(renderContext.gl.FRAMEBUFFER, null);
    }

    protected setCoverDepthState(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        renderContext.gl.depthMask(false);
        renderContext.gl.depthFunc(renderContext.gl.EQUAL);
        renderContext.gl.enable(renderContext.gl.DEPTH_TEST);
    }

    protected clear(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        renderContext.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        renderContext.gl.clear(renderContext.gl.COLOR_BUFFER_BIT);
    }

    protected setAADepthState(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        renderContext.gl.depthMask(false);
        renderContext.gl.depthFunc(renderContext.gl.EQUAL);
        renderContext.gl.enable(renderContext.gl.DEPTH_TEST);
    }

    protected clearForResolve(renderer: Renderer) {}

    get directRenderingMode(): DirectRenderingMode {
        return 'pathID';
    }

    protected get directDepthTexture(): WebGLTexture {
        return this._directDepthTexture;
    }
}
