// pathfinder/client/src/ecaa-strategy.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';

import {AntialiasingStrategy} from './aa-strategy';
import {createFramebuffer, createFramebufferColorTexture} from './gl-utils';
import {createFramebufferDepthTexture, setTextureParameters, UniformMap} from './gl-utils';
import {WebGLVertexArrayObject} from './gl-utils';
import {B_QUAD_LOWER_INDICES_OFFSET, B_QUAD_SIZE, B_QUAD_UPPER_INDICES_OFFSET} from './meshes';
import {PathfinderShaderProgram} from './shader-loader';
import {UINT32_SIZE, unwrapNull} from './utils';
import {MonochromePathfinderView} from './view';
import PathfinderBufferTexture from './buffer-texture';

interface UpperAndLower<T> {
    upper: T;
    lower: T;
}

export abstract class ECAAStrategy extends AntialiasingStrategy {
    constructor(level: number, subpixelAA: boolean) {
        super();

        this.subpixelAA = subpixelAA;

        this.supersampledFramebufferSize = glmatrix.vec2.create();
        this.destFramebufferSize = glmatrix.vec2.create();
    }

    init(view: MonochromePathfinderView) {
        super.init(view);
        this.bVertexPositionBufferTexture = new PathfinderBufferTexture(view.gl,
                                                                        'uBVertexPosition');
        this.bVertexPathIDBufferTexture = new PathfinderBufferTexture(view.gl, 'uBVertexPathID');
    }

    attachMeshes(view: MonochromePathfinderView) {
        const bVertexPositions = new Float32Array(view.meshData[0].bVertexPositions);
        const bVertexPathIDs = new Uint8Array(view.meshData[0].bVertexPathIDs);
        this.bVertexPositionBufferTexture.upload(view.gl, bVertexPositions);
        this.bVertexPathIDBufferTexture.upload(view.gl, bVertexPathIDs);

        this.createEdgeDetectVAO(view);
        this.createCoverVAO(view);
        this.createLineVAOs(view);
        this.createCurveVAOs(view);
        this.createResolveVAO(view);
    }

