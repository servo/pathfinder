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

import {AntialiasingStrategy, SubpixelAAType} from './aa-strategy';
import PathfinderBufferTexture from './buffer-texture';
import {createFramebuffer, createFramebufferColorTexture} from './gl-utils';
import {createFramebufferDepthTexture, setTextureParameters, UniformMap} from './gl-utils';
import {WebGLVertexArrayObject} from './gl-utils';
import {B_QUAD_LOWER_INDICES_OFFSET, B_QUAD_SIZE, B_QUAD_UPPER_INDICES_OFFSET} from './meshes';
import {PathfinderShaderProgram} from './shader-loader';
import {computeStemDarkeningAmount} from './text';
import {FLOAT32_SIZE, lerp, UINT32_SIZE, unwrapNull} from './utils';
import {Renderer} from './view';

interface FastEdgeVAOs {
    upper: WebGLVertexArrayObject;
    lower: WebGLVertexArrayObject;
}

type Direction = 'upper' | 'lower';

const DIRECTIONS: Direction[] = ['upper', 'lower'];

export abstract class XCAAStrategy extends AntialiasingStrategy {
    abstract shouldRenderDirect: boolean;

    protected directColorTexture: WebGLTexture;
    protected directPathIDTexture: WebGLTexture;
    protected aaDepthTexture: WebGLTexture;

    protected pathBoundsBufferTexture: PathfinderBufferTexture;

    protected supersampledFramebufferSize: glmatrix.vec2;
    protected destFramebufferSize: glmatrix.vec2;

    protected subpixelAA: SubpixelAAType;

