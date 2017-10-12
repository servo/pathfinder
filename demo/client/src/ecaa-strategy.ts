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
import * as _ from 'lodash';

import {AntialiasingStrategy, SubpixelAAType} from './aa-strategy';
import PathfinderBufferTexture from './buffer-texture';
import {createFramebuffer, createFramebufferColorTexture} from './gl-utils';
import {createFramebufferDepthTexture, setTextureParameters, UniformMap} from './gl-utils';
import {WebGLVertexArrayObject} from './gl-utils';
import {B_QUAD_LOWER_INDICES_OFFSET, B_QUAD_SIZE, B_QUAD_UPPER_INDICES_OFFSET} from './meshes';
import {PathfinderShaderProgram} from './shader-loader';
import {FLOAT32_SIZE, UINT32_SIZE, unwrapNull} from './utils';
import {MonochromeDemoView} from './view';

interface FastEdgeVAOs {
    upper: WebGLVertexArrayObject;
    lower: WebGLVertexArrayObject;
}

type Direction = 'upper' | 'lower';

const DIRECTIONS: Direction[] = ['upper', 'lower'];

export abstract class ECAAStrategy extends AntialiasingStrategy {
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
    private coverVAO: WebGLVertexArrayObject;
    private fastLineVAOs: FastEdgeVAOs;
    private fastCurveVAOs: FastEdgeVAOs;
    private boldLineVAO: WebGLVertexArrayObject;
    private boldCurveVAO: WebGLVertexArrayObject;
    private resolveVAO: WebGLVertexArrayObject;

    private mode: 'fast' | 'bold' = 'bold';

    constructor(level: number, subpixelAA: SubpixelAAType) {
        super();

        this.subpixelAA = subpixelAA;

        this.supersampledFramebufferSize = glmatrix.vec2.create();
        this.destFramebufferSize = glmatrix.vec2.create();
    }

    init(view: MonochromeDemoView) {
        super.init(view);
    }

    attachMeshes(view: MonochromeDemoView) {
        const bVertexPositions = new Float32Array(view.meshData[0].bVertexPositions);
        const bVertexPathIDs = new Uint8Array(view.meshData[0].bVertexPathIDs);

        this.createEdgeDetectVAO(view);
        this.createCoverVAO(view);
        this.createFastLineVAOs(view);
        this.createFastCurveVAOs(view);
        this.createBoldLineVAO(view);
        this.createBoldCurveVAO(view);
        this.createResolveVAO(view);
    }