    setFramebufferSize(view: MonochromePathfinderView) {
        this.destFramebufferSize = glmatrix.vec2.clone(view.destAllocatedSize);
        glmatrix.vec2.mul(this.supersampledFramebufferSize,
                          this.destFramebufferSize,
                          this.supersampleScale);

        this.initDirectFramebuffer(view);
        this.initEdgeDetectFramebuffer(view);
        this.initAAAlphaFramebuffer(view);
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, null);
    }

    get transform(): glmatrix.mat4 {
        return glmatrix.mat4.create();
    }

    protected initDirectFramebuffer(view: MonochromePathfinderView) {
        this.directColorTexture = createFramebufferColorTexture(view.gl, this.destFramebufferSize);
        this.directPathIDTexture = createFramebufferColorTexture(view.gl,
                                                                 this.destFramebufferSize);
        this.directFramebuffer =
            createFramebuffer(view.gl,
                              view.drawBuffersExt,
                              [this.directColorTexture, this.directPathIDTexture],
                              this.directDepthTexture);
    }

    private initAAAlphaFramebuffer(view: MonochromePathfinderView) {
        this.aaAlphaTexture = unwrapNull(view.gl.createTexture());
        view.gl.activeTexture(view.gl.TEXTURE0);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.aaAlphaTexture);
        view.gl.texImage2D(view.gl.TEXTURE_2D,
                           0,
                           view.gl.RGB,
                           this.supersampledFramebufferSize[0],
                           this.supersampledFramebufferSize[1],
                           0,
                           view.gl.RGB,
                           view.textureHalfFloatExt.HALF_FLOAT_OES,
                           null);
        setTextureParameters(view.gl, view.gl.NEAREST);

        this.aaDepthTexture = createFramebufferDepthTexture(view.gl,
                                                            this.supersampledFramebufferSize);

        this.aaFramebuffer = createFramebuffer(view.gl,
                                               view.drawBuffersExt,
                                               [this.aaAlphaTexture],
                                               this.aaDepthTexture);
    }

    private createCoverVAO(view: MonochromePathfinderView) {
        this.coverVAO = view.vertexArrayObjectExt.createVertexArrayOES();
        view.vertexArrayObjectExt.bindVertexArrayOES(this.coverVAO);

        const coverProgram = view.shaderPrograms.ecaaCover;
        const attributes = coverProgram.attributes;
        view.gl.useProgram(coverProgram.program);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadPositionsBuffer);
        view.gl.vertexAttribPointer(attributes.aQuadPosition, 2, view.gl.FLOAT, false, 0, 0);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.meshes[0].bQuads);
        view.gl.vertexAttribPointer(attributes.aUpperPointIndices,
                                    4,
                                    view.gl.UNSIGNED_SHORT,
                                    false,
                                    B_QUAD_SIZE,
                                    B_QUAD_UPPER_INDICES_OFFSET);
        view.gl.vertexAttribPointer(attributes.aLowerPointIndices,
                                    4,
                                    view.gl.UNSIGNED_SHORT,
                                    false,
                                    B_QUAD_SIZE,
                                    B_QUAD_LOWER_INDICES_OFFSET);
        view.gl.enableVertexAttribArray(attributes.aQuadPosition);
        view.gl.enableVertexAttribArray(attributes.aUpperPointIndices);
        view.gl.enableVertexAttribArray(attributes.aLowerPointIndices);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aUpperPointIndices, 1);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLowerPointIndices, 1);
        view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private createLineVAOs(view: MonochromePathfinderView) {
        const lineProgram = view.shaderPrograms.ecaaLine;
        const attributes = lineProgram.attributes;

        const vaos: Partial<UpperAndLower<WebGLVertexArrayObject>> = {};
        for (const direction of ['upper', 'lower'] as Array<'upper' | 'lower'>) {
            vaos[direction] = view.vertexArrayObjectExt.createVertexArrayOES();
            view.vertexArrayObjectExt.bindVertexArrayOES(vaos[direction]);

            const lineIndexBuffer = {
                upper: view.meshes[0].edgeUpperLineIndices,
                lower: view.meshes[0].edgeLowerLineIndices,
            }[direction];

            view.gl.useProgram(lineProgram.program);
            view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadPositionsBuffer);
            view.gl.vertexAttribPointer(attributes.aQuadPosition, 2, view.gl.FLOAT, false, 0, 0);
            view.gl.bindBuffer(view.gl.ARRAY_BUFFER, lineIndexBuffer);
            view.gl.vertexAttribPointer(attributes.aLineIndices,
                                        4,
                                        view.gl.UNSIGNED_SHORT,
                                        false,
                                        0,
                                        0);
            view.gl.enableVertexAttribArray(attributes.aQuadPosition);
            view.gl.enableVertexAttribArray(attributes.aLineIndices);
            view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLineIndices, 1);
            view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);
        }

        view.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.lineVAOs = vaos as UpperAndLower<WebGLVertexArrayObject>;
    }

    private createCurveVAOs(view: MonochromePathfinderView) {
        const curveProgram = view.shaderPrograms.ecaaCurve;
        const attributes = curveProgram.attributes;

        const vaos: Partial<UpperAndLower<WebGLVertexArrayObject>> = {};
        for (const direction of ['upper', 'lower'] as Array<'upper' | 'lower'>) {
            vaos[direction] = view.vertexArrayObjectExt.createVertexArrayOES();
            view.vertexArrayObjectExt.bindVertexArrayOES(vaos[direction]);

            const curveIndexBuffer = {
                upper: view.meshes[0].edgeUpperCurveIndices,
                lower: view.meshes[0].edgeLowerCurveIndices,
            }[direction];

            view.gl.useProgram(curveProgram.program);
            view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadPositionsBuffer);
            view.gl.vertexAttribPointer(attributes.aQuadPosition, 2, view.gl.FLOAT, false, 0, 0);
            view.gl.bindBuffer(view.gl.ARRAY_BUFFER, curveIndexBuffer);
            view.gl.vertexAttribPointer(attributes.aCurveEndpointIndices,
                                        4,
                                        view.gl.UNSIGNED_SHORT,
                                        false,
                                        UINT32_SIZE * 4,
                                        0);
            view.gl.vertexAttribPointer(attributes.aCurveControlPointIndex,
                                        2,
                                        view.gl.UNSIGNED_SHORT,
                                        false,
                                        UINT32_SIZE * 4,
                                        UINT32_SIZE * 2);
            view.gl.enableVertexAttribArray(attributes.aQuadPosition);
            view.gl.enableVertexAttribArray(attributes.aCurveEndpointIndices);
            view.gl.enableVertexAttribArray(attributes.aCurveControlPointIndex);
            view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aCurveEndpointIndices, 1);
            view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aCurveControlPointIndex, 1);
            view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);
        }

        view.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.curveVAOs = vaos as UpperAndLower<WebGLVertexArrayObject>;
    }

    private createResolveVAO(view: MonochromePathfinderView) {
        this.resolveVAO = view.vertexArrayObjectExt.createVertexArrayOES();
        view.vertexArrayObjectExt.bindVertexArrayOES(this.resolveVAO);

        const resolveProgram = this.getResolveProgram(view);
        view.gl.useProgram(resolveProgram.program);
        view.initQuadVAO(resolveProgram.attributes);

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    prepare(view: MonochromePathfinderView) {
        const usedSize = this.supersampledUsedSize(view);;
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, this.directFramebuffer);
        view.gl.viewport(0,
                         0,
                         this.supersampledFramebufferSize[0],
                         this.supersampledFramebufferSize[1]);
        view.gl.scissor(0, 0, usedSize[0], usedSize[1]);
        view.gl.enable(view.gl.SCISSOR_TEST);

        // Clear out the color and depth textures.
        view.drawBuffersExt.drawBuffersWEBGL([
            view.drawBuffersExt.COLOR_ATTACHMENT0_WEBGL,
            view.drawBuffersExt.NONE,
        ]);
        view.gl.clearColor(1.0, 1.0, 1.0, 1.0);
        view.gl.clearDepth(0.0);
        view.gl.depthMask(true);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT | view.gl.DEPTH_BUFFER_BIT);

        // Clear out the path ID texture.
        view.drawBuffersExt.drawBuffersWEBGL([
            view.drawBuffersExt.NONE,
            view.drawBuffersExt.COLOR_ATTACHMENT1_WEBGL,
        ]);
        view.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT);

        // Render to both textures.
        view.drawBuffersExt.drawBuffersWEBGL([
            view.drawBuffersExt.COLOR_ATTACHMENT0_WEBGL,
            view.drawBuffersExt.COLOR_ATTACHMENT1_WEBGL,
        ]);
    }

    antialias(view: MonochromePathfinderView) {
        // Detect edges if necessary.
        this.detectEdgesIfNecessary(view);

        // Conservatively cover.
        this.cover(view);

        // Antialias.
        this.antialiasLines(view);
        this.antialiasCurves(view);
    }

    resolve(view: MonochromePathfinderView) {
        // Resolve the antialiasing.
        this.resolveAA(view);
    }

    private cover(view: MonochromePathfinderView) {
        // Set state for conservative coverage.
        const coverProgram = view.shaderPrograms.ecaaCover;
        const usedSize = this.supersampledUsedSize(view);
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, this.aaFramebuffer);
        view.gl.viewport(0,
                         0,
                         this.supersampledFramebufferSize[0],
                         this.supersampledFramebufferSize[1]);
        view.gl.scissor(0, 0, usedSize[0], usedSize[1]);
        view.gl.enable(view.gl.SCISSOR_TEST);

        this.setCoverDepthState(view);

        view.gl.blendEquation(view.gl.FUNC_ADD);
        view.gl.blendFunc(view.gl.ONE, view.gl.ONE);
        view.gl.enable(view.gl.BLEND);

        this.clearForCover(view);

        // Conservatively cover.
        view.gl.useProgram(coverProgram.program);
        view.vertexArrayObjectExt.bindVertexArrayOES(this.coverVAO);
        const uniforms = coverProgram.uniforms;
        view.setTransformSTUniform(uniforms, 0);
        view.setFramebufferSizeUniform(uniforms);
        this.bVertexPositionBufferTexture.bind(view.gl, uniforms, 0);
        this.bVertexPathIDBufferTexture.bind(view.gl, uniforms, 1);
        view.pathTransformBufferTextures[0].bind(view.gl, uniforms, 2);
        if (view.pathHintsBufferTexture !== null)
            view.pathHintsBufferTexture.bind(view.gl, uniforms, 3);
        view.instancedArraysExt.drawElementsInstancedANGLE(view.gl.TRIANGLES,
                                                           6,
                                                           view.gl.UNSIGNED_BYTE,
                                                           0,
                                                           view.meshData[0].bQuadCount);
        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private setAAState(view: MonochromePathfinderView) {
        const usedSize = this.supersampledUsedSize(view);
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, this.aaFramebuffer);
        view.gl.viewport(0,
                         0,
                         this.supersampledFramebufferSize[0],
                         this.supersampledFramebufferSize[1]);
        view.gl.scissor(0, 0, usedSize[0], usedSize[1]);
        view.gl.enable(view.gl.SCISSOR_TEST);

        this.setAADepthState(view);

        view.gl.blendEquation(view.gl.FUNC_REVERSE_SUBTRACT);
        view.gl.blendFunc(view.gl.ONE, view.gl.ONE);
        view.gl.enable(view.gl.BLEND);
    }

    private setAAUniforms(view: MonochromePathfinderView, uniforms: UniformMap) {
        view.setTransformSTUniform(uniforms, 0);
        view.setFramebufferSizeUniform(uniforms);
        this.bVertexPositionBufferTexture.bind(view.gl, uniforms, 0);
        this.bVertexPathIDBufferTexture.bind(view.gl, uniforms, 1);
        view.pathTransformBufferTextures[0].bind(view.gl, uniforms, 2);
        if (view.pathHintsBufferTexture !== null)
            view.pathHintsBufferTexture.bind(view.gl, uniforms, 3);
    }

    private antialiasLines(view: MonochromePathfinderView) {
        this.setAAState(view);

        const lineProgram = view.shaderPrograms.ecaaLine;
        view.gl.useProgram(lineProgram.program);
        const uniforms = lineProgram.uniforms;
        this.setAAUniforms(view, uniforms);

        for (const direction of ['upper', 'lower'] as Array<keyof UpperAndLower<void>>) {
            view.vertexArrayObjectExt.bindVertexArrayOES(this.lineVAOs[direction]);
            view.gl.uniform1i(uniforms.uLowerPart, direction === 'lower' ? 1 : 0);
            const count = {
                upper: view.meshData[0].edgeUpperLineIndexCount,
                lower: view.meshData[0].edgeLowerLineIndexCount,
            }[direction];
            view.instancedArraysExt.drawElementsInstancedANGLE(view.gl.TRIANGLES,
                                                               6,
                                                               view.gl.UNSIGNED_BYTE,
                                                               0,
                                                               count);
        }

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private antialiasCurves(view: MonochromePathfinderView) {
        this.setAAState(view);

        const curveProgram = view.shaderPrograms.ecaaCurve;
        view.gl.useProgram(curveProgram.program);
        const uniforms = curveProgram.uniforms;
        this.setAAUniforms(view, uniforms);

        for (const direction of ['upper', 'lower'] as Array<keyof UpperAndLower<void>>) {
            view.vertexArrayObjectExt.bindVertexArrayOES(this.curveVAOs[direction]);
            view.gl.uniform1i(uniforms.uLowerPart, direction === 'lower' ? 1 : 0);
            const count = {
                upper: view.meshData[0].edgeUpperCurveIndexCount,
                lower: view.meshData[0].edgeLowerCurveIndexCount,
            }[direction];
            view.instancedArraysExt.drawElementsInstancedANGLE(view.gl.TRIANGLES,
                                                               6,
                                                               view.gl.UNSIGNED_BYTE,
                                                               0,
                                                               count);
        }

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private resolveAA(view: MonochromePathfinderView) {
        // Set state for ECAA resolve.
        const usedSize = view.destUsedSize;
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, view.destFramebuffer);
        view.gl.viewport(0, 0, this.destFramebufferSize[0], this.destFramebufferSize[1]);
        view.gl.scissor(0, 0, usedSize[0], usedSize[1]);
        view.gl.enable(view.gl.SCISSOR_TEST);
        this.setResolveDepthState(view);
        view.gl.disable(view.gl.BLEND);
        if (view.destFramebuffer != null)
            view.drawBuffersExt.drawBuffersWEBGL([view.drawBuffersExt.COLOR_ATTACHMENT0_WEBGL]);
        else
            view.drawBuffersExt.drawBuffersWEBGL([view.gl.BACK]);

        // Clear out the resolve buffer, if necessary.
        this.clearForResolve(view);

        // Resolve.
        const resolveProgram = this.getResolveProgram(view);
        view.gl.useProgram(resolveProgram.program);
        view.vertexArrayObjectExt.bindVertexArrayOES(this.resolveVAO);
        view.setFramebufferSizeUniform(resolveProgram.uniforms);
        view.gl.activeTexture(view.gl.TEXTURE0);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.aaAlphaTexture);
        view.gl.uniform1i(resolveProgram.uniforms.uAAAlpha, 0);
        view.gl.uniform2i(resolveProgram.uniforms.uAAAlphaDimensions,
                          this.supersampledFramebufferSize[0],
                          this.supersampledFramebufferSize[1]);
        this.setResolveUniforms(view, resolveProgram);
        view.setTransformSTAndTexScaleUniformsForDest(resolveProgram.uniforms);
        view.gl.drawElements(view.gl.TRIANGLES, 6, view.gl.UNSIGNED_BYTE, 0);
        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected setCoverDepthState(view: MonochromePathfinderView): void {
        view.gl.disable(view.gl.DEPTH_TEST);
    }

    protected setResolveDepthState(view: MonochromePathfinderView): void {
        view.gl.disable(view.gl.DEPTH_TEST);
    }

    protected supersampledUsedSize(view: MonochromePathfinderView): glmatrix.vec2 {
        const usedSize = glmatrix.vec2.create();
        glmatrix.vec2.mul(usedSize, view.destUsedSize, this.supersampleScale);
        return usedSize;
    }

    protected abstract getResolveProgram(view: MonochromePathfinderView): PathfinderShaderProgram;
    protected abstract initEdgeDetectFramebuffer(view: MonochromePathfinderView): void;
    protected abstract createEdgeDetectVAO(view: MonochromePathfinderView): void;
    protected abstract detectEdgesIfNecessary(view: MonochromePathfinderView): void; 
    protected abstract clearForCover(view: MonochromePathfinderView): void;
    protected abstract setAADepthState(view: MonochromePathfinderView): void;
    protected abstract clearForResolve(view: MonochromePathfinderView): void;
    protected abstract setResolveUniforms(view: MonochromePathfinderView,
                                          program: PathfinderShaderProgram): void;

    protected get directDepthTexture(): WebGLTexture | null {
        return null;
    }

    protected get supersampleScale(): glmatrix.vec2 {
        return glmatrix.vec2.fromValues(this.subpixelAA ? 3.0 : 1.0, 1.0);
    }

    abstract shouldRenderDirect: boolean;

    private bVertexPositionBufferTexture: PathfinderBufferTexture;
    private bVertexPathIDBufferTexture: PathfinderBufferTexture;
    private directFramebuffer: WebGLFramebuffer;
    private aaAlphaTexture: WebGLTexture;
    private aaFramebuffer: WebGLFramebuffer;
    private coverVAO: WebGLVertexArrayObject;
    private lineVAOs: UpperAndLower<WebGLVertexArrayObject>;
    private curveVAOs: UpperAndLower<WebGLVertexArrayObject>;
    private resolveVAO: WebGLVertexArrayObject;

    protected directColorTexture: WebGLTexture;
    protected directPathIDTexture: WebGLTexture;
    protected aaDepthTexture: WebGLTexture;

    protected supersampledFramebufferSize: glmatrix.vec2;
    protected destFramebufferSize: glmatrix.vec2;

    protected subpixelAA: boolean;
}