    private directFramebuffer: WebGLFramebuffer;
    private aaAlphaTexture: WebGLTexture;
    private aaFramebuffer: WebGLFramebuffer;
    private resolveVAO: WebGLVertexArrayObject;

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
        this.createEdgeDetectVAO(renderer);
        this.createResolveVAO(renderer);
    }

    setFramebufferSize(renderer: Renderer) {
        this.destFramebufferSize = glmatrix.vec2.clone(renderer.destAllocatedSize);
        glmatrix.vec2.mul(this.supersampledFramebufferSize,
                          this.destFramebufferSize,
                          this.supersampleScale);

        this.initDirectFramebuffer(renderer);
        this.initEdgeDetectFramebuffer(renderer);
        this.initAAAlphaFramebuffer(renderer);
        renderer.gl.bindFramebuffer(renderer.gl.FRAMEBUFFER, null);
    }

    prepare(renderer: Renderer) {
        const usedSize = this.supersampledUsedSize(renderer);
        renderer.gl.bindFramebuffer(renderer.gl.FRAMEBUFFER, this.directFramebuffer);
        renderer.gl.viewport(0,
                             0,
                             this.supersampledFramebufferSize[0],
                             this.supersampledFramebufferSize[1]);
        renderer.gl.scissor(0, 0, usedSize[0], usedSize[1]);
        renderer.gl.enable(renderer.gl.SCISSOR_TEST);

        // Clear out the color and depth textures.
        renderer.drawBuffersExt.drawBuffersWEBGL([
            renderer.drawBuffersExt.COLOR_ATTACHMENT0_WEBGL,
            renderer.gl.NONE,
        ]);
        renderer.gl.clearColor(1.0, 1.0, 1.0, 1.0);
        renderer.gl.clearDepth(0.0);
        renderer.gl.depthMask(true);
        renderer.gl.clear(renderer.gl.COLOR_BUFFER_BIT | renderer.gl.DEPTH_BUFFER_BIT);

        // Clear out the path ID texture.
        renderer.drawBuffersExt.drawBuffersWEBGL([
            renderer.gl.NONE,
            renderer.drawBuffersExt.COLOR_ATTACHMENT1_WEBGL,
        ]);
        renderer.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        renderer.gl.clear(renderer.gl.COLOR_BUFFER_BIT);

        // Render to both textures.
        renderer.drawBuffersExt.drawBuffersWEBGL([
            renderer.drawBuffersExt.COLOR_ATTACHMENT0_WEBGL,
            renderer.drawBuffersExt.COLOR_ATTACHMENT1_WEBGL,
        ]);
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
        this.directColorTexture = createFramebufferColorTexture(renderer.gl,
                                                                this.destFramebufferSize,
                                                                renderer.colorAlphaFormat);
        this.directPathIDTexture = createFramebufferColorTexture(renderer.gl,
                                                                 this.destFramebufferSize,
                                                                 renderer.colorAlphaFormat);
        this.directFramebuffer =
            createFramebuffer(renderer.gl,
                              renderer.drawBuffersExt,
                              [this.directColorTexture, this.directPathIDTexture],
                              this.directDepthTexture);
    }

    protected setResolveDepthState(renderer: Renderer): void {
        renderer.gl.disable(renderer.gl.DEPTH_TEST);
    }

    protected supersampledUsedSize(renderer: Renderer): glmatrix.vec2 {
        const usedSize = glmatrix.vec2.create();
        glmatrix.vec2.mul(usedSize, renderer.destUsedSize, this.supersampleScale);
        return usedSize;
    }

    protected prepareAA(renderer: Renderer): void {
        // Set state for antialiasing.
        const usedSize = this.supersampledUsedSize(renderer);
        renderer.gl.bindFramebuffer(renderer.gl.FRAMEBUFFER, this.aaFramebuffer);
        renderer.gl.viewport(0,
                             0,
                             this.supersampledFramebufferSize[0],
                             this.supersampledFramebufferSize[1]);
        renderer.gl.scissor(0, 0, usedSize[0], usedSize[1]);
        renderer.gl.enable(renderer.gl.SCISSOR_TEST);

        this.createPathBoundsBufferTexture(renderer);
    }

    protected setAAState(renderer: Renderer) {
        const usedSize = this.supersampledUsedSize(renderer);
        renderer.gl.bindFramebuffer(renderer.gl.FRAMEBUFFER, this.aaFramebuffer);
        renderer.gl.viewport(0,
                             0,
                             this.supersampledFramebufferSize[0],
                             this.supersampledFramebufferSize[1]);
        renderer.gl.scissor(0, 0, usedSize[0], usedSize[1]);
        renderer.gl.enable(renderer.gl.SCISSOR_TEST);

        this.setAADepthState(renderer);
    }

    protected setAAUniforms(renderer: Renderer, uniforms: UniformMap) {
        renderer.setTransformSTUniform(uniforms, 0);
        renderer.setFramebufferSizeUniform(uniforms);
        renderer.pathTransformBufferTextures[0].bind(renderer.gl, uniforms, 0);
        this.pathBoundsBufferTexture.bind(renderer.gl, uniforms, 1);
        renderer.setHintsUniform(uniforms);
    }

    protected abstract clear(renderer: Renderer): void;
    protected abstract getResolveProgram(renderer: Renderer): PathfinderShaderProgram;
    protected abstract initEdgeDetectFramebuffer(renderer: Renderer): void;
    protected abstract createEdgeDetectVAO(renderer: Renderer): void;
    protected abstract detectEdgesIfNecessary(renderer: Renderer): void;
    protected abstract setAADepthState(renderer: Renderer): void;
    protected abstract clearForResolve(renderer: Renderer): void;
    protected abstract setResolveUniforms(renderer: Renderer, program: PathfinderShaderProgram):
                                          void;

    private initAAAlphaFramebuffer(renderer: Renderer) {
        this.aaAlphaTexture = unwrapNull(renderer.gl.createTexture());
        renderer.gl.activeTexture(renderer.gl.TEXTURE0);
        renderer.gl.bindTexture(renderer.gl.TEXTURE_2D, this.aaAlphaTexture);
        renderer.gl.texImage2D(renderer.gl.TEXTURE_2D,
                               0,
                               renderer.gl.RGB,
                               this.supersampledFramebufferSize[0],
                               this.supersampledFramebufferSize[1],
                               0,
                               renderer.gl.RGB,
                               renderer.textureHalfFloatExt.HALF_FLOAT_OES,
                               null);
        setTextureParameters(renderer.gl, renderer.gl.NEAREST);

        this.aaDepthTexture = createFramebufferDepthTexture(renderer.gl,
                                                            this.supersampledFramebufferSize);

        this.aaFramebuffer = createFramebuffer(renderer.gl,
                                               renderer.drawBuffersExt,
                                               [this.aaAlphaTexture],
                                               this.aaDepthTexture);
    }

    private createPathBoundsBufferTexture(renderer: Renderer) {
        const pathBounds = renderer.pathBoundingRects(0);
        this.pathBoundsBufferTexture = new PathfinderBufferTexture(renderer.gl, 'uPathBounds');
        this.pathBoundsBufferTexture.upload(renderer.gl, pathBounds);
    }

    private createResolveVAO(renderer: Renderer) {
        this.resolveVAO = renderer.vertexArrayObjectExt.createVertexArrayOES();
        renderer.vertexArrayObjectExt.bindVertexArrayOES(this.resolveVAO);

        const resolveProgram = this.getResolveProgram(renderer);
        renderer.gl.useProgram(resolveProgram.program);
        renderer.initQuadVAO(resolveProgram.attributes);

        renderer.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private resolveAA(renderer: Renderer) {
        // Set state for ECAA resolve.
        const usedSize = renderer.destUsedSize;
        renderer.gl.bindFramebuffer(renderer.gl.FRAMEBUFFER, renderer.destFramebuffer);
        renderer.gl.viewport(0, 0, this.destFramebufferSize[0], this.destFramebufferSize[1]);
        renderer.gl.scissor(0, 0, usedSize[0], usedSize[1]);
        renderer.gl.enable(renderer.gl.SCISSOR_TEST);
        this.setResolveDepthState(renderer);
        renderer.gl.disable(renderer.gl.BLEND);
        if (renderer.destFramebuffer != null)
            renderer.drawBuffersExt.drawBuffersWEBGL([renderer.drawBuffersExt.COLOR_ATTACHMENT0_WEBGL]);
        else
            renderer.drawBuffersExt.drawBuffersWEBGL([renderer.gl.BACK]);

        // Clear out the resolve buffer, if necessary.
        this.clearForResolve(renderer);

        // Resolve.
        const resolveProgram = this.getResolveProgram(renderer);
        renderer.gl.useProgram(resolveProgram.program);
        renderer.vertexArrayObjectExt.bindVertexArrayOES(this.resolveVAO);
        renderer.setFramebufferSizeUniform(resolveProgram.uniforms);
        renderer.gl.activeTexture(renderer.gl.TEXTURE0);
        renderer.gl.bindTexture(renderer.gl.TEXTURE_2D, this.aaAlphaTexture);
        renderer.gl.uniform1i(resolveProgram.uniforms.uAAAlpha, 0);
        renderer.gl.uniform2i(resolveProgram.uniforms.uAAAlphaDimensions,
                          this.supersampledFramebufferSize[0],
                          this.supersampledFramebufferSize[1]);
        this.setResolveUniforms(renderer, resolveProgram);
        renderer.setTransformSTAndTexScaleUniformsForDest(resolveProgram.uniforms);
        renderer.gl.drawElements(renderer.gl.TRIANGLES, 6, renderer.gl.UNSIGNED_BYTE, 0);
        renderer.vertexArrayObjectExt.bindVertexArrayOES(null);
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

        renderer.gl.blendEquation(renderer.gl.FUNC_ADD);
        renderer.gl.blendFunc(renderer.gl.ONE, renderer.gl.ONE);
        renderer.gl.enable(renderer.gl.BLEND);

        this.clear(renderer);
    }

    protected setCoverDepthState(renderer: Renderer): void {
        renderer.gl.disable(renderer.gl.DEPTH_TEST);
    }

    private createCoverVAO(renderer: Renderer) {
        this.coverVAO = renderer.vertexArrayObjectExt.createVertexArrayOES();
        renderer.vertexArrayObjectExt.bindVertexArrayOES(this.coverVAO);

        const coverProgram = renderer.shaderPrograms.mcaaCover;
        const attributes = coverProgram.attributes;
        renderer.gl.useProgram(coverProgram.program);
        renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, renderer.quadPositionsBuffer);
        renderer.gl.vertexAttribPointer(attributes.aQuadPosition, 2, renderer.gl.FLOAT, false, 0, 0);
        renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER,
                               renderer.meshes[0].edgeBoundingBoxVertexPositions);
        renderer.gl.vertexAttribPointer(attributes.aUpperLeftPosition,
                                        2,
                                        renderer.gl.FLOAT,
                                        false,
                                        FLOAT32_SIZE * 4,
                                        0);
        renderer.gl.vertexAttribPointer(attributes.aLowerRightPosition,
                                        2,
                                        renderer.gl.FLOAT,
                                        false,
                                        FLOAT32_SIZE * 4,
                                        FLOAT32_SIZE * 2);
        renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER,
                               renderer.meshes[0].edgeBoundingBoxPathIDs);
        renderer.gl.vertexAttribPointer(attributes.aPathID,
                                        1,
                                        renderer.gl.UNSIGNED_SHORT,
                                        false,
                                        0,
                                        0);
        renderer.gl.enableVertexAttribArray(attributes.aQuadPosition);
        renderer.gl.enableVertexAttribArray(attributes.aUpperLeftPosition);
        renderer.gl.enableVertexAttribArray(attributes.aLowerRightPosition);
        renderer.gl.enableVertexAttribArray(attributes.aPathID);
        renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aUpperLeftPosition, 1);
        renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLowerRightPosition, 1);
        renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);
        renderer.gl.bindBuffer(renderer.gl.ELEMENT_ARRAY_BUFFER, renderer.quadElementsBuffer);

        renderer.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private createLineVAOs(renderer: Renderer) {
        const vaos: Partial<FastEdgeVAOs> = {};
        const lineProgram = renderer.shaderPrograms.mcaaLine;
        const attributes = lineProgram.attributes;

        for (const direction of DIRECTIONS) {
            const vao = renderer.vertexArrayObjectExt.createVertexArrayOES();
            renderer.vertexArrayObjectExt.bindVertexArrayOES(vao);

            const lineVertexPositionsBuffer = {
                lower: renderer.meshes[0].edgeLowerLineVertexPositions,
                upper: renderer.meshes[0].edgeUpperLineVertexPositions,
            }[direction];
            const linePathIDsBuffer = {
                lower: renderer.meshes[0].edgeLowerLinePathIDs,
                upper: renderer.meshes[0].edgeUpperLinePathIDs,
            }[direction];

            renderer.gl.useProgram(lineProgram.program);
            renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, renderer.quadPositionsBuffer);
            renderer.gl.vertexAttribPointer(attributes.aQuadPosition, 2, renderer.gl.FLOAT, false, 0, 0);
            renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, lineVertexPositionsBuffer);
            renderer.gl.vertexAttribPointer(attributes.aLeftPosition,
                                            2,
                                            renderer.gl.FLOAT,
                                            false,
                                            FLOAT32_SIZE * 4,
                                            0);
            renderer.gl.vertexAttribPointer(attributes.aRightPosition,
                                            2,
                                            renderer.gl.FLOAT,
                                            false,
                                            FLOAT32_SIZE * 4,
                                            FLOAT32_SIZE * 2);
            renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, linePathIDsBuffer);
            renderer.gl.vertexAttribPointer(attributes.aPathID,
                                            1,
                                            renderer.gl.UNSIGNED_SHORT,
                                            false,
                                            0,
                                            0);

            renderer.gl.enableVertexAttribArray(attributes.aQuadPosition);
            renderer.gl.enableVertexAttribArray(attributes.aLeftPosition);
            renderer.gl.enableVertexAttribArray(attributes.aRightPosition);
            renderer.gl.enableVertexAttribArray(attributes.aPathID);

            renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
            renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightPosition, 1);
            renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);

            renderer.gl.bindBuffer(renderer.gl.ELEMENT_ARRAY_BUFFER, renderer.quadElementsBuffer);

            vaos[direction] = vao;
        }

        renderer.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.lineVAOs = vaos as FastEdgeVAOs;
    }

    private createCurveVAOs(renderer: Renderer) {
        const vaos: Partial<FastEdgeVAOs> = {};
        const curveProgram = renderer.shaderPrograms.mcaaCurve;
        const attributes = curveProgram.attributes;

        for (const direction of DIRECTIONS) {
            const vao = renderer.vertexArrayObjectExt.createVertexArrayOES();
            renderer.vertexArrayObjectExt.bindVertexArrayOES(vao);

            const curveVertexPositionsBuffer = {
                lower: renderer.meshes[0].edgeLowerCurveVertexPositions,
                upper: renderer.meshes[0].edgeUpperCurveVertexPositions,
            }[direction];
            const curvePathIDsBuffer = {
                lower: renderer.meshes[0].edgeLowerCurvePathIDs,
                upper: renderer.meshes[0].edgeUpperCurvePathIDs,
            }[direction];

            renderer.gl.useProgram(curveProgram.program);
            renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, renderer.quadPositionsBuffer);
            renderer.gl.vertexAttribPointer(attributes.aQuadPosition,
                                            2,
                                            renderer.gl.FLOAT,
                                            false,
                                            0,
                                            0);
            renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, curveVertexPositionsBuffer);
            renderer.gl.vertexAttribPointer(attributes.aLeftPosition,
                                            2,
                                            renderer.gl.FLOAT,
                                            false,
                                            FLOAT32_SIZE * 6,
                                            0);
            renderer.gl.vertexAttribPointer(attributes.aControlPointPosition,
                                            2,
                                            renderer.gl.FLOAT,
                                            false,
                                            FLOAT32_SIZE * 6,
                                            FLOAT32_SIZE * 2);
            renderer.gl.vertexAttribPointer(attributes.aRightPosition,
                                            2,
                                            renderer.gl.FLOAT,
                                            false,
                                            FLOAT32_SIZE * 6,
                                            FLOAT32_SIZE * 4);
            renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, curvePathIDsBuffer);
            renderer.gl.vertexAttribPointer(attributes.aPathID,
                                            1,
                                            renderer.gl.UNSIGNED_SHORT,
                                            false,
                                            0,
                                            0);

            renderer.gl.enableVertexAttribArray(attributes.aQuadPosition);
            renderer.gl.enableVertexAttribArray(attributes.aLeftPosition);
            renderer.gl.enableVertexAttribArray(attributes.aControlPointPosition);
            renderer.gl.enableVertexAttribArray(attributes.aRightPosition);
            renderer.gl.enableVertexAttribArray(attributes.aPathID);

            renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
            renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aControlPointPosition,
                                                                1);
            renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightPosition, 1);
            renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);

            renderer.gl.bindBuffer(renderer.gl.ELEMENT_ARRAY_BUFFER, renderer.quadElementsBuffer);

            vaos[direction] = vao;
        }

        renderer.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.curveVAOs = vaos as FastEdgeVAOs;
    }

    private cover(renderer: Renderer): void {
        // Conservatively cover.
        const coverProgram = renderer.shaderPrograms.mcaaCover;
        renderer.gl.useProgram(coverProgram.program);
        renderer.vertexArrayObjectExt.bindVertexArrayOES(this.coverVAO);
        this.setAAUniforms(renderer, coverProgram.uniforms);
        renderer.instancedArraysExt.drawElementsInstancedANGLE(renderer.gl.TRIANGLES,
                                                               6,
                                                               renderer.gl.UNSIGNED_BYTE,
                                                               0,
                                                               renderer.meshData[0].bQuadCount);
        renderer.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private setBlendModeForAA(renderer: Renderer, direction: 'upper' | 'lower') {
        renderer.gl.blendEquation(renderer.gl.FUNC_ADD);
        renderer.gl.blendFunc(renderer.gl.ONE, renderer.gl.ONE);
        renderer.gl.enable(renderer.gl.BLEND);
    }

    private antialiasLines(renderer: Renderer) {
        this.setAAState(renderer);

        const lineProgram = renderer.shaderPrograms.mcaaLine;

        renderer.gl.useProgram(lineProgram.program);
        const uniforms = lineProgram.uniforms;
        this.setAAUniforms(renderer, uniforms);

        for (const direction of DIRECTIONS) {
            const vao = this.lineVAOs[direction];
            renderer.vertexArrayObjectExt.bindVertexArrayOES(vao);

            this.setBlendModeForAA(renderer, direction);
            renderer.gl.uniform1i(uniforms.uWinding, direction === 'upper' ? 1 : 0);

            const count = {
                lower: renderer.meshData[0].edgeLowerLineCount,
                upper: renderer.meshData[0].edgeUpperLineCount,
            }[direction];
            renderer.instancedArraysExt.drawElementsInstancedANGLE(renderer.gl.TRIANGLES,
                                                                   6,
                                                                   renderer.gl.UNSIGNED_BYTE,
                                                                   0,
                                                                   count);
        }

        renderer.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private antialiasCurves(renderer: Renderer) {
        this.setAAState(renderer);

        const curveProgram = renderer.shaderPrograms.mcaaCurve;

        renderer.gl.useProgram(curveProgram.program);
        const uniforms = curveProgram.uniforms;
        this.setAAUniforms(renderer, uniforms);

        for (const direction of DIRECTIONS) {
            const vao = this.curveVAOs[direction];
            renderer.vertexArrayObjectExt.bindVertexArrayOES(vao);

            this.setBlendModeForAA(renderer, direction);
            renderer.gl.uniform1i(uniforms.uWinding, direction === 'upper' ? 1 : 0);

            const count = {
                lower: renderer.meshData[0].edgeLowerCurveCount,
                upper: renderer.meshData[0].edgeUpperCurveCount,
            }[direction];
            renderer.instancedArraysExt.drawElementsInstancedANGLE(renderer.gl.TRIANGLES,
                                                                   6,
                                                                   renderer.gl.UNSIGNED_BYTE,
                                                                   0,
                                                                   count);
        }

        renderer.vertexArrayObjectExt.bindVertexArrayOES(null);
    }
}

export class ECAAStrategy extends XCAAStrategy {
    private lineVAO: WebGLVertexArrayObject;
    private curveVAO: WebGLVertexArrayObject;

    get shouldRenderDirect() {
        return false;
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
        const emboldenAmount = renderer.emboldenAmount;
        renderer.gl.uniform2f(uniforms.uEmboldenAmount, emboldenAmount[0], emboldenAmount[1]);
    }

    protected getResolveProgram(renderer: Renderer): PathfinderShaderProgram {
        if (this.subpixelAA !== 'none')
            return renderer.shaderPrograms.xcaaMonoSubpixelResolve;
        return renderer.shaderPrograms.xcaaMonoResolve;
    }

    protected initEdgeDetectFramebuffer(renderer: Renderer) {}

    protected createEdgeDetectVAO(renderer: Renderer) {}

    protected detectEdgesIfNecessary(renderer: Renderer) {}

    protected clear(renderer: Renderer) {
        renderer.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        renderer.gl.clearDepth(0.0);
        renderer.gl.clear(renderer.gl.COLOR_BUFFER_BIT | renderer.gl.DEPTH_BUFFER_BIT);
    }

    protected setAADepthState(renderer: Renderer) {
        renderer.gl.disable(renderer.gl.DEPTH_TEST);
    }

    protected clearForResolve(renderer: Renderer) {
        renderer.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        renderer.gl.clear(renderer.gl.COLOR_BUFFER_BIT);
    }

    protected setResolveUniforms(renderer: Renderer, program: PathfinderShaderProgram) {
        if (renderer.bgColor != null)
            renderer.gl.uniform4fv(program.uniforms.uBGColor, renderer.bgColor);
        if (renderer.fgColor != null)
            renderer.gl.uniform4fv(program.uniforms.uFGColor, renderer.fgColor);
    }

    private setBlendModeForAA(renderer: Renderer) {
        renderer.gl.blendEquation(renderer.gl.FUNC_ADD);
        renderer.gl.blendFunc(renderer.gl.ONE, renderer.gl.ONE);
        renderer.gl.enable(renderer.gl.BLEND);
    }

    private createLineVAO(renderer: Renderer) {
        const lineProgram = renderer.shaderPrograms.ecaaLine;
        const attributes = lineProgram.attributes;

        const vao = renderer.vertexArrayObjectExt.createVertexArrayOES();
        renderer.vertexArrayObjectExt.bindVertexArrayOES(vao);

        const lineVertexPositionsBuffer = renderer.meshes[0].segmentLines;
        const linePathIDsBuffer = renderer.meshes[0].segmentLinePathIDs;
        const lineNormalsBuffer = renderer.meshes[0].segmentLineNormals;

        renderer.gl.useProgram(lineProgram.program);
        renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, renderer.quadPositionsBuffer);
        renderer.gl.vertexAttribPointer(attributes.aQuadPosition, 2, renderer.gl.FLOAT, false, 0, 0);
        renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, lineVertexPositionsBuffer);
        renderer.gl.vertexAttribPointer(attributes.aLeftPosition,
                                    2,
                                    renderer.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 4,
                                    0);
        renderer.gl.vertexAttribPointer(attributes.aRightPosition,
                                    2,
                                    renderer.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 4,
                                    FLOAT32_SIZE * 2);
        renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, linePathIDsBuffer);
        renderer.gl.vertexAttribPointer(attributes.aPathID,
                                    1,
                                    renderer.gl.UNSIGNED_SHORT,
                                    false,
                                    0,
                                    0);
        renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, lineNormalsBuffer);
        renderer.gl.vertexAttribPointer(attributes.aLeftNormalAngle,
                                    1,
                                    renderer.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 2,
                                    0);
        renderer.gl.vertexAttribPointer(attributes.aRightNormalAngle,
                                    1,
                                    renderer.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 2,
                                    FLOAT32_SIZE);

        renderer.gl.enableVertexAttribArray(attributes.aQuadPosition);
        renderer.gl.enableVertexAttribArray(attributes.aLeftPosition);
        renderer.gl.enableVertexAttribArray(attributes.aRightPosition);
        renderer.gl.enableVertexAttribArray(attributes.aPathID);
        renderer.gl.enableVertexAttribArray(attributes.aLeftNormalAngle);
        renderer.gl.enableVertexAttribArray(attributes.aRightNormalAngle);

        renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
        renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightPosition, 1);
        renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);
        renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftNormalAngle, 1);
        renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightNormalAngle, 1);

        renderer.gl.bindBuffer(renderer.gl.ELEMENT_ARRAY_BUFFER, renderer.quadElementsBuffer);

        renderer.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.lineVAO = vao;
    }

    private createCurveVAO(renderer: Renderer) {
        const curveProgram = renderer.shaderPrograms.ecaaCurve;
        const attributes = curveProgram.attributes;

        const vao = renderer.vertexArrayObjectExt.createVertexArrayOES();
        renderer.vertexArrayObjectExt.bindVertexArrayOES(vao);

        const curveVertexPositionsBuffer = renderer.meshes[0].segmentCurves;
        const curvePathIDsBuffer = renderer.meshes[0].segmentCurvePathIDs;
        const curveNormalsBuffer = renderer.meshes[0].segmentCurveNormals;

        renderer.gl.useProgram(curveProgram.program);
        renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, renderer.quadPositionsBuffer);
        renderer.gl.vertexAttribPointer(attributes.aQuadPosition, 2, renderer.gl.FLOAT, false, 0, 0);
        renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, curveVertexPositionsBuffer);
        renderer.gl.vertexAttribPointer(attributes.aLeftPosition,
                                        2,
                                        renderer.gl.FLOAT,
                                        false,
                                        FLOAT32_SIZE * 6,
                                        0);
        renderer.gl.vertexAttribPointer(attributes.aControlPointPosition,
                                        2,
                                        renderer.gl.FLOAT,
                                        false,
                                        FLOAT32_SIZE * 6,
                                        FLOAT32_SIZE * 2);
        renderer.gl.vertexAttribPointer(attributes.aRightPosition,
                                        2,
                                        renderer.gl.FLOAT,
                                        false,
                                        FLOAT32_SIZE * 6,
                                        FLOAT32_SIZE * 4);
        renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, curvePathIDsBuffer);
        renderer.gl.vertexAttribPointer(attributes.aPathID,
                                        1,
                                        renderer.gl.UNSIGNED_SHORT,
                                        false,
                                        0,
                                        0);
        renderer.gl.bindBuffer(renderer.gl.ARRAY_BUFFER, curveNormalsBuffer);
        renderer.gl.vertexAttribPointer(attributes.aNormalAngles,
                                        3,
                                        renderer.gl.FLOAT,
                                        false,
                                        FLOAT32_SIZE * 3,
                                        0);

        renderer.gl.enableVertexAttribArray(attributes.aQuadPosition);
        renderer.gl.enableVertexAttribArray(attributes.aLeftPosition);
        renderer.gl.enableVertexAttribArray(attributes.aControlPointPosition);
        renderer.gl.enableVertexAttribArray(attributes.aRightPosition);
        renderer.gl.enableVertexAttribArray(attributes.aPathID);
        renderer.gl.enableVertexAttribArray(attributes.aNormalAngles);

        renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
        renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aControlPointPosition, 1);
        renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightPosition, 1);
        renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);
        renderer.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aNormalAngles, 1);

        renderer.gl.bindBuffer(renderer.gl.ELEMENT_ARRAY_BUFFER, renderer.quadElementsBuffer);

        renderer.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.curveVAO = vao;
    }

    private antialiasLines(renderer: Renderer) {
        this.setAAState(renderer);

        const lineProgram = renderer.shaderPrograms.ecaaLine;

        renderer.gl.useProgram(lineProgram.program);
        const uniforms = lineProgram.uniforms;
        this.setAAUniforms(renderer, uniforms);

        const vao = this.lineVAO;
        renderer.vertexArrayObjectExt.bindVertexArrayOES(vao);

        this.setBlendModeForAA(renderer);

        const count = renderer.meshData[0].segmentLineCount;
        renderer.instancedArraysExt.drawElementsInstancedANGLE(renderer.gl.TRIANGLES,
                                                               6,
                                                               renderer.gl.UNSIGNED_BYTE,
                                                               0,
                                                               count);

        renderer.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private antialiasCurves(renderer: Renderer) {
        this.setAAState(renderer);

        const curveProgram = renderer.shaderPrograms.ecaaCurve;

        renderer.gl.useProgram(curveProgram.program);
        const uniforms = curveProgram.uniforms;
        this.setAAUniforms(renderer, uniforms);

        const vao = this.curveVAO;
        renderer.vertexArrayObjectExt.bindVertexArrayOES(vao);

        this.setBlendModeForAA(renderer);

        const count = renderer.meshData[0].segmentCurveCount;
        renderer.instancedArraysExt.drawElementsInstancedANGLE(renderer.gl.TRIANGLES,
                                                               6,
                                                               renderer.gl.UNSIGNED_BYTE,
                                                               0,
                                                               count);

        renderer.vertexArrayObjectExt.bindVertexArrayOES(null);
    }
}