    setFramebufferSize(view: MonochromeDemoView) {
        this.destFramebufferSize = glmatrix.vec2.clone(view.destAllocatedSize);
        glmatrix.vec2.mul(this.supersampledFramebufferSize,
                          this.destFramebufferSize,
                          this.supersampleScale);

        this.initDirectFramebuffer(view);
        this.initEdgeDetectFramebuffer(view);
        this.initAAAlphaFramebuffer(view);
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, null);
    }

    prepare(view: MonochromeDemoView) {
        const usedSize = this.supersampledUsedSize(view);
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

    antialias(view: MonochromeDemoView) {
        // Detect edges if necessary.
        this.detectEdgesIfNecessary(view);

        // Set up antialiasing.
        this.prepareAA(view);

        // Conservatively cover if needed, and antialias.
        if (this.mode === 'fast') {
            this.cover(view);
            this.antialiasLines(view);
            this.antialiasCurves(view);
        } else {
            this.antialiasLinesBold(view);
            this.antialiasCurvesBold(view);
        }
    }

    resolve(view: MonochromeDemoView) {
        // Resolve the antialiasing.
        this.resolveAA(view);
    }

    get transform(): glmatrix.mat4 {
        return glmatrix.mat4.create();
    }

    protected initDirectFramebuffer(view: MonochromeDemoView) {
        this.directColorTexture = createFramebufferColorTexture(view, this.destFramebufferSize);
        this.directPathIDTexture = createFramebufferColorTexture(view, this.destFramebufferSize);
        this.directFramebuffer =
            createFramebuffer(view.gl,
                              view.drawBuffersExt,
                              [this.directColorTexture, this.directPathIDTexture],
                              this.directDepthTexture);
    }

    protected setCoverDepthState(view: MonochromeDemoView): void {
        view.gl.disable(view.gl.DEPTH_TEST);
    }

    protected setResolveDepthState(view: MonochromeDemoView): void {
        view.gl.disable(view.gl.DEPTH_TEST);
    }

    protected supersampledUsedSize(view: MonochromeDemoView): glmatrix.vec2 {
        const usedSize = glmatrix.vec2.create();
        glmatrix.vec2.mul(usedSize, view.destUsedSize, this.supersampleScale);
        return usedSize;
    }

    protected abstract getResolveProgram(view: MonochromeDemoView): PathfinderShaderProgram;
    protected abstract initEdgeDetectFramebuffer(view: MonochromeDemoView): void;
    protected abstract createEdgeDetectVAO(view: MonochromeDemoView): void;
    protected abstract detectEdgesIfNecessary(view: MonochromeDemoView): void;
    protected abstract clearForCover(view: MonochromeDemoView): void;
    protected abstract setAADepthState(view: MonochromeDemoView): void;
    protected abstract clearForResolve(view: MonochromeDemoView): void;
    protected abstract setResolveUniforms(view: MonochromeDemoView,
                                          program: PathfinderShaderProgram): void;

    private initAAAlphaFramebuffer(view: MonochromeDemoView) {
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

    private createPathBoundsBufferTexture(view: MonochromeDemoView) {
        const pathBounds = view.pathBoundingRects(0);
        this.pathBoundsBufferTexture = new PathfinderBufferTexture(view.gl, 'uPathBounds');
        this.pathBoundsBufferTexture.upload(view.gl, pathBounds);
    }

    private createCoverVAO(view: MonochromeDemoView) {
        this.coverVAO = view.vertexArrayObjectExt.createVertexArrayOES();
        view.vertexArrayObjectExt.bindVertexArrayOES(this.coverVAO);

        const coverProgram = view.shaderPrograms.ecaaFastCover;
        const attributes = coverProgram.attributes;
        view.gl.useProgram(coverProgram.program);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadPositionsBuffer);
        view.gl.vertexAttribPointer(attributes.aQuadPosition, 2, view.gl.FLOAT, false, 0, 0);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.meshes[0].edgeBoundingBoxVertexPositions);
        view.gl.vertexAttribPointer(attributes.aUpperLeftPosition,
                                    2,
                                    view.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 4,
                                    0);
        view.gl.vertexAttribPointer(attributes.aLowerRightPosition,
                                    2,
                                    view.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 4,
                                    FLOAT32_SIZE * 2);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.meshes[0].edgeBoundingBoxPathIDs);
        view.gl.vertexAttribPointer(attributes.aPathID,
                                    1,
                                    view.gl.UNSIGNED_SHORT,
                                    false,
                                    0,
                                    0);
        view.gl.enableVertexAttribArray(attributes.aQuadPosition);
        view.gl.enableVertexAttribArray(attributes.aUpperLeftPosition);
        view.gl.enableVertexAttribArray(attributes.aLowerRightPosition);
        view.gl.enableVertexAttribArray(attributes.aPathID);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aUpperLeftPosition, 1);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLowerRightPosition, 1);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);
        view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private createFastLineVAOs(view: MonochromeDemoView) {
        const boldLineProgram = view.shaderPrograms.ecaaBoldLine;

        const vaos: Partial<FastEdgeVAOs> = {};
        const lineProgram = boldLineProgram;
        const attributes = lineProgram.attributes;

        for (const direction of DIRECTIONS) {
            const vao = view.vertexArrayObjectExt.createVertexArrayOES();
            view.vertexArrayObjectExt.bindVertexArrayOES(vao);

            const lineVertexPositionsBuffer = {
                lower: view.meshes[0].edgeLowerLineVertexPositions,
                upper: view.meshes[0].edgeUpperLineVertexPositions,
            }[direction];
            const linePathIDsBuffer = {
                lower: view.meshes[0].edgeLowerLinePathIDs,
                upper: view.meshes[0].edgeUpperLinePathIDs,
            }[direction];

            view.gl.useProgram(lineProgram.program);
            view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadPositionsBuffer);
            view.gl.vertexAttribPointer(attributes.aQuadPosition, 2, view.gl.FLOAT, false, 0, 0);
            view.gl.bindBuffer(view.gl.ARRAY_BUFFER, lineVertexPositionsBuffer);
            view.gl.vertexAttribPointer(attributes.aLeftPosition,
                                        2,
                                        view.gl.FLOAT,
                                        false,
                                        FLOAT32_SIZE * 4,
                                        0);
            view.gl.vertexAttribPointer(attributes.aRightPosition,
                                        2,
                                        view.gl.FLOAT,
                                        false,
                                        FLOAT32_SIZE * 4,
                                        FLOAT32_SIZE * 2);
            view.gl.bindBuffer(view.gl.ARRAY_BUFFER, linePathIDsBuffer);
            view.gl.vertexAttribPointer(attributes.aPathID,
                                        1,
                                        view.gl.UNSIGNED_SHORT,
                                        false,
                                        0,
                                        0);

            view.gl.enableVertexAttribArray(attributes.aQuadPosition);
            view.gl.enableVertexAttribArray(attributes.aLeftPosition);
            view.gl.enableVertexAttribArray(attributes.aRightPosition);
            view.gl.enableVertexAttribArray(attributes.aPathID);

            view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
            view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightPosition, 1);
            view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);

            view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);

            vaos[direction] = vao;
        }

        view.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.fastLineVAOs = vaos as FastEdgeVAOs;
    }

    private createFastCurveVAOs(view: MonochromeDemoView) {
        const vaos: Partial<FastEdgeVAOs> = {};
        const curveProgram = view.shaderPrograms.ecaaFastCurve;
        const attributes = curveProgram.attributes;

        for (const direction of DIRECTIONS) {
            const vao = view.vertexArrayObjectExt.createVertexArrayOES();
            view.vertexArrayObjectExt.bindVertexArrayOES(vao);

            const curveVertexPositionsBuffer = {
                lower: view.meshes[0].edgeLowerCurveVertexPositions,
                upper: view.meshes[0].edgeUpperCurveVertexPositions,
            }[direction];
            const curvePathIDsBuffer = {
                lower: view.meshes[0].edgeLowerCurvePathIDs,
                upper: view.meshes[0].edgeUpperCurvePathIDs,
            }[direction];

            view.gl.useProgram(curveProgram.program);
            view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadPositionsBuffer);
            view.gl.vertexAttribPointer(attributes.aQuadPosition, 2, view.gl.FLOAT, false, 0, 0);
            view.gl.bindBuffer(view.gl.ARRAY_BUFFER, curveVertexPositionsBuffer);
            view.gl.vertexAttribPointer(attributes.aLeftPosition,
                                        2,
                                        view.gl.FLOAT,
                                        false,
                                        FLOAT32_SIZE * 6,
                                        0);
            view.gl.vertexAttribPointer(attributes.aControlPointPosition,
                                        2,
                                        view.gl.FLOAT,
                                        false,
                                        FLOAT32_SIZE * 6,
                                        FLOAT32_SIZE * 2);
            view.gl.vertexAttribPointer(attributes.aRightPosition,
                                        2,
                                        view.gl.FLOAT,
                                        false,
                                        FLOAT32_SIZE * 6,
                                        FLOAT32_SIZE * 4);
            view.gl.bindBuffer(view.gl.ARRAY_BUFFER, curvePathIDsBuffer);
            view.gl.vertexAttribPointer(attributes.aPathID,
                                        1,
                                        view.gl.UNSIGNED_SHORT,
                                        false,
                                        0,
                                        0);

            view.gl.enableVertexAttribArray(attributes.aQuadPosition);
            view.gl.enableVertexAttribArray(attributes.aLeftPosition);
            view.gl.enableVertexAttribArray(attributes.aControlPointPosition);
            view.gl.enableVertexAttribArray(attributes.aRightPosition);
            view.gl.enableVertexAttribArray(attributes.aPathID);

            view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
            view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aControlPointPosition,
                                                                1);
            view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightPosition, 1);
            view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);

            view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);

            vaos[direction] = vao;
        }

        view.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.fastCurveVAOs = vaos as FastEdgeVAOs;
    }

    private createBoldLineVAO(view: MonochromeDemoView) {
        const lineProgram = view.shaderPrograms.ecaaBoldLine;
        const attributes = lineProgram.attributes;

        const vao = view.vertexArrayObjectExt.createVertexArrayOES();
        view.vertexArrayObjectExt.bindVertexArrayOES(vao);

        const lineVertexPositionsBuffer = view.meshes[0].segmentLines;
        const linePathIDsBuffer = view.meshes[0].segmentLinePathIDs;
        const lineNormalsBuffer = view.meshes[0].segmentLineNormals;

        view.gl.useProgram(lineProgram.program);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadPositionsBuffer);
        view.gl.vertexAttribPointer(attributes.aQuadPosition, 2, view.gl.FLOAT, false, 0, 0);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, lineVertexPositionsBuffer);
        view.gl.vertexAttribPointer(attributes.aLeftPosition,
                                    2,
                                    view.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 4,
                                    0);
        view.gl.vertexAttribPointer(attributes.aRightPosition,
                                    2,
                                    view.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 4,
                                    FLOAT32_SIZE * 2);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, linePathIDsBuffer);
        view.gl.vertexAttribPointer(attributes.aPathID,
                                    1,
                                    view.gl.UNSIGNED_SHORT,
                                    false,
                                    0,
                                    0);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, lineNormalsBuffer);
        view.gl.vertexAttribPointer(attributes.aLeftNormalAngle,
                                    1,
                                    view.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 2,
                                    0);
        view.gl.vertexAttribPointer(attributes.aRightNormalAngle,
                                    1,
                                    view.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 2,
                                    FLOAT32_SIZE);

        view.gl.enableVertexAttribArray(attributes.aQuadPosition);
        view.gl.enableVertexAttribArray(attributes.aLeftPosition);
        view.gl.enableVertexAttribArray(attributes.aRightPosition);
        view.gl.enableVertexAttribArray(attributes.aPathID);
        view.gl.enableVertexAttribArray(attributes.aLeftNormalAngle);
        view.gl.enableVertexAttribArray(attributes.aRightNormalAngle);

        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightPosition, 1);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftNormalAngle, 1);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightNormalAngle, 1);

        view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);

        view.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.boldLineVAO = vao;
    }

    private createBoldCurveVAO(view: MonochromeDemoView) {
        const curveProgram = view.shaderPrograms.ecaaBoldCurve;
        const attributes = curveProgram.attributes;

        const vao = view.vertexArrayObjectExt.createVertexArrayOES();
        view.vertexArrayObjectExt.bindVertexArrayOES(vao);

        const curveVertexPositionsBuffer = view.meshes[0].segmentCurves;
        const curvePathIDsBuffer = view.meshes[0].segmentCurvePathIDs;
        const curveNormalsBuffer = view.meshes[0].segmentCurveNormals;

        view.gl.useProgram(curveProgram.program);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, view.quadPositionsBuffer);
        view.gl.vertexAttribPointer(attributes.aQuadPosition, 2, view.gl.FLOAT, false, 0, 0);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, curveVertexPositionsBuffer);
        view.gl.vertexAttribPointer(attributes.aLeftPosition,
                                    2,
                                    view.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 6,
                                    0);
        view.gl.vertexAttribPointer(attributes.aControlPointPosition,
                                    2,
                                    view.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 6,
                                    FLOAT32_SIZE * 2);
        view.gl.vertexAttribPointer(attributes.aRightPosition,
                                    2,
                                    view.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 6,
                                    FLOAT32_SIZE * 4);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, curvePathIDsBuffer);
        view.gl.vertexAttribPointer(attributes.aPathID,
                                    1,
                                    view.gl.UNSIGNED_SHORT,
                                    false,
                                    0,
                                    0);
        view.gl.bindBuffer(view.gl.ARRAY_BUFFER, curveNormalsBuffer);
        view.gl.vertexAttribPointer(attributes.aLeftNormalAngle,
                                    1,
                                    view.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 2,
                                    0);
        view.gl.vertexAttribPointer(attributes.aRightNormalAngle,
                                    1,
                                    view.gl.FLOAT,
                                    false,
                                    FLOAT32_SIZE * 2,
                                    FLOAT32_SIZE);

        view.gl.enableVertexAttribArray(attributes.aQuadPosition);
        view.gl.enableVertexAttribArray(attributes.aLeftPosition);
        view.gl.enableVertexAttribArray(attributes.aControlPointPosition);
        view.gl.enableVertexAttribArray(attributes.aRightPosition);
        view.gl.enableVertexAttribArray(attributes.aPathID);
        view.gl.enableVertexAttribArray(attributes.aLeftNormalAngle);
        view.gl.enableVertexAttribArray(attributes.aRightNormalAngle);

        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftPosition, 1);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aControlPointPosition, 1);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightPosition, 1);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aPathID, 1);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aLeftNormalAngle, 1);
        view.instancedArraysExt.vertexAttribDivisorANGLE(attributes.aRightNormalAngle, 1);

        view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);

        view.vertexArrayObjectExt.bindVertexArrayOES(null);

        this.boldCurveVAO = vao;
    }

    private createResolveVAO(view: MonochromeDemoView) {
        this.resolveVAO = view.vertexArrayObjectExt.createVertexArrayOES();
        view.vertexArrayObjectExt.bindVertexArrayOES(this.resolveVAO);

        const resolveProgram = this.getResolveProgram(view);
        view.gl.useProgram(resolveProgram.program);
        view.initQuadVAO(resolveProgram.attributes);

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private prepareAA(view: MonochromeDemoView): void {
        // Set state for antialiasing.
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

        this.createPathBoundsBufferTexture(view);
    }

    private cover(view: MonochromeDemoView): void {
        // Conservatively cover.
        const coverProgram = view.shaderPrograms.ecaaFastCover;
        view.gl.useProgram(coverProgram.program);
        view.vertexArrayObjectExt.bindVertexArrayOES(this.coverVAO);
        this.setAAUniforms(view, coverProgram.uniforms);
        view.instancedArraysExt.drawElementsInstancedANGLE(view.gl.TRIANGLES,
                                                           6,
                                                           view.gl.UNSIGNED_BYTE,
                                                           0,
                                                           view.meshData[0].bQuadCount);
        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private setAAState(view: MonochromeDemoView) {
        const usedSize = this.supersampledUsedSize(view);
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, this.aaFramebuffer);
        view.gl.viewport(0,
                         0,
                         this.supersampledFramebufferSize[0],
                         this.supersampledFramebufferSize[1]);
        view.gl.scissor(0, 0, usedSize[0], usedSize[1]);
        view.gl.enable(view.gl.SCISSOR_TEST);

        this.setAADepthState(view);
    }

    private setBlendModeForAA(view: MonochromeDemoView) {
        if (this.mode === 'bold')
            view.gl.blendEquation(view.gl.FUNC_ADD);
        else
            view.gl.blendEquation(view.gl.FUNC_REVERSE_SUBTRACT);

        view.gl.blendFunc(view.gl.ONE, view.gl.ONE);
        view.gl.enable(view.gl.BLEND);
    }

    private setAAUniforms(view: MonochromeDemoView, uniforms: UniformMap) {
        view.setTransformSTUniform(uniforms, 0);
        view.setFramebufferSizeUniform(uniforms);
        view.pathTransformBufferTextures[0].bind(view.gl, uniforms, 0);
        this.pathBoundsBufferTexture.bind(view.gl, uniforms, 1);
        view.setHintsUniform(uniforms);
        view.gl.uniform1f(uniforms.uEmboldenAmount, 25.0);
    }

    private antialiasLines(view: MonochromeDemoView) {
        this.setAAState(view);

        const lineProgram = view.shaderPrograms.ecaaFastLine;

        view.gl.useProgram(lineProgram.program);
        const uniforms = lineProgram.uniforms;
        this.setAAUniforms(view, uniforms);

        for (const direction of DIRECTIONS) {
            const vao = this.fastLineVAOs[direction];
            view.vertexArrayObjectExt.bindVertexArrayOES(vao);

            this.setBlendModeForAA(view);
            view.gl.uniform1i(uniforms.uLowerPart, direction === 'lower' ? 1 : 0);

            const count = {
                lower: view.meshData[0].edgeLowerLineCount,
                upper: view.meshData[0].edgeUpperLineCount,
            }[direction];
            view.instancedArraysExt.drawElementsInstancedANGLE(view.gl.TRIANGLES,
                                                               6,
                                                               view.gl.UNSIGNED_BYTE,
                                                               0,
                                                               count);
        }

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private antialiasCurves(view: MonochromeDemoView) {
        this.setAAState(view);

        const curveProgram = view.shaderPrograms.ecaaFastCurve;

        view.gl.useProgram(curveProgram.program);
        const uniforms = curveProgram.uniforms;
        this.setAAUniforms(view, uniforms);

        for (const direction of DIRECTIONS) {
            const vao = this.fastCurveVAOs[direction];
            view.vertexArrayObjectExt.bindVertexArrayOES(vao);

            this.setBlendModeForAA(view);
            view.gl.uniform1i(uniforms.uLowerPart, direction === 'lower' ? 1 : 0);

            const count = {
                lower: view.meshData[0].edgeLowerCurveCount,
                upper: view.meshData[0].edgeUpperCurveCount,
            }[direction];
            view.instancedArraysExt.drawElementsInstancedANGLE(view.gl.TRIANGLES,
                                                               6,
                                                               view.gl.UNSIGNED_BYTE,
                                                               0,
                                                               count);
        }

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private antialiasLinesBold(view: MonochromeDemoView) {
        this.setAAState(view);

        const lineProgram = view.shaderPrograms.ecaaBoldLine;

        view.gl.useProgram(lineProgram.program);
        const uniforms = lineProgram.uniforms;
        this.setAAUniforms(view, uniforms);

        const vao = this.boldLineVAO;
        view.vertexArrayObjectExt.bindVertexArrayOES(vao);

        this.setBlendModeForAA(view);

        const count = view.meshData[0].segmentLineCount;
        view.instancedArraysExt.drawElementsInstancedANGLE(view.gl.TRIANGLES,
                                                           6,
                                                           view.gl.UNSIGNED_BYTE,
                                                           0,
                                                           count);

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private antialiasCurvesBold(view: MonochromeDemoView) {
        this.setAAState(view);

        const curveProgram = view.shaderPrograms.ecaaBoldCurve;

        view.gl.useProgram(curveProgram.program);
        const uniforms = curveProgram.uniforms;
        this.setAAUniforms(view, uniforms);

        const vao = this.boldCurveVAO;
        view.vertexArrayObjectExt.bindVertexArrayOES(vao);

        this.setBlendModeForAA(view);

        const count = view.meshData[0].segmentCurveCount;
        view.instancedArraysExt.drawElementsInstancedANGLE(view.gl.TRIANGLES,
                                                           6,
                                                           view.gl.UNSIGNED_BYTE,
                                                           0,
                                                           count);
        console.log("drew", count, "curves");

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    private resolveAA(view: MonochromeDemoView) {
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

    protected get directDepthTexture(): WebGLTexture | null {
        return null;
    }

    protected get supersampleScale(): glmatrix.vec2 {
        return glmatrix.vec2.fromValues(this.subpixelAA !== 'none' ? 3.0 : 1.0, 1.0);
    }
}

export class ECAAMonochromeStrategy extends ECAAStrategy {
    protected getResolveProgram(view: MonochromeDemoView): PathfinderShaderProgram {
        if (this.subpixelAA !== 'none')
            return view.shaderPrograms.ecaaMonoSubpixelResolve;
        return view.shaderPrograms.ecaaMonoResolve;
    }

    protected initEdgeDetectFramebuffer(view: MonochromeDemoView) {}

    protected createEdgeDetectVAO(view: MonochromeDemoView) {}

    protected detectEdgesIfNecessary(view: MonochromeDemoView) {}

    protected clearForCover(view: MonochromeDemoView) {
        view.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        view.gl.clearDepth(0.0);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT | view.gl.DEPTH_BUFFER_BIT);
    }

    protected setAADepthState(view: MonochromeDemoView) {
        view.gl.disable(view.gl.DEPTH_TEST);
    }

    protected clearForResolve(view: MonochromeDemoView) {
        view.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT);
    }

    protected setResolveUniforms(view: MonochromeDemoView, program: PathfinderShaderProgram) {
        view.gl.uniform4fv(program.uniforms.uBGColor, view.bgColor);
        view.gl.uniform4fv(program.uniforms.uFGColor, view.fgColor);
    }

    get shouldRenderDirect() {
        return false;
    }
}

export class ECAAMulticolorStrategy extends ECAAStrategy {
    private _directDepthTexture: WebGLTexture;

    private edgeDetectFramebuffer: WebGLFramebuffer;
    private edgeDetectVAO: WebGLVertexArrayObject;
    private bgColorTexture: WebGLTexture;
    private fgColorTexture: WebGLTexture;

    protected getResolveProgram(view: MonochromeDemoView): PathfinderShaderProgram {
        return view.shaderPrograms.ecaaMultiResolve;
    }

    protected initDirectFramebuffer(view: MonochromeDemoView) {
        this._directDepthTexture =
            createFramebufferDepthTexture(view.gl, this.supersampledFramebufferSize);
        super.initDirectFramebuffer(view);
    }

    protected initEdgeDetectFramebuffer(view: MonochromeDemoView) {
        this.bgColorTexture = createFramebufferColorTexture(view,
                                                            this.supersampledFramebufferSize);
        this.fgColorTexture = createFramebufferColorTexture(view,
                                                            this.supersampledFramebufferSize);
        this.edgeDetectFramebuffer = createFramebuffer(view.gl,
                                                       view.drawBuffersExt,
                                                       [this.bgColorTexture, this.fgColorTexture],
                                                       this.aaDepthTexture);
    }

    protected createEdgeDetectVAO(view: MonochromeDemoView) {
        this.edgeDetectVAO = view.vertexArrayObjectExt.createVertexArrayOES();
        view.vertexArrayObjectExt.bindVertexArrayOES(this.edgeDetectVAO);

        const edgeDetectProgram = view.shaderPrograms.ecaaEdgeDetect;
        view.gl.useProgram(edgeDetectProgram.program);
        view.initQuadVAO(edgeDetectProgram.attributes);

        view.vertexArrayObjectExt.bindVertexArrayOES(null);
    }

    protected detectEdgesIfNecessary(view: MonochromeDemoView) {
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

    protected setCoverDepthState(view: MonochromeDemoView) {
        view.gl.depthMask(false);
        view.gl.depthFunc(view.gl.ALWAYS);
        view.gl.enable(view.gl.DEPTH_TEST);
    }

    protected clearForCover(view: MonochromeDemoView) {
        view.gl.clearColor(0.0, 0.0, 0.0, 0.0);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT);
    }

    protected setAADepthState(view: MonochromeDemoView) {
        view.gl.depthMask(false);
        view.gl.depthFunc(view.gl.EQUAL);
        view.gl.enable(view.gl.DEPTH_TEST);
    }

    protected setResolveDepthState(view: MonochromeDemoView) {
        view.gl.depthMask(false);
        view.gl.depthFunc(view.gl.NOTEQUAL);
        view.gl.enable(view.gl.DEPTH_TEST);
    }

    protected clearForResolve(view: MonochromeDemoView) {}

    protected setResolveUniforms(view: MonochromeDemoView, program: PathfinderShaderProgram) {
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
}