export class ECAAMonochromeStrategy extends ECAAStrategy {
    protected getResolveProgram(view: MonochromePathfinderView): PathfinderShaderProgram {
        if (this.subpixelAA)
            return view.shaderPrograms.ecaaMonoSubpixelResolve;
        return view.shaderPrograms.ecaaMonoResolve;
    }

    protected initEdgeDetectFramebuffer(view: MonochromePathfinderView) {}

    protected createEdgeDetectVAO(view: MonochromePathfinderView) {}

    protected detectEdgesIfNecessary(view: MonochromePathfinderView) {}

    protected clearForCover(view: MonochromePathfinderView) {
        view.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        view.gl.clearDepth(0.0);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT | view.gl.DEPTH_BUFFER_BIT);
    }

    protected setAADepthState(view: MonochromePathfinderView) {
        view.gl.disable(view.gl.DEPTH_TEST);
    }

    protected clearForResolve(view: MonochromePathfinderView) {
        view.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT);
    }

    protected setResolveUniforms(view: MonochromePathfinderView, program: PathfinderShaderProgram) {
        view.gl.uniform4fv(program.uniforms.uBGColor, view.bgColor);
        view.gl.uniform4fv(program.uniforms.uFGColor, view.fgColor);
    }

    get shouldRenderDirect() {
        return false;
    }
}