export class MCAAMonochromeStrategy extends MCAAStrategy {
    protected getResolveProgram(renderer: Renderer): PathfinderShaderProgram {
        if (this.subpixelAA !== 'none')
            return renderer.shaderPrograms.xcaaMonoSubpixelResolve;
        return renderer.shaderPrograms.xcaaMonoResolve;
    }

    protected initEdgeDetectFramebuffer(renderer: Renderer) {}

    protected createEdgeDetectVAO(renderer: Renderer) {}

    protected detectEdgesIfNecessary(renderer: Renderer) {}

    protected clear(renderer: Renderer) {
        renderer.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        renderer.gl.clearDepth(0.0);
        renderer.gl.clear(renderer.gl.COLOR_BUFFER_BIT | renderer.gl.DEPTH_BUFFER_BIT);
    }

    protected setAADepthState(renderer: Renderer) {
        renderer.gl.disable(renderer.gl.DEPTH_TEST);
    }

    protected clearForResolve(renderer: Renderer) {
        renderer.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        renderer.gl.clear(renderer.gl.COLOR_BUFFER_BIT);
    }

    protected setResolveUniforms(renderer: Renderer, program: PathfinderShaderProgram) {
        if (renderer.bgColor != null)
            renderer.gl.uniform4fv(program.uniforms.uBGColor, renderer.bgColor);
        if (renderer.fgColor != null)
            renderer.gl.uniform4fv(program.uniforms.uFGColor, renderer.fgColor);
    }