export class ECAAMulticolorStrategy extends ECAAStrategy {
    protected getResolveProgram(view: MonochromePathfinderView): PathfinderShaderProgram {
        return view.shaderPrograms.ecaaMultiResolve;
    }

    protected initDirectFramebuffer(view: MonochromePathfinderView) {
        this._directDepthTexture =
            createFramebufferDepthTexture(view.gl, this.supersampledFramebufferSize);
        super.initDirectFramebuffer(view);
    }

    protected initEdgeDetectFramebuffer(view: MonochromePathfinderView) {
        this.bgColorTexture = createFramebufferColorTexture(view.gl,
                                                            this.supersampledFramebufferSize);
        this.fgColorTexture = createFramebufferColorTexture(view.gl,
                                                            this.supersampledFramebufferSize);
        this.edgeDetectFramebuffer = createFramebuffer(view.gl,
                                                       view.drawBuffersExt,
                                                       [this.bgColorTexture, this.fgColorTexture],
                                                       this.aaDepthTexture);
    }

    protected createEdgeDetectVAO(view: MonochromePathfinderView) {
        this.edgeDetectVAO = view.vertexArrayObjectExt.createVertexArrayOES();
        view.vertexArrayObjectExt.bindVertexArrayOES(this.edgeDetectVAO);

        const edgeDetectProgram = view.shaderPrograms.ecaaEdgeDetect;
        view.gl.useProgram(edgeDetectProgram.program);
        view.initQuadVAO(edgeDetectProgram.attributes);

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected detectEdgesIfNecessary(view: MonochromePathfinderView) {
        // Set state for edge detection.
        const edgeDetectProgram = view.shaderPrograms.ecaaEdgeDetect;
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, this.edgeDetectFramebuffer);
        view.gl.viewport(0,
                         0,
                         this.supersampledFramebufferSize[0],
                         this.supersampledFramebufferSize[1]);

        view.drawBuffersExt.drawBuffersWEBGL([
            view.drawBuffersExt.COLOR_ATTACHMENT0_WEBGL,
            view.drawBuffersExt.COLOR_ATTACHMENT1_WEBGL,
        ]);

        view.gl.depthMask(true);
        view.gl.depthFunc(view.gl.ALWAYS);
        view.gl.enable(view.gl.DEPTH_TEST);
        view.gl.disable(view.gl.BLEND);

        view.gl.clearDepth(0.0);
        view.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT | view.gl.DEPTH_BUFFER_BIT);

        // Perform edge detection.
        view.gl.useProgram(edgeDetectProgram.program);
        view.vertexArrayObjectExt.bindVertexArrayOES(this.edgeDetectVAO);
        view.setFramebufferSizeUniform(edgeDetectProgram.uniforms);
        view.setTransformSTAndTexScaleUniformsForDest(edgeDetectProgram.uniforms);
        view.gl.activeTexture(view.gl.TEXTURE0);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.directColorTexture);
        view.gl.uniform1i(edgeDetectProgram.uniforms.uColor, 0);
        view.gl.activeTexture(view.gl.TEXTURE1);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.directPathIDTexture);
        view.gl.uniform1i(edgeDetectProgram.uniforms.uPathID, 1);
        view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);
        view.gl.drawElements(view.gl.TRIANGLES, 6, view.gl.UNSIGNED_BYTE, 0);
        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected setCoverDepthState(view: MonochromePathfinderView) {
        view.gl.depthMask(false);
        view.gl.depthFunc(view.gl.ALWAYS);
        view.gl.enable(view.gl.DEPTH_TEST);
    }

    protected clearForCover(view: MonochromePathfinderView) {
        view.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT);
    }

    protected setAADepthState(view: MonochromePathfinderView) {
        view.gl.depthMask(false);
        view.gl.depthFunc(view.gl.EQUAL);
        view.gl.enable(view.gl.DEPTH_TEST);
    }

    protected setResolveDepthState(view: MonochromePathfinderView) {
        view.gl.depthMask(false);
        view.gl.depthFunc(view.gl.NOTEQUAL);
        view.gl.enable(view.gl.DEPTH_TEST);
    }

    protected clearForResolve(view: MonochromePathfinderView) {}

    protected setResolveUniforms(view: MonochromePathfinderView, program: PathfinderShaderProgram) {
        view.gl.activeTexture(view.gl.TEXTURE1);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.bgColorTexture);
        view.gl.uniform1i(program.uniforms.uBGColor, 1);
        view.gl.activeTexture(view.gl.TEXTURE2);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.fgColorTexture);
        view.gl.uniform1i(program.uniforms.uFGColor, 2);
    }

    get shouldRenderDirect() {
        return true;
    }

    protected get directDepthTexture(): WebGLTexture {
        return this._directDepthTexture;
    }

    private _directDepthTexture: WebGLTexture;

    private edgeDetectFramebuffer: WebGLFramebuffer;
    private edgeDetectVAO: WebGLVertexArrayObject;
    private bgColorTexture: WebGLTexture;
    private fgColorTexture: WebGLTexture;
}