    get shouldRenderDirect() {
        return false;
    }
}

/// Switches between the mesh-based MCAA and ECAA depending on whether stem darkening is enabled.
///
/// FIXME(pcwalton): Share textures and FBOs between the two strategies.
export class AdaptiveMonochromeXCAAStrategy implements AntialiasingStrategy {
    private mcaaStrategy: MCAAMonochromeStrategy;
    private ecaaStrategy: ECAAStrategy;

    get shouldRenderDirect(): boolean {
        return false;
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
    private bgColorTexture: WebGLTexture;
    private fgColorTexture: WebGLTexture;

    protected getResolveProgram(renderer: Renderer): PathfinderShaderProgram {
        return renderer.shaderPrograms.xcaaMultiResolve;
    }

    protected initDirectFramebuffer(renderer: Renderer) {
        this._directDepthTexture =
            createFramebufferDepthTexture(renderer.gl, this.supersampledFramebufferSize);
        super.initDirectFramebuffer(renderer);
    }

    protected initEdgeDetectFramebuffer(renderer: Renderer) {
        this.bgColorTexture = createFramebufferColorTexture(renderer.gl,
                                                            this.supersampledFramebufferSize,
                                                            renderer.colorAlphaFormat);
        this.fgColorTexture = createFramebufferColorTexture(renderer.gl,
                                                            this.supersampledFramebufferSize,
                                                            renderer.colorAlphaFormat);
        this.edgeDetectFramebuffer = createFramebuffer(renderer.gl,
                                                       renderer.drawBuffersExt,
                                                       [this.bgColorTexture, this.fgColorTexture],
                                                       this.aaDepthTexture);
    }

    protected createEdgeDetectVAO(renderer: Renderer) {
        this.edgeDetectVAO = renderer.vertexArrayObjectExt.createVertexArrayOES();
        renderer.vertexArrayObjectExt.bindVertexArrayOES(this.edgeDetectVAO);

        const edgeDetectProgram = renderer.shaderPrograms.xcaaEdgeDetect;
        renderer.gl.useProgram(edgeDetectProgram.program);
        renderer.initQuadVAO(edgeDetectProgram.attributes);

        renderer.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected detectEdgesIfNecessary(renderer: Renderer) {
        // Set state for edge detection.
        const edgeDetectProgram = renderer.shaderPrograms.xcaaEdgeDetect;
        renderer.gl.bindFramebuffer(renderer.gl.FRAMEBUFFER, this.edgeDetectFramebuffer);
        renderer.gl.viewport(0,
                             0,
                             this.supersampledFramebufferSize[0],
                             this.supersampledFramebufferSize[1]);

        renderer.drawBuffersExt.drawBuffersWEBGL([
            renderer.drawBuffersExt.COLOR_ATTACHMENT0_WEBGL,
            renderer.drawBuffersExt.COLOR_ATTACHMENT1_WEBGL,
        ]);

        renderer.gl.depthMask(true);
        renderer.gl.depthFunc(renderer.gl.ALWAYS);
        renderer.gl.enable(renderer.gl.DEPTH_TEST);
        renderer.gl.disable(renderer.gl.BLEND);

        renderer.gl.clearDepth(0.0);
        renderer.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        renderer.gl.clear(renderer.gl.COLOR_BUFFER_BIT | renderer.gl.DEPTH_BUFFER_BIT);

        // Perform edge detection.
        renderer.gl.useProgram(edgeDetectProgram.program);
        renderer.vertexArrayObjectExt.bindVertexArrayOES(this.edgeDetectVAO);
        renderer.setFramebufferSizeUniform(edgeDetectProgram.uniforms);
        renderer.setTransformSTAndTexScaleUniformsForDest(edgeDetectProgram.uniforms);
        renderer.gl.activeTexture(renderer.gl.TEXTURE0);
        renderer.gl.bindTexture(renderer.gl.TEXTURE_2D, this.directColorTexture);
        renderer.gl.uniform1i(edgeDetectProgram.uniforms.uColor, 0);
        renderer.gl.activeTexture(renderer.gl.TEXTURE1);
        renderer.gl.bindTexture(renderer.gl.TEXTURE_2D, this.directPathIDTexture);
        renderer.gl.uniform1i(edgeDetectProgram.uniforms.uPathID, 1);
        renderer.gl.bindBuffer(renderer.gl.ELEMENT_ARRAY_BUFFER, renderer.quadElementsBuffer);
        renderer.gl.drawElements(renderer.gl.TRIANGLES, 6, renderer.gl.UNSIGNED_BYTE, 0);
        renderer.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected setCoverDepthState(renderer: Renderer) {
        renderer.gl.depthMask(false);
        renderer.gl.depthFunc(renderer.gl.ALWAYS);
        renderer.gl.enable(renderer.gl.DEPTH_TEST);
    }

    protected clear(renderer: Renderer) {
        renderer.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        renderer.gl.clear(renderer.gl.COLOR_BUFFER_BIT);
    }

    protected setAADepthState(renderer: Renderer) {
        renderer.gl.depthMask(false);
        renderer.gl.depthFunc(renderer.gl.EQUAL);
        renderer.gl.enable(renderer.gl.DEPTH_TEST);
    }

    protected setResolveDepthState(renderer: Renderer) {
        renderer.gl.depthMask(false);
        renderer.gl.depthFunc(renderer.gl.NOTEQUAL);
        renderer.gl.enable(renderer.gl.DEPTH_TEST);
    }

    protected clearForResolve(renderer: Renderer) {}

    protected setResolveUniforms(renderer: Renderer, program: PathfinderShaderProgram) {
        renderer.gl.activeTexture(renderer.gl.TEXTURE1);
        renderer.gl.bindTexture(renderer.gl.TEXTURE_2D, this.bgColorTexture);
        renderer.gl.uniform1i(program.uniforms.uBGColor, 1);
        renderer.gl.activeTexture(renderer.gl.TEXTURE2);
        renderer.gl.bindTexture(renderer.gl.TEXTURE_2D, this.fgColorTexture);
        renderer.gl.uniform1i(program.uniforms.uFGColor, 2);
    }

    get shouldRenderDirect() {
        return true;
    }

    protected get directDepthTexture(): WebGLTexture {
        return this._directDepthTexture;
    }
}
